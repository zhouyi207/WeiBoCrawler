import { useCallback, useEffect, useRef, useState } from "react";
import { AlertCircleIcon } from "lucide-react";
import { toast } from "sonner";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { FloatingScrollArea } from "@/app/ui/FloatingScrollArea";
import {
  PLATFORMS,
  PLATFORM_LABELS,
  type Platform,
  type ProxyGlobalRow,
  type ProxyHealthBrief,
  type ProxyIp,
  type ProxyPlatformRow,
} from "@/features/domain/types";
import {
  checkAllProxiesDualHealth,
  deleteProxy,
  listProxiesGlobal,
  listProxiesHealth,
  listProxiesRuntime,
} from "@/services/tauri/commands";
import { formatProxyEndpoint } from "./utils/format";
import { AddProxyDialog } from "./AddProxyDialog";
import { IpLogModal } from "./IpLogModal";
import { ProxyProbeSettingsDialog } from "./ProxyProbeSettingsDialog";
import {
  DualHealthRefreshDialog,
  type DualHealthRefreshMode,
  type DualHealthRefreshStatus,
} from "./DualHealthRefreshDialog";
import { GlobalIpTable } from "./components/GlobalIpTable";
import { IpPageHeader } from "./components/IpPageHeader";
import { PlatformIpTable } from "./components/PlatformIpTable";

/**
 * IP 代理管理页：tab = 全局 + 各平台。
 *
 * 拆分原则：
 * - 行 / 表格 / 头部 / 卡片 / 工具函数都放在 ./components 与 ./utils 下；
 * - 这个文件只剩「数据加载 + tab 状态机 + dialog 编排」。
 */

type ActiveTab = "global" | Platform;

