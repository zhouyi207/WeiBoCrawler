import { useCallback, useEffect, useState } from "react";
import { AlertCircleIcon, RefreshCwIcon } from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import {
  PLATFORM_LABELS,
  type IpStatus,
  type ProxyHealthBrief,
  type ProxyIp,
  type ProxyLogEntry,
} from "@/features/domain/types";
import { listProxiesHealth, listProxyLogs } from "@/services/tauri/commands";
import { formatProxyEndpoint } from "./utils/format";

/** 全局档徽章（仅 `available` / `invalid`，标头使用）。 */
const GLOBAL_BADGE: Record<
  Extract<IpStatus, "available" | "invalid">,
  { label: string; variant: "default" | "destructive"; hint: string }
> = {
  available: {
    label: "出口可达",
    variant: "default",
    hint: "近 5 分钟内 network 错误未达到 ≥10 阈值，出口本身可用。",
  },
  invalid: {
    label: "失效",
    variant: "destructive",
    hint: "近 5 分钟内 ≥10 次网络类错误（超时、连不通、DNS 等），出口暂视为不可达。窗口滑出后可恢复；若要永久停用请删除该代理。",
  },
};

/** per-platform 受限项徽章。`restricted` 是该平台 scope 上的判定。 */
function platformLabel(p: string): string {
  return (PLATFORM_LABELS as Record<string, string>)[p] ?? p;
}

/**
 * 错误归因 → 友好文案。后端 `risk::ErrorKind::as_tag` 与 `attribute` 决定了
 * 哪些 kind 会落库（写到 proxy_failure_events）；这里只负责展示。
 */
const KIND_LABEL: Record<string, string> = {
  network: "网络异常",
  http_status: "HTTP 状态",
  login_required: "登录失效",
  business_reject: "业务拒绝",
  other: "其他",
};

function formatOccurredAt(iso: string): string {
  // 后端 RFC3339（带时区），转本地时间字符串。失败回退到原文。
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toLocaleString();
  } catch {
    return iso;
  }
}

/**
 * IP 日志 modal。展示：
 * - 顶部：派生健康档位 + 触发 IP 与备注（让用户对得上是哪条代理）；
 * - 中部：累计错误数 / 最近一次错误时间；
 * - 底部：最近 N 条失败事件流（仅按代理维度，不展示 IP↔账号关联）。
 *
 * 所有数据均来自 `proxy_failure_events`：worker 在请求失败 + `risk::record`
 * 归因到代理时同时写入。事件超过 24h 由 `risk_event_repo::purge_older_than`
 * 在 scheduler 退出前清理；这里看到的就是窗口期内的实时画像。
 *
 * 注：业务上明确「IP 与账号仅在任务配置做笛卡尔积，运行期不再耦合」，
 * 因此本视图刻意不引入 listAccounts / accountId 解析。
 */
