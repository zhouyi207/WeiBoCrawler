import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { PauseCircleIcon, ShieldAlertIcon, ShieldCheckIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { FloatingScrollArea } from "@/app/ui/FloatingScrollArea";
import {
  PLATFORM_LABELS,
  PLATFORMS,
  TASK_STATUS_LABELS,
  type CrawlTask,
  type DashboardStats,
  type Platform,
  type PlatformOverview,
} from "@/features/domain/types";
import {
  resolveTaskDisplayStatus,
  TASK_STATUS_BADGE_VARIANT,
} from "@/features/domain/taskDisplayStatus";
import {
  getDashboardStats,
  getTaskProgress,
  listTasks,
  type TaskProgress,
} from "@/services/tauri/commands";

const EMPTY_STATS: DashboardStats = {
  taskStats: { running: 0, paused: 0, error: 0, total: 0 },
  accountStats: { normal: 0, restricted: 0, error: 0, total: 0 },
  ipStats: { available: 0, restricted: 0, invalid: 0, total: 0 },
  perPlatform: [],
  recentLogs: [],
};

function crawlMetricsForPlatform(
  tasks: CrawlTask[],
  platform: Platform,
  progressMap: Record<string, TaskProgress | undefined>,
) {
  const list = tasks.filter((t) => t.platform === platform);
  const running = list.filter(
    (t) => resolveTaskDisplayStatus(t, progressMap[t.id]) === "running",
  ).length;
  return { running, total: list.length };
}

/**
 * 首页「平台健康概览」表格：账号 × IP × 采集（与账号、IP 同级的一级分组「采集」）。
 *
 * ```
 *                  ┌─── 账号 ───┐  ┌─── IP ───┐  ┌── 采集 ─────┐
 *  平台    正常  受限  异常  | 可用  受限  异常 | 运行中 任务 活跃率
 *  微博     3    0     1    |  4    1     0   |   1     4    25%
 *  抖音     2    1     0    |  3    2     0   |   0     2     —
 * ```
 *
 * **IP 列重叠语义提醒**（与后端 `PlatformOverview` 一致）：
 * - 同一个 `globalStatus = invalid` 的代理会在**每个平台行的「异常」列**都被计入；
 * - 同一个被限的代理只会在它真正受限的那个平台行的「受限」列里出现，
 *   在其它平台行里仍然算「可用」。
 *
 * 所以行间不要做加和判断；这是 per-platform scope 派生，不是 disjoint 分桶。
 *
 * 行集合按 [`PLATFORMS`] 顺序固定，但只渲染「实际有账号的平台」
 * （`accountTotal > 0`）。后端通常已经在源头过滤过；这里再兜一次防御。
 */
function PlatformHealthOverview({
  perPlatform,
  tasks,
  progressMap,
}: {
  perPlatform: PlatformOverview[];
  tasks: CrawlTask[];
  progressMap: Record<string, TaskProgress | undefined>;
}) {
  const byPlatform = new Map(perPlatform.map((p) => [p.platform, p] as const));
  const rows = PLATFORMS.map((p) => byPlatform.get(p)).filter(
    (s): s is PlatformOverview => !!s && s.accountTotal > 0,
  );

  if (rows.length === 0) {
    return <p className="text-sm text-muted-foreground">暂无账号</p>;
  }

  // 10 列：平台 | 账号×3 | IP×3 | 采集×3（与账号、IP 相同的一级分组「采集」）。
  const GRID_COLS =
    "grid-cols-[1fr_3.25rem_3.25rem_3.25rem_3.25rem_3.25rem_3.25rem_3.25rem_3.25rem_3.25rem]";

  return (
    <div className="space-y-1">
      {/* 一级表头：账号 / IP / 采集 */}
      <div
        className={`grid ${GRID_COLS} items-center gap-2 px-2 pb-1 text-xs font-medium text-muted-foreground`}
      >
        <span />
        <span className="col-span-3 text-center">账号</span>
        <span className="col-span-3 border-l pl-2 text-center">IP</span>
        <span className="col-span-3 border-l pl-2 text-center">采集</span>
      </div>
      {/* 二级表头 */}
      <div
        className={`grid ${GRID_COLS} items-center gap-2 border-b px-2 pb-1 text-xs text-muted-foreground`}
      >
        <span>平台</span>
        <span className="text-right">正常</span>
        <span className="text-right">受限</span>
        <span className="text-right">异常</span>
        <span className="border-l pl-2 text-right">可用</span>
        <span className="text-right">受限</span>
        <span className="text-right">异常</span>
        <span className="border-l pl-2 text-right">运行中</span>
        <span className="text-right">任务</span>
        <span className="text-right">活跃率</span>
      </div>
      {rows.map((row) => {
        const plat = row.platform as Platform;
        const { running, total } = crawlMetricsForPlatform(
          tasks,
          plat,
          progressMap,
        );
        const pct =
          total > 0 ? Math.round((running / total) * 100) : null;
        return (
          <div
            key={row.platform}
            className={`grid ${GRID_COLS} items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-muted/50`}
          >
            <span className="font-medium">
              {PLATFORM_LABELS[plat] ?? row.platform}
            </span>
            <span className="text-right tabular-nums font-semibold text-green-600">
              {row.accountNormal}
            </span>
            <span className="text-right tabular-nums font-semibold text-yellow-600">
              {row.accountRestricted}
            </span>
            <span className="text-right tabular-nums font-semibold text-destructive">
              {row.accountError}
            </span>
            <span className="border-l pl-2 text-right tabular-nums font-semibold text-green-600">
              {row.ipAvailable}
            </span>
            <span className="text-right tabular-nums font-semibold text-yellow-600">
              {row.ipRestricted}
            </span>
            <span className="text-right tabular-nums font-semibold text-destructive">
              {row.ipInvalid}
            </span>
            <span className="border-l pl-2 text-right tabular-nums font-semibold text-green-600">
              {running}
            </span>
            <span className="text-right tabular-nums font-semibold">
              {total}
            </span>
            <span className="text-right tabular-nums text-muted-foreground">
              {pct != null ? `${pct}%` : "—"}
            </span>
          </div>
        );
      })}
    </div>
  );
}

const LOG_LEVEL_STYLES: Record<string, string> = {
  info: "bg-blue-500/10 text-blue-600",
  warn: "bg-yellow-500/10 text-yellow-600",
  error: "bg-destructive/10 text-destructive",
};

const LOG_SCOPE_LABEL: Record<string, string> = {
  account: "账号",
  proxy: "代理",
  task: "任务",
  risk: "风控",
  legacy: "历史",
};

function logLevelClass(level: string): string {
  return LOG_LEVEL_STYLES[level.toLowerCase()] ?? "bg-muted text-muted-foreground";
}

function logScopeLabel(scope: string): string {
  return LOG_SCOPE_LABEL[scope] ?? scope;
}

/** 与采集页 `crawl-progress` 载荷一致（camelCase / snake_case）。 */
interface CrawlProgressPayload {
  taskId?: string;
  task_id?: string;
  status: string;
  message: string;
}

export function HomePage() {
  const [stats, setStats] = useState<DashboardStats>(EMPTY_STATS);
  const [tasks, setTasks] = useState<CrawlTask[]>([]);
  const [progressMap, setProgressMap] = useState<
    Record<string, TaskProgress | undefined>
  >({});
  const progressTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const [statsResult, tasksResult] = await Promise.allSettled([
        getDashboardStats(),
        listTasks(null),
      ]);
      if (cancelled) return;
      if (statsResult.status === "fulfilled") {
        setStats(statsResult.value);
      } else {
        console.error("[HomePage] getDashboardStats failed", statsResult.reason);
      }
      if (tasksResult.status === "fulfilled") {
        setTasks(tasksResult.value);
      } else {
        console.error("[HomePage] listTasks failed", tasksResult.reason);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // 与采集页一致：轮询 `get_task_progress`，用于「失败请求 → 展示异常」。
  useEffect(() => {
    async function pollProgress() {
      for (const t of tasks) {
        try {
          const p = await getTaskProgress(t.id);
          if (p.total > 0) {
            setProgressMap((prev) => ({ ...prev, [t.id]: p }));
          }
        } catch {
          // ignore per-task errors
        }
      }
    }
    void pollProgress();
    progressTimerRef.current = setInterval(pollProgress, 3000);
    return () => {
      if (progressTimerRef.current) clearInterval(progressTimerRef.current);
    };
  }, [tasks]);

  // 任务收尾后刷新列表，避免库状态与进度不同步。
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;
    void listen<CrawlProgressPayload>("crawl-progress", (event) => {
      const st = event.payload.status;
      if (st === "done" || st === "error" || st === "risk") {
        void listTasks(null).then((list) => {
          if (!active) return;
          setTasks(list);
        });
      }
    }).then((fn) => {
      if (active) unlisten = fn;
      else fn();
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const { perPlatform, recentLogs } = stats;

  const quickTasks = tasks.filter((t) =>
    (["weibo", "douyin", "xiaohongshu", "kuaishou"] as Platform[]).includes(
      t.platform,
    ),
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden p-4">
      {/* 与数据管理 / 采集管理等页同一套：顶栏下第一行标题 + 说明，`p-4` 对齐侧栏内容区 */}
      <div className="flex shrink-0 flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <h1 className="text-2xl font-bold tracking-tight">系统总览</h1>
          <p className="text-sm text-muted-foreground">
            采集系统运行状态一览
          </p>
        </div>
      </div>

      {/* 健康 / 任务：自然高度；最近日志：flex-1 占满剩余高度，仅此处使用 FloatingScrollArea */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-4">
        <Card className="shrink-0">
          <CardHeader className="flex shrink-0 flex-row flex-wrap items-center justify-between gap-2 space-y-0 pb-3">
            <CardTitle className="flex h-7 min-w-0 items-center gap-2 text-base leading-7">
              <ShieldCheckIcon className="size-4" />
              平台健康概览
            </CardTitle>
          </CardHeader>
          <CardContent>
            <PlatformHealthOverview
              perPlatform={perPlatform}
              tasks={tasks}
              progressMap={progressMap}
            />
          </CardContent>
        </Card>

        <Card className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          <CardHeader className="flex shrink-0 flex-row flex-wrap items-center justify-between gap-2 space-y-0 pb-3">
            <CardTitle className="flex h-7 min-w-0 items-center gap-2 text-base leading-7">
              <ShieldAlertIcon className="size-4" />
              最近日志
            </CardTitle>
            <div className="flex h-7 flex-wrap items-center justify-end gap-2">
              <span className="text-muted-foreground text-xs">
                共 {recentLogs.length} 条 · 应用事件（任务 / 账号 / 代理 / 风控等）
              </span>
            </div>
          </CardHeader>
          <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden pt-0">
            <FloatingScrollArea className="min-h-0 flex-1">
              <div className="overflow-x-auto pr-2">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead className="whitespace-nowrap">时间</TableHead>
                      <TableHead className="whitespace-nowrap">范围</TableHead>
                      <TableHead className="whitespace-nowrap">级别</TableHead>
                      <TableHead className="whitespace-nowrap">动作</TableHead>
                      <TableHead className="min-w-[200px]">消息</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {recentLogs.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={5}
                          className="text-muted-foreground h-24 text-center"
                        >
                          暂无日志。任务调度、账号与代理变更等将写入应用事件表。
                        </TableCell>
                      </TableRow>
                    ) : (
                      recentLogs.map((log, i) => (
                        <TableRow
                          key={`${log.time}-${log.scope}-${log.action}-${i}`}
                        >
                          <TableCell className="whitespace-nowrap font-mono text-xs text-muted-foreground">
                            {log.time}
                          </TableCell>
                          <TableCell className="whitespace-nowrap text-xs">
                            {logScopeLabel(log.scope)}
                          </TableCell>
                          <TableCell className="whitespace-nowrap">
                            <Badge
                              variant="secondary"
                              className={logLevelClass(log.level)}
                            >
                              {log.level.toUpperCase()}
                            </Badge>
                          </TableCell>
                          <TableCell
                            className="max-w-[160px] truncate font-mono text-xs text-muted-foreground"
                            title={log.action || undefined}
                          >
                            {log.action?.trim() ? log.action : "—"}
                          </TableCell>
                          <TableCell
                            className="max-w-[min(48vw,28rem)] truncate text-xs leading-snug"
                            title={log.message}
                          >
                            {log.message}
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>
            </FloatingScrollArea>
          </CardContent>
        </Card>

        <Card className="shrink-0">
          <CardHeader className="flex shrink-0 flex-row flex-wrap items-center justify-between gap-2 space-y-0 pb-3">
            <CardTitle className="flex h-7 min-w-0 items-center gap-2 text-base leading-7">
              <PauseCircleIcon className="size-4" />
              任务运行状态
            </CardTitle>
          </CardHeader>
          <CardContent>
            {quickTasks.length === 0 ? (
              <p className="text-sm text-muted-foreground">暂无任务</p>
            ) : (
              <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
                {quickTasks.map((task) => {
                  const displayStatus = resolveTaskDisplayStatus(
                    task,
                    progressMap[task.id],
                  );
                  return (
                    <div
                      key={task.id}
                      className="flex items-center justify-between rounded-lg border p-3"
                    >
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">
                          {task.name}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          {PLATFORM_LABELS[task.platform]}
                        </p>
                      </div>
                      <Badge variant={TASK_STATUS_BADGE_VARIANT[displayStatus]}>
                        {TASK_STATUS_LABELS[displayStatus]}
                      </Badge>
                    </div>
                  );
                })}
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