export function IpPage() {
  const [activeTab, setActiveTab] = useState<ActiveTab>("global");
  const [globalRows, setGlobalRows] = useState<ProxyGlobalRow[]>([]);
  /** 各平台 tab 的最近一次拉取结果。切换 tab 时直接渲染缓存，再发请求覆盖。 */
  const [platformRows, setPlatformRows] = useState<Record<string, ProxyPlatformRow[]>>({});
  const [healthMap, setHealthMap] = useState<Record<string, ProxyHealthBrief>>({});
  const [loadingGlobal, setLoadingGlobal] = useState(true);
  const [loadingPlatform, setLoadingPlatform] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState("");

  const [addOpen, setAddOpen] = useState(false);
  const [editing, setEditing] = useState<ProxyIp | null>(null);
  const [deleting, setDeleting] = useState<ProxyIp | null>(null);
  const [deletingBusy, setDeletingBusy] = useState(false);
  const [viewingLog, setViewingLog] = useState<ProxyIp | null>(null);
  const [probeSettingsOpen, setProbeSettingsOpen] = useState(false);

  const [refreshDialogOpen, setRefreshDialogOpen] = useState(false);
  const [refreshDialogStatus, setRefreshDialogStatus] =
    useState<DualHealthRefreshStatus>("running");
  const [refreshDialogMode, setRefreshDialogMode] =
    useState<DualHealthRefreshMode>("global");
  const [refreshDialogStep, setRefreshDialogStep] = useState(0);
  const [refreshDialogError, setRefreshDialogError] = useState<string | null>(
    null,
  );
  const refreshPhaseTimerRef = useRef<ReturnType<typeof setInterval> | null>(
    null,
  );

  const clearRefreshPhaseTimer = useCallback(() => {
    if (refreshPhaseTimerRef.current) {
      clearInterval(refreshPhaseTimerRef.current);
      refreshPhaseTimerRef.current = null;
    }
  }, []);

  useEffect(() => () => clearRefreshPhaseTimer(), [clearRefreshPhaseTimer]);

  // ── data loading ──────────────────────────────────────────────────────────

  /** 健康档位（卡片 / IP 列徽章共用），失败不打断主流程。 */
  const refreshHealth = useCallback(async () => {
    try {
      const next = await listProxiesHealth();
      const map: Record<string, ProxyHealthBrief> = {};
      for (const h of next) map[h.id] = h;
      setHealthMap(map);
    } catch (e) {
      console.warn("[IpPage] listProxiesHealth failed:", e);
    }
  }, []);

  const loadGlobal = useCallback(async () => {
    setLoadingGlobal(true);
    setError("");
    try {
      const next = await listProxiesGlobal();
      setGlobalRows(next);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoadingGlobal(false);
    }
  }, []);

  const loadPlatform = useCallback(async (platform: Platform) => {
    setLoadingPlatform(true);
    setError("");
    try {
      const next = await listProxiesRuntime(platform);
      setPlatformRows((prev) => ({ ...prev, [platform]: next }));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoadingPlatform(false);
    }
  }, []);

  // 切换 tab 时拉一次该 tab 的数据；global tab 已在首屏拉过。
  useEffect(() => {
    if (activeTab === "global") {
      void loadGlobal();
    } else {
      void loadPlatform(activeTab);
    }
  }, [activeTab, loadGlobal, loadPlatform]);

  // 首屏额外拉一次健康档位，给 IP 列徽章用。
  useEffect(() => {
    void refreshHealth();
  }, [refreshHealth]);

  // ── refresh button 上下文 ──────────────────────────────────────────────────

  /**
   * 「刷新并测延迟」按钮按当前 tab 上下文转发：
   * - 全局：批量并行刷新 geo + cn / intl 双探针（每条 ~10s/条），
   *   后端一次性写回 `proxies` 行（geo_* / cn_latency_ms / intl_latency_ms /
   *   last_probed_at），返回最新全局行；同时把已缓存的 platform tab 数据全部失效，
   *   下次切换 tab 自动重拉（避免在 platform tab 看到陈旧的「绑定 / 运行账号数」）。
   * - 平台：先拉一次当前平台最新 runtime 快照（瞬时），再后台顺手刷新全局表 + 健康
   *   档位 + 其它已缓存 platform tab，让无论用户切去哪都能看到最新数字。
   */
  const handleRefresh = useCallback(async () => {
    clearRefreshPhaseTimer();
    const isGlobal = activeTab === "global";
    setRefreshDialogMode(isGlobal ? "global" : "platform");
    setRefreshDialogStatus("running");
    setRefreshDialogStep(0);
    setRefreshDialogError(null);
    setRefreshDialogOpen(true);

    const maxStep = isGlobal ? 2 : 1;
    const intervalMs = isGlobal ? 3200 : 480;
    refreshPhaseTimerRef.current = setInterval(() => {
      setRefreshDialogStep((s) => Math.min(s + 1, maxStep));
    }, intervalMs);

    setRefreshing(true);
    try {
      if (isGlobal) {
        const next = await checkAllProxiesDualHealth();
        clearRefreshPhaseTimer();
        setGlobalRows(next);
        setPlatformRows({});
        const okCn = next.filter((r) => r.cnLatency.kind === "success").length;
        const okIntl = next.filter((r) => r.intlLatency.kind === "success").length;
        const filledGeo = next.filter((r) => Boolean(r.geoCountry || r.geoCity)).length;
        toast.success("刷新并测延迟已完成", {
          description: `共 ${next.length} 条，国内可达 ${okCn} / 国外可达 ${okIntl}，地理命中 ${filledGeo}`,
        });
      } else {
        await loadPlatform(activeTab);
        const otherPlatforms = Object.keys(platformRows).filter(
          (p) => p !== activeTab,
        ) as Platform[];
        await Promise.allSettled([
          loadGlobal(),
          ...otherPlatforms.map((p) => loadPlatform(p)),
        ]);
        clearRefreshPhaseTimer();
        toast.success("已刷新当前平台数据");
      }
      void refreshHealth();
      setRefreshDialogStatus("done");
      window.setTimeout(() => setRefreshDialogOpen(false), 1400);
    } catch (e) {
      clearRefreshPhaseTimer();
      const msg = e instanceof Error ? e.message : String(e);
      setRefreshDialogError(msg);
      setRefreshDialogStatus("error");
      toast.error("刷新失败", { description: msg });
    } finally {
      setRefreshing(false);
    }
  }, [
    activeTab,
    clearRefreshPhaseTimer,
    loadGlobal,
    loadPlatform,
    platformRows,
    refreshHealth,
  ]);

  // ── 删除 ──────────────────────────────────────────────────────────────────

  const handleConfirmDelete = useCallback(async () => {
    if (!deleting) return;
    setDeletingBusy(true);
    try {
      await deleteProxy(deleting.id);
      // 同步从全局 + 各平台缓存里把该行移除。
      setGlobalRows((prev) => prev.filter((p) => p.id !== deleting.id));
      setPlatformRows((prev) => {
        const next: Record<string, ProxyPlatformRow[]> = {};
        for (const [k, list] of Object.entries(prev)) {
          next[k] = list.filter((p) => p.id !== deleting.id);
        }
        return next;
      });
      toast.success("代理已删除", {
        description: formatProxyEndpoint(deleting.address),
      });
      setDeleting(null);
    } catch (e) {
      toast.error("删除代理失败", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setDeletingBusy(false);
    }
  }, [deleting]);

  // ── render ────────────────────────────────────────────────────────────────

  const refreshDisabled =
    refreshing || (activeTab === "global" ? loadingGlobal : loadingPlatform);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden p-4">
      <div className="shrink-0">
        <IpPageHeader
          activeTabIsGlobal={activeTab === "global"}
          refreshing={refreshing}
          refreshDisabled={refreshDisabled}
          onOpenProbeSettings={() => setProbeSettingsOpen(true)}
          onRefresh={() => void handleRefresh()}
          onAdd={() => setAddOpen(true)}
        />
      </div>

      {error && (
        <Alert variant="destructive" className="shrink-0">
          <AlertCircleIcon />
          <AlertTitle>加载失败</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <Tabs
        value={activeTab}
        onValueChange={(v) => setActiveTab(v as ActiveTab)}
        className="min-h-0 flex flex-1 flex-col gap-2 overflow-hidden"
      >
        <TabsList className="shrink-0">
          <TabsTrigger value="global">全局</TabsTrigger>
          {PLATFORMS.map((p) => (
            <TabsTrigger key={p} value={p}>
              {PLATFORM_LABELS[p]}
            </TabsTrigger>
          ))}
        </TabsList>

        <TabsContent
          value="global"
          className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
        >
          <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <CardHeader className="flex shrink-0 flex-row flex-wrap items-center justify-between gap-2 space-y-0 pb-3">
              <CardTitle className="flex h-7 min-w-0 items-center text-base leading-7">
                全局视图
              </CardTitle>
              <div className="flex flex-wrap items-center justify-end gap-2">
                <span className="text-muted-foreground text-xs">
                  共 {globalRows.length} 条
                </span>
              </div>
            </CardHeader>
            <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden pt-0">
              <FloatingScrollArea className="min-h-0 flex-1">
                <div className="overflow-x-auto pr-2">
                  <GlobalIpTable
                    rows={globalRows}
                    loading={loadingGlobal}
                    healthMap={healthMap}
                    onViewLog={setViewingLog}
                    onEdit={setEditing}
                    onDelete={setDeleting}
                  />
                </div>
              </FloatingScrollArea>
            </CardContent>
          </Card>
        </TabsContent>

        {PLATFORMS.map((p) => {
          const platformList = platformRows[p] ?? [];
          return (
            <TabsContent
              key={p}
              value={p}
              className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
            >
              <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
                <CardHeader className="flex shrink-0 flex-row flex-wrap items-center justify-between gap-2 space-y-0 pb-3">
                  <CardTitle className="flex h-7 min-w-0 items-center text-base leading-7">
                    {PLATFORM_LABELS[p]} 运行画像
                  </CardTitle>
                  <div className="flex flex-wrap items-center justify-end gap-2">
                    <span className="text-muted-foreground text-xs">
                      共 {platformList.length} 条
                    </span>
                  </div>
                </CardHeader>
                <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden pt-0">
                  <FloatingScrollArea className="min-h-0 flex-1">
                    <div className="overflow-x-auto pr-2">
                      <PlatformIpTable
                        platform={p}
                        rows={platformList}
                        loading={loadingPlatform && !platformRows[p]}
                        onViewLog={setViewingLog}
                        onEdit={setEditing}
                        onDelete={setDeleting}
                      />
                    </div>
                  </FloatingScrollArea>
                </CardContent>
              </Card>
            </TabsContent>
          );
        })}
      </Tabs>

      <AddProxyDialog
        open={addOpen}
        onOpenChange={setAddOpen}
        onSubmitted={() => {
          // 新加完无论当前在哪个 tab，先刷一次全局；platform tab 下次切换会自动刷。
          void loadGlobal();
        }}
      />

      <AddProxyDialog
        open={editing != null}
        onOpenChange={(o) => {
          if (!o) setEditing(null);
        }}
        initial={editing}
        onSubmitted={(updated) => {
          // 编辑只动单行：globalRows 局部覆盖，platform 下次切换 tab 自动刷新。
          setGlobalRows((prev) =>
            prev.map((row) => (row.id === updated.id ? { ...row, ...updated } : row)),
          );
        }}
      />

      <IpLogModal
        open={viewingLog != null}
        onOpenChange={(o) => {
          if (!o) setViewingLog(null);
        }}
        proxy={viewingLog}
      />

      <ProxyProbeSettingsDialog
        open={probeSettingsOpen}
        onOpenChange={setProbeSettingsOpen}
      />

      <DualHealthRefreshDialog
        open={refreshDialogOpen}
        mode={refreshDialogMode}
        status={refreshDialogStatus}
        activeStepIndex={refreshDialogStep}
        errorMessage={refreshDialogError}
        onOpenChange={(o) => {
          setRefreshDialogOpen(o);
          if (!o) {
            clearRefreshPhaseTimer();
            setRefreshDialogError(null);
          }
        }}
      />

      <AlertDialog
        open={deleting != null}
        onOpenChange={(o) => {
          if (!o && !deletingBusy) setDeleting(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>删除该代理？</AlertDialogTitle>
            <AlertDialogDescription>
              将永久删除{" "}
              <span className="font-mono">
                {deleting ? formatProxyEndpoint(deleting.address) : ""}
              </span>
              。已绑定该代理的账号需手动改绑；正在使用该代理跑的任务会回退到本机直连。
              本操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deletingBusy}>取消</AlertDialogCancel>
            <AlertDialogAction
              disabled={deletingBusy}
              variant="destructive"
              onClick={(ev) => {
                ev.preventDefault();
                void handleConfirmDelete();
              }}
            >
              {deletingBusy ? "删除中…" : "确认删除"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
