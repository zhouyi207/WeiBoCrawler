import { useEffect, useState } from "react";
import {
  ActivityIcon,
  PauseCircleIcon,
  ShieldAlertIcon,
  ShieldCheckIcon,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Separator } from "@/components/ui/separator";
import { FloatingScrollArea } from "@/app/ui/FloatingScrollArea";
import {
  PLATFORM_LABELS,
  PLATFORMS,
  type CrawlTask,
  type DashboardStats,
  type Platform,
  type PlatformOverview,
} from "@/features/domain/types";
import { getDashboardStats, listTasks } from "@/services/tauri/commands";

const EMPTY_STATS: DashboardStats = {
  taskStats: { running: 0, paused: 0, error: 0, total: 0 },
  accountStats: { normal: 0, restricted: 0, error: 0, total: 0 },
  ipStats: { available: 0, restricted: 0, invalid: 0, total: 0 },
  perPlatform: [],
  recentLogs: [],
};

function PlatformRow({
  platform,
  tasks,
}: {
  platform: Platform;
  tasks: CrawlTask[];
}) {
  const platformTasks = tasks.filter((t) => t.platform === platform);
  const running = platformTasks.filter((t) => t.status === "running").length;
  const total = platformTasks.length;
  const pct = total > 0 ? (running / total) * 100 : 0;

  return (
    <div className="flex items-center gap-4">
      <span className="w-16 shrink-0 text-sm font-medium">
        {PLATFORM_LABELS[platform]}
      </span>
      <Progress value={pct} className="flex-1" />
      <span className="w-24 shrink-0 text-right text-xs text-muted-foreground">
        {running}/{total} 运行中
      </span>
    </div>
  );
}

/**
 * 首页「平台健康概览」表格。把账号 × IP 的二维健康数据融合到一行：
 *
 * ```
 *                  ┌─── 账号 ───┐  ┌─── IP ───┐
 *  平台    正常  受限  异常  | 可用  受限  异常
 *  微博     3    0     1    |  4    1     0
 *  抖音     2    1     0    |  3    2     0
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
}: {
  perPlatform: PlatformOverview[];
}) {
  const byPlatform = new Map(perPlatform.map((p) => [p.platform, p] as const));
  const rows = PLATFORMS.map((p) => byPlatform.get(p)).filter(
    (s): s is PlatformOverview => !!s && s.accountTotal > 0,
  );

  if (rows.length === 0) {
    return <p className="text-sm text-muted-foreground">暂无账号</p>;
  }

  // 7 列 grid：平台 | 账号3列 | 分隔线 | IP3列。
  // 用同一份 grid-template 给二级表头、一级表头、数据行三层共享，保证列对齐。
  // 中间那条竖向分隔线占独立的 1px 列，跨所有行（用 border-l 贴在 IP 块第一列上）。
  const GRID_COLS = "grid-cols-[1fr_3.25rem_3.25rem_3.25rem_3.25rem_3.25rem_3.25rem]";

  return (
    <div className="space-y-1">
      {/* 一级表头：账号 / IP 分组 */}
      <div
        className={`grid ${GRID_COLS} items-center gap-2 px-2 pb-1 text-xs font-medium text-muted-foreground`}
      >
        <span />
        <span className="col-span-3 text-center">账号</span>
        <span className="col-span-3 border-l pl-2 text-center">IP</span>
      </div>
      {/* 二级表头：短词 */}
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
      </div>
      {rows.map((row) => (
        <div
          key={row.platform}
          className={`grid ${GRID_COLS} items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-muted/50`}
        >
          <span className="font-medium">
            {PLATFORM_LABELS[row.platform as Platform] ?? row.platform}
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
        </div>
      ))}
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

export function HomePage() {
  const [stats, setStats] = useState<DashboardStats>(EMPTY_STATS);
  const [tasks, setTasks] = useState<CrawlTask[]>([]);

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

  const { perPlatform, recentLogs } = stats;

  const quickTasks = tasks.filter((t) =>
    (["weibo", "douyin", "xiaohongshu", "kuaishou"] as Platform[]).includes(
      t.platform,
    ),
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <FloatingScrollArea>
        <div className="space-y-6 p-6">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">系统总览</h1>
            <p className="text-sm text-muted-foreground">
              采集系统运行状态一览
            </p>
          </div>

          {/* 平台健康概览：账号 × IP 二维融合表 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <ShieldCheckIcon className="size-4" />
                平台健康概览
              </CardTitle>
            </CardHeader>
            <CardContent>
              <PlatformHealthOverview perPlatform={perPlatform} />
            </CardContent>
          </Card>

          {/* Platform overview + Recent logs */}
          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-base">
                  <ActivityIcon className="size-4" />
                  平台采集状态
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                {PLATFORMS.map((p) => (
                  <PlatformRow key={p} platform={p} tasks={tasks} />
                ))}
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-base">
                  <ShieldAlertIcon className="size-4" />
                  最近日志
                </CardTitle>
              </CardHeader>
              <CardContent className="min-h-0">
                {recentLogs.length === 0 ? (
                  <p className="text-sm text-muted-foreground">暂无日志</p>
                ) : (
                  <FloatingScrollArea className="h-72 max-h-72 shrink-0 flex-none">
                    <div className="space-y-3 pr-1 pb-1">
                      {recentLogs.map((log, i) => (
                        <div key={`${log.time}-${log.scope}-${log.action}-${i}`}>
                          <div className="flex flex-wrap items-start gap-2 sm:gap-3">
                            <span className="mt-0.5 shrink-0 text-xs tabular-nums text-muted-foreground">
                              {log.time}
                            </span>
                            <Badge
                              variant="outline"
                              className="shrink-0 text-[10px] font-normal"
                            >
                              {logScopeLabel(log.scope)}
                            </Badge>
                            <Badge
                              variant="secondary"
                              className={logLevelClass(log.level)}
                            >
                              {log.level.toUpperCase()}
                            </Badge>
                            <span className="min-w-0 flex-1 text-sm leading-snug">
                              {log.message}
                            </span>
                          </div>
                          {i < recentLogs.length - 1 && (
                            <Separator className="mt-3" />
                          )}
                        </div>
                      ))}
                    </div>
                  </FloatingScrollArea>
                )}
              </CardContent>
            </Card>
          </div>

          {/* Quick task status list */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <PauseCircleIcon className="size-4" />
                任务运行状态
              </CardTitle>
            </CardHeader>
            <CardContent>
              {quickTasks.length === 0 ? (
                <p className="text-sm text-muted-foreground">暂无任务</p>
              ) : (
                <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
                  {quickTasks.map((task) => (
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
                      <Badge
                        variant={
                          task.status === "running"
                            ? "default"
                            : task.status === "paused"
                              ? "secondary"
                              : "destructive"
                        }
                      >
                        {task.status === "running"
                          ? "运行中"
                          : task.status === "paused"
                            ? "已暂停"
                            : "异常"}
                      </Badge>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </FloatingScrollArea>
    </div>
  );
}
