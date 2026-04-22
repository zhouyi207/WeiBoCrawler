import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import {
  AlertCircleIcon,
  Loader2Icon,
  PlusIcon,
  PlayIcon,
  PauseIcon,
  Trash2Icon,
  PencilIcon,
  RotateCwIcon,
  RefreshCwIcon,
  Settings2Icon,
  EraserIcon,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
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
  TASK_TYPE_LABELS,
  TASK_STATUS_LABELS,
  type Platform,
  type CrawlTask,
} from "@/features/domain/types";
import {
  resolveTaskDisplayStatus,
  TASK_STATUS_BADGE_VARIANT,
} from "@/features/domain/taskDisplayStatus";
import {
  deleteTask,
  listTasks,
  pauseTask,
  startTask,
  restartTask,
  getTaskProgress,
  retryFailedRequests,
  type TaskProgress,
} from "@/services/tauri/commands";
import { CreateTaskModal } from "./CreateTaskModal";
import { CrawlBackoffSettingsDialog } from "./CrawlBackoffSettingsDialog";
import {
  loadPersistedTaskLogs,
  persistTaskLogs,
  removeTaskLogsFromStorage,
  MAX_LOG_LINES_PER_TASK,
} from "./crawlTaskLogsStorage";

const STRATEGY_LABELS: Record<string, string> = {
  round_robin: "轮询",
  random: "随机",
};

/** 与后端 `CrawlProgressEvent`（camelCase）一致 */
interface CrawlProgressPayload {
  taskId?: string;
  task_id?: string;
  status: string;
  message: string;
}

function taskIdFromPayload(p: CrawlProgressPayload): string {
  return p.taskId ?? p.task_id ?? "";
}