export function IpLogModal({
  open,
  onOpenChange,
  proxy,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 关闭时为 `null`，避免空打开。 */
  proxy: ProxyIp | null;
}) {
  const [logs, setLogs] = useState<ProxyLogEntry[]>([]);
  const [health, setHealth] = useState<ProxyHealthBrief | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    if (!proxy) return;
    setLoading(true);
    setError("");
    try {
      const [logsRes, healthRes] = await Promise.all([
        listProxyLogs(proxy.id, 100),
        listProxiesHealth(),
      ]);
      setLogs(logsRes);
      const mine = healthRes.find((h) => h.id === proxy.id);
      setHealth(
        mine ?? { id: proxy.id, globalStatus: "available", restrictions: [] },
      );
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      toast.error("加载日志失败", { description: msg });
    } finally {
      setLoading(false);
    }
  }, [proxy]);

  useEffect(() => {
    if (open && proxy) {
      void refresh();
    } else if (!open) {
      // 关闭时清空，避免下次打开闪过上一条代理的旧数据。
      setLogs([]);
      setHealth(null);
      setError("");
    }
  }, [open, proxy, refresh]);

  const globalBadge =
    health?.globalStatus === "invalid"
      ? GLOBAL_BADGE.invalid
      : health
        ? GLOBAL_BADGE.available
        : null;
  const restrictedPlatforms = health?.restrictions ?? [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {/*
        Body：`flex-1 min-h-0 overflow-y-auto` 不加 vertical padding，sticky 标题才能贴顶；
        统计区 `px-4 pt-3`，「事件时间线」`sticky top-0` 全宽，列表 `px-4 pb-4`。
      */}
      <DialogContent className="flex max-h-[85vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl">
        <DialogHeader className="shrink-0 border-b border-border/60 px-4 py-3">
          <DialogTitle className="flex flex-wrap items-center gap-2">
            <span>IP 日志</span>
            {globalBadge && (
              <Badge variant={globalBadge.variant} title={globalBadge.hint}>
                {globalBadge.label}
              </Badge>
            )}
            {/*
              v4 / 方案 C：受限是 (IP, platform) scope 的语义，把每个被限平台
              单独以小徽章列出来，方便用户一眼看到「在哪些平台不可用」。
              全局已经 invalid 时后端 restrictions[] 就是空，不会重复展示。
            */}
            {restrictedPlatforms.map((r) => (
              <Badge
                key={r.platform}
                variant="secondary"
                className="bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200"
                title={`该 IP 在「${platformLabel(r.platform)}」近 5 分钟内失败次数超阈值，已被该平台 scope 判定为受限；其它平台不受影响。`}
              >
                ⚠ {platformLabel(r.platform)}
              </Badge>
            ))}
          </DialogTitle>
          <DialogDescription className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span className="font-mono break-all text-foreground/80">
              {proxy ? formatProxyEndpoint(proxy.address) : "—"}
            </span>
            {proxy?.remark && (
              <span className="text-muted-foreground">· {proxy.remark}</span>
            )}
          </DialogDescription>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
          <div className="flex flex-col gap-4 px-4 pt-3">
            {error && (
              <Alert variant="destructive">
                <AlertCircleIcon />
                <AlertTitle>加载失败</AlertTitle>
                <AlertDescription className="break-words">
                  {error}
                </AlertDescription>
              </Alert>
            )}

            <div className="grid grid-cols-2 gap-3 text-sm">
              <div className="min-w-0 rounded-md border p-2">
                <div className="text-xs text-muted-foreground">近窗口错误数</div>
                <div className="text-lg font-semibold">{logs.length}</div>
              </div>
              <div className="min-w-0 rounded-md border p-2">
                <div className="text-xs text-muted-foreground">最近一次</div>
                <div
                  className="truncate text-sm"
                  title={logs[0]?.occurredAt ?? ""}
                >
                  {logs[0] ? formatOccurredAt(logs[0].occurredAt) : "—"}
                </div>
              </div>
            </div>

            <Separator />
          </div>

          <div className="sticky top-0 z-10 flex items-center justify-between gap-2 border-b border-border/60 bg-popover px-4 py-2 supports-backdrop-filter:bg-popover/95">
            <span className="min-w-0 text-sm font-medium">事件时间线</span>
            <Button
              variant="ghost"
              size="sm"
              className="shrink-0"
              onClick={() => void refresh()}
              disabled={loading}
            >
              <RefreshCwIcon
                className={cn("size-3.5", loading && "animate-spin")}
              />
              刷新
            </Button>
          </div>

          <div className="space-y-2 px-4 pb-4 pt-2">
            <div className="overflow-hidden rounded-md border">
              {loading && logs.length === 0 ? (
                <div className="p-6 text-center text-sm text-muted-foreground">
                  加载中…
                </div>
              ) : logs.length === 0 ? (
                <div className="p-6 text-center text-sm text-muted-foreground">
                  {/* 没有日志：要么真的没出过错，要么记录已被 24h 清理任务回收。 */}
                  近 24 小时内没有该代理的失败记录。
                </div>
              ) : (
                <ul className="px-1">
                  {logs.map((log, index) => (
                    <li
                      key={log.id}
                      className={cn(
                        "relative flex gap-3 py-3 pl-2 pr-1",
                        index !== logs.length - 1 && "border-b border-border/50",
                      )}
                    >
                      <div
                        className="flex w-5 shrink-0 flex-col items-center pt-1"
                        aria-hidden
                      >
                        <span className="z-1 size-2 shrink-0 rounded-full bg-muted-foreground/45 ring-2 ring-popover" />
                        {index !== logs.length - 1 ? (
                          <span className="mt-1 w-px flex-1 min-h-3 bg-border" />
                        ) : null}
                      </div>
                      <div className="min-w-0 flex-1 space-y-1.5">
                        <div className="flex flex-wrap items-center gap-1.5">
                          <Badge
                            variant={
                              log.errorKind === "network"
                                ? "destructive"
                                : "secondary"
                            }
                            className="max-w-full text-[10px] break-words whitespace-normal"
                          >
                            {KIND_LABEL[log.errorKind] ?? log.errorKind}
                            {log.httpStatus != null && ` · ${log.httpStatus}`}
                          </Badge>
                          {log.platform ? (
                            <Badge
                              variant="outline"
                              className="max-w-full text-[10px] font-normal break-words whitespace-normal"
                              title={`该次失败发生在「${platformLabel(log.platform)}」任务上`}
                            >
                              {platformLabel(log.platform)}
                            </Badge>
                          ) : (
                            <Badge
                              variant="outline"
                              className="max-w-full text-[10px] font-normal break-words whitespace-normal text-muted-foreground"
                              title="历史事件未带平台信息"
                            >
                              未知平台
                            </Badge>
                          )}
                        </div>
                        <time
                          className="block text-[11px] leading-snug text-muted-foreground tabular-nums"
                          title={log.occurredAt}
                        >
                          {formatOccurredAt(log.occurredAt)}
                        </time>
                        {log.message ? (
                          <p className="text-xs leading-relaxed text-muted-foreground [overflow-wrap:anywhere]">
                            {log.message}
                          </p>
                        ) : null}
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>

        <DialogFooter className="shrink-0 !m-0 flex-row justify-end gap-2 border-t bg-muted/30 px-4 py-3">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            关闭
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