export function CrawlPage() {
  const [tasks, setTasks] = useState<CrawlTask[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [createOpen, setCreateOpen] = useState(false);
  const [backoffSettingsOpen, setBackoffSettingsOpen] = useState(false);
  const [activePlatform, setActivePlatform] = useState<Platform>("weibo");
  const [editingTask, setEditingTask] = useState<CrawlTask | null>(null);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());
  const [taskLogs, setTaskLogs] = useState<Record<string, string[]>>(
    loadPersistedTaskLogs
  );
  const [busyId, setBusyId] = useState<string | null>(null);
  const [progressMap, setProgressMap] = useState<Record<string, TaskProgress>>({});
  const progressTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // 二次确认对话框（删除 / 重新采集）：替换原先的 window.confirm。
  const [confirmState, setConfirmState] = useState<{
    title: string;
    description: string;
    confirmLabel: string;
    destructive?: boolean;
    onConfirm: () => void | Promise<void>;
  } | null>(null);

  const loadTasks = useCallback(async () => {
    setLoadError("");
    try {
      const list = await listTasks(null);
      setTasks(list);
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const sortedTasks = useMemo(
    () =>
      [...tasks].sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [tasks]
  );

  const hasAnyTaskLogs = useMemo(
    () => Object.values(taskLogs).some((lines) => lines.length > 0),
    [taskLogs]
  );

  useEffect(() => {
    void loadTasks();
  }, [loadTasks]);

  /** 与 `listen('crawl-progress')` 同步：内存 + localStorage */
  useEffect(() => {
    persistTaskLogs(taskLogs);
  }, [taskLogs]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    void listen<CrawlProgressPayload>("crawl-progress", (event) => {
      const p = event.payload;
      const id = taskIdFromPayload(p);
      if (!id) return;
      const ts = new Date().toLocaleTimeString();
      const line = `[${ts}] [${p.status}] ${p.message}`;
      setTaskLogs((prev) => {
        const cur = prev[id] ?? [];
        const next = [...cur, line].slice(-MAX_LOG_LINES_PER_TASK);
        return { ...prev, [id]: next };
      });

      if (p.status === "done" || p.status === "error") {
        void loadTasks();
      }
      if (p.status === "risk") {
        toast.warning("风控提示", {
          description: p.message,
          duration: 8000,
        });
        void loadTasks();
      }
    }).then((fn) => {
      if (active) unlisten = fn;
      else fn();
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [loadTasks]);

  // Poll progress for all tasks periodically.
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

  async function handleRetryFailed(task: CrawlTask) {
    if (busyId) return;
    setBusyId(task.id);
    try {
      const n = await retryFailedRequests(task.id);
      if (n > 0) {
        setExpandedIds((prev) => new Set(prev).add(task.id));
        toast.success(`已重排 ${n} 条失败请求`, {
          description: `任务「${task.name}」`,
        });
      } else {
        toast.info("没有需要重试的失败请求", {
          description: `任务「${task.name}」`,
        });
      }
      await loadTasks();
    } catch (e) {
      toast.error("重试失败请求时出错", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setBusyId(null);
    }
  }

  function handleRestart(task: CrawlTask) {
    setConfirmState({
      title: "重新采集任务",
      description: `确定重新采集任务「${task.name}」？将清除所有已有请求并重新开始。`,
      confirmLabel: "重新采集",
      destructive: true,
      onConfirm: async () => {
        if (busyId) return;
        setBusyId(task.id);
        try {
          await restartTask(task.id);
          setExpandedIds((prev) => new Set(prev).add(task.id));
          toast.success(`任务「${task.name}」已重新启动`);
          await loadTasks();
        } catch (e) {
          toast.error("重新采集失败", {
            description: e instanceof Error ? e.message : String(e),
          });
        } finally {
          setBusyId(null);
        }
      },
    });
  }

  function toggleExpanded(id: string) {
    setExpandedIds((prev) => {
      const n = new Set(prev);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  }

  async function handleToggleRun(task: CrawlTask) {
    if (busyId) return;
    setBusyId(task.id);
    const wasRunning = task.status === "running";
    try {
      if (wasRunning) {
        await pauseTask(task.id);
        toast.success(`任务「${task.name}」已暂停`);
      } else {
        await startTask(task.id);
        setExpandedIds((prev) => new Set(prev).add(task.id));
        toast.success(`任务「${task.name}」已开始采集`);
      }
      await loadTasks();
    } catch (e) {
      toast.error(wasRunning ? "暂停任务失败" : "启动任务失败", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setBusyId(null);
    }
  }

  function handleDelete(task: CrawlTask) {
    setConfirmState({
      title: "删除采集任务",
      description: `确定删除任务「${task.name}」？此操作不可恢复，已有的采集记录会一并清除。`,
      confirmLabel: "删除",
      destructive: true,
      onConfirm: async () => {
        if (busyId) return;
        setBusyId(task.id);
        try {
          await deleteTask(task.id);
          removeTaskLogsFromStorage(task.id);
          setTaskLogs((prev) => {
            const { [task.id]: _, ...rest } = prev;
            return rest;
          });
          setExpandedIds((prev) => {
            const n = new Set(prev);
            n.delete(task.id);
            return n;
          });
          toast.success(`任务「${task.name}」已删除`);
          await loadTasks();
        } catch (e) {
          toast.error("删除任务失败", {
            description: e instanceof Error ? e.message : String(e),
          });
        } finally {
          setBusyId(null);
        }
      },
    });
  }

  function openCreate(forPlatform: Platform) {
    setActivePlatform(forPlatform);
    setEditingTask(null);
    setCreateOpen(true);
  }

  function openEdit(task: CrawlTask) {
    setActivePlatform(task.platform);
    setEditingTask(task);
    setCreateOpen(true);
  }

  function clearLogsForTask(taskId: string) {
    setTaskLogs((prev) => {
      if (!(taskId in prev)) return prev;
      const next = { ...prev };
      delete next[taskId];
      return next;
    });
  }

  function clearLogsForPlatformTab(platform: Platform) {
    const ids = sortedTasks
      .filter((t) => t.platform === platform)
      .map((t) => t.id);
    if (ids.length === 0) return;
    setTaskLogs((prev) => {
      const next = { ...prev };
      for (const id of ids) delete next[id];
      return next;
    });
    toast.success("已清空当前平台任务的本地日志");
  }

  function confirmClearAllTaskLogs() {
    setConfirmState({
      title: "清空全部任务日志",
      description:
        "将删除所有任务在本地持久保存的采集进度日志，且不可恢复。确定继续？",
      confirmLabel: "清空",
      destructive: true,
      onConfirm: () => {
        setTaskLogs({});
        toast.success("已清空全部任务日志");
      },
    });
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden p-4">
      <div className="flex shrink-0 flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <h1 className="text-2xl font-bold tracking-tight">采集管理</h1>
          <p className="text-sm text-muted-foreground">
            按平台分类查看任务；启动采集后会自动展开日志（本地持久化）。点击行可展开/收起。
          </p>
        </div>
        <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="gap-1.5"
            disabled={!hasAnyTaskLogs}
            onClick={() => confirmClearAllTaskLogs()}
            title="清空所有任务在本地保存的采集日志"
          >
            <EraserIcon className="size-4" />
            清空全部日志
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="gap-1.5"
            onClick={() => setBackoffSettingsOpen(true)}
            title="各平台 Worker 连续失败后的熔断退避秒数"
          >
            <Settings2Icon className="size-4" />
            采集熔断退避（秒）
          </Button>
        </div>
      </div>

      {loadError ? (
        <Alert variant="destructive" className="shrink-0">
          <AlertCircleIcon />
          <AlertTitle>加载任务列表失败</AlertTitle>
          <AlertDescription>{loadError}</AlertDescription>
        </Alert>
      ) : null}

      <Tabs
        value={activePlatform}
        onValueChange={(v) => setActivePlatform(v as Platform)}
        className="min-h-0 flex flex-1 flex-col gap-2 overflow-hidden"
      >
        <TabsList className="shrink-0">
          {PLATFORMS.map((p) => {
            const count = sortedTasks.filter((t) => t.platform === p).length;
            return (
              <TabsTrigger key={p} value={p}>
                {PLATFORM_LABELS[p]}
                <Badge variant="secondary" className="ml-1.5 text-[10px]">
                  {count}
                </Badge>
              </TabsTrigger>
            );
          })}
        </TabsList>

        {PLATFORMS.map((p) => {
          const platformTasks = sortedTasks.filter((t) => t.platform === p);
          const hasLogsOnThisTab = platformTasks.some(
            (t) => (taskLogs[t.id] ?? []).length > 0
          );
          return (
            <TabsContent
              key={p}
              value={p}
              className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
            >
              <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
                <CardHeader className="flex shrink-0 flex-row flex-wrap items-center justify-between gap-2 space-y-0 pb-3">
                  <CardTitle className="flex h-7 min-w-0 items-center text-base leading-7">
                    {PLATFORM_LABELS[p]} 采集任务
                  </CardTitle>
                  <div className="flex flex-wrap items-center justify-end gap-2">
                    <span className="text-muted-foreground text-xs">
                      共 {platformTasks.length} 条
                    </span>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="shrink-0 gap-1.5"
                      disabled={!hasLogsOnThisTab}
                      onClick={() => clearLogsForPlatformTab(p)}
                      title="清空当前平台下所有任务的本地采集日志"
                    >
                      <EraserIcon className="size-4" />
                      清空本页日志
                    </Button>
                    <Button
                      type="button"
                      size="sm"
                      className="shrink-0 gap-1.5"
                      onClick={() => openCreate(p)}
                    >
                      <PlusIcon className="size-4" />
                      新建采集任务
                    </Button>
                  </div>
                </CardHeader>
                <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden pt-0">
                  <FloatingScrollArea className="min-h-0 flex-1">
                    <div className="overflow-x-auto pr-2">
                      <Table className="table-fixed">
                        <colgroup>
                          <col />
                          <col className="w-[5.5rem]" />
                          <col className="w-[5.5rem]" />
                          <col className="w-[4.5rem]" />
                          <col className="w-[4rem]" />
                          <col className="w-[4rem]" />
                          <col className="w-[5.5rem]" />
                          <col className="w-[7rem]" />
                        </colgroup>
                        <TableHeader>
                          <TableRow>
                            <TableHead className="whitespace-nowrap">
                              任务名称
                            </TableHead>
                            <TableHead className="whitespace-nowrap">类型</TableHead>
                            <TableHead className="whitespace-nowrap">采集策略</TableHead>
                            <TableHead className="whitespace-nowrap text-center">
                              速率
                            </TableHead>
                            <TableHead className="whitespace-nowrap text-center">
                              账号池
                            </TableHead>
                            <TableHead className="whitespace-nowrap text-center">
                              IP池
                            </TableHead>
                            <TableHead
                              className="whitespace-nowrap text-center"
                              title="账号数 × 代理数 = 最大并发 worker 数"
                            >
                              并发
                            </TableHead>
                            <TableHead className="whitespace-nowrap">状态</TableHead>
                            <TableHead className="whitespace-nowrap text-right">
                              操作
                            </TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          {loading && platformTasks.length === 0 ? (
                            <TableRow>
                              <TableCell
                                colSpan={9}
                                className="text-muted-foreground h-24 text-center"
                              >
                                <Loader2Icon className="mx-auto size-6 animate-spin opacity-70" />
                              </TableCell>
                            </TableRow>
                          ) : platformTasks.length === 0 ? (
                            <TableRow>
                              <TableCell
                                colSpan={9}
                                className="text-muted-foreground h-24 text-center"
                              >
                                暂无 {PLATFORM_LABELS[p]} 采集任务，点击「新建采集任务」。
                              </TableCell>
                            </TableRow>
                          ) : (
                            platformTasks.map((task) => {
                              const expanded = expandedIds.has(task.id);
                              const logs = taskLogs[task.id] ?? [];
                              const displayStatus = resolveTaskDisplayStatus(
                                task,
                                progressMap[task.id],
                              );
                              return (
                                <Fragment key={task.id}>
                                  <TableRow
                                    data-state={expanded ? "open" : undefined}
                                    role="button"
                                    tabIndex={0}
                                    aria-expanded={expanded}
                                    title={expanded ? "点击收起日志" : "点击展开日志"}
                                    className="group cursor-pointer"
                                    onClick={() => toggleExpanded(task.id)}
                                    onKeyDown={(e) => {
                                      if (e.key === "Enter" || e.key === " ") {
                                        e.preventDefault();
                                        toggleExpanded(task.id);
                                      }
                                    }}
                                  >
                                    <TableCell className="max-w-0 font-medium">
                                      <span className="block truncate" title={task.name}>
                                        {task.name}
                                      </span>
                                    </TableCell>
                                    <TableCell>
                                      {TASK_TYPE_LABELS[task.type]}
                                    </TableCell>
                                    <TableCell>
                                      {STRATEGY_LABELS[task.strategy] ??
                                        task.strategy}
                                    </TableCell>
                                    <TableCell className="text-center">
                                      {task.rateLimit}/min
                                    </TableCell>
                                    <TableCell className="text-center">
                                      {task.boundAccountIds?.length ??
                                        task.accountPoolSize}
                                    </TableCell>
                                    <TableCell className="text-center">
                                      {task.boundProxyIds?.length ?? task.ipPoolSize}
                                    </TableCell>
                                    <TableCell className="text-center">
                                      {(task.boundAccountIds?.length ??
                                        task.accountPoolSize) *
                                        Math.max(
                                          task.boundProxyIds?.length ??
                                            task.ipPoolSize,
                                          1,
                                        )}
                                    </TableCell>
                                    <TableCell>
                                      <Badge
                                        variant={TASK_STATUS_BADGE_VARIANT[displayStatus]}
                                      >
                                        {TASK_STATUS_LABELS[displayStatus]}
                                      </Badge>
                                    </TableCell>
                                    <TableCell className="text-right">
                                      <div className="flex items-center justify-end gap-0.5">
                                        <Button
                                          variant="ghost"
                                          size="icon-xs"
                                          disabled={busyId === task.id}
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            openEdit(task);
                                          }}
                                          title="编辑"
                                        >
                                          <PencilIcon />
                                        </Button>
                                        {task.status === "running" ? (
                                          <Button
                                            variant="ghost"
                                            size="icon-xs"
                                            disabled={busyId === task.id}
                                            onClick={(e) => {
                                              e.stopPropagation();
                                              void handleToggleRun(task);
                                            }}
                                            title="暂停"
                                          >
                                            <PauseIcon />
                                          </Button>
                                        ) : task.status === "completed" ? (
                                          <Button
                                            variant="ghost"
                                            size="icon-xs"
                                            disabled={busyId === task.id}
                                            onClick={(e) => {
                                              e.stopPropagation();
                                              void handleRestart(task);
                                            }}
                                            title="重新采集"
                                          >
                                            <RotateCwIcon />
                                          </Button>
                                        ) : (
                                          <Button
                                            variant="ghost"
                                            size="icon-xs"
                                            disabled={busyId === task.id}
                                            onClick={(e) => {
                                              e.stopPropagation();
                                              void handleToggleRun(task);
                                            }}
                                            title="启动"
                                          >
                                            <PlayIcon />
                                          </Button>
                                        )}
                                        <Button
                                          variant="ghost"
                                          size="icon-xs"
                                          disabled={busyId === task.id}
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            void handleDelete(task);
                                          }}
                                          title="删除"
                                        >
                                          <Trash2Icon />
                                        </Button>
                                      </div>
                                    </TableCell>
                                  </TableRow>
                                  {expanded && (
                                    <TableRow className="bg-muted/20 hover:bg-muted/25">
                                      <TableCell
                                        colSpan={9}
                                        className="whitespace-normal p-3 align-top"
                                      >
                                        <div className="min-w-0 space-y-2">
                                          {(() => {
                                            const prog = progressMap[task.id];
                                            const showRestartRetry =
                                              !!prog &&
                                              prog.total > 0 &&
                                              prog.failed > 0 &&
                                              task.status !== "running";
                                            const hasProgress =
                                              !!prog && prog.total > 0;

                                            const actionButtons = (
                                              <>
                                                <Button
                                                  variant="outline"
                                                  size="sm"
                                                  className="h-6 gap-1 px-2 text-xs"
                                                  disabled={
                                                    busyId === task.id ||
                                                    logs.length === 0
                                                  }
                                                  onClick={(e) => {
                                                    e.stopPropagation();
                                                    clearLogsForTask(task.id);
                                                    toast.message(
                                                      "已清空该任务的本地日志",
                                                    );
                                                  }}
                                                >
                                                  <EraserIcon className="size-3" />
                                                  清空日志
                                                </Button>
                                                {showRestartRetry && (
                                                  <>
                                                    <Button
                                                      variant="outline"
                                                      size="sm"
                                                      className="h-6 gap-1 px-2 text-xs"
                                                      disabled={
                                                        busyId === task.id
                                                      }
                                                      onClick={(e) => {
                                                        e.stopPropagation();
                                                        void handleRestart(task);
                                                      }}
                                                    >
                                                      <RotateCwIcon className="size-3" />
                                                      重新采集
                                                    </Button>
                                                    <Button
                                                      variant="outline"
                                                      size="sm"
                                                      className="h-6 gap-1 px-2 text-xs"
                                                      disabled={
                                                        busyId === task.id
                                                      }
                                                      onClick={(e) => {
                                                        e.stopPropagation();
                                                        void handleRetryFailed(
                                                          task,
                                                        );
                                                      }}
                                                    >
                                                      <RefreshCwIcon className="size-3" />
                                                      重试失败请求
                                                    </Button>
                                                  </>
                                                )}
                                              </>
                                            );

                                            if (!hasProgress) {
                                              return (
                                                <div className="flex flex-wrap items-center justify-end gap-2">
                                                  {actionButtons}
                                                </div>
                                              );
                                            }

                                            const pct = Math.round(
                                              (prog.done / prog.total) * 100,
                                            );
                                            return (
                                              <div className="space-y-1">
                                                <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
                                                  <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                                                    <span>
                                                      进度 {prog.done}/
                                                      {prog.total} ({pct}%)
                                                    </span>
                                                    {prog.failed > 0 && (
                                                      <span className="font-medium text-destructive">
                                                        失败 {prog.failed}
                                                      </span>
                                                    )}
                                                    {prog.running > 0 && (
                                                      <span className="text-primary">
                                                        执行中 {prog.running}
                                                      </span>
                                                    )}
                                                    {prog.pending > 0 && (
                                                      <span>等待 {prog.pending}</span>
                                                    )}
                                                  </div>
                                                  <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
                                                    {actionButtons}
                                                  </div>
                                                </div>
                                                <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
                                                  <div
                                                    className="h-full rounded-full bg-primary transition-all"
                                                    style={{
                                                      width: `${pct}%`,
                                                    }}
                                                  />
                                                </div>
                                              </div>
                                            );
                                          })()}
                                          {logs.length === 0 ? (
                                            <p className="text-xs text-muted-foreground">
                                              暂无日志。启动任务后将在此显示每条采集进度与入库结果。
                                            </p>
                                          ) : (
                                            <pre className="max-h-56 max-w-full overflow-x-auto overflow-y-auto rounded-md border bg-background/80 p-2 font-mono text-[11px] leading-relaxed break-words whitespace-pre-wrap">
                                              {logs.join("\n")}
                                            </pre>
                                          )}
                                        </div>
                                      </TableCell>
                                    </TableRow>
                                  )}
                                </Fragment>
                              );
                            })
                          )}
                        </TableBody>
                      </Table>
                    </div>
                  </FloatingScrollArea>
                </CardContent>
              </Card>
            </TabsContent>
          );
        })}
      </Tabs>

      <CreateTaskModal
        open={createOpen}
        onOpenChange={(o) => {
          setCreateOpen(o);
          if (!o) setEditingTask(null);
        }}
        platform={activePlatform}
        editingTask={editingTask}
        onCreated={loadTasks}
      />

      <CrawlBackoffSettingsDialog
        open={backoffSettingsOpen}
        onOpenChange={setBackoffSettingsOpen}
      />

      <AlertDialog
        open={confirmState !== null}
        onOpenChange={(o) => {
          if (!o) setConfirmState(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{confirmState?.title}</AlertDialogTitle>
            <AlertDialogDescription>
              {confirmState?.description}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              variant={confirmState?.destructive ? "destructive" : "default"}
              onClick={() => {
                const action = confirmState?.onConfirm;
                setConfirmState(null);
                void action?.();
              }}
            >
              {confirmState?.confirmLabel ?? "确认"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
