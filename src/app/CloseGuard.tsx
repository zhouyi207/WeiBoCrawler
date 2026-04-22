import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toast } from "sonner";
import { Loader2Icon } from "lucide-react";
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
import { Button } from "@/components/ui/button";
import { PLATFORM_LABELS, type CrawlTask } from "@/features/domain/types";
import {
  allowCloseOnce,
  listTasks,
  pauseTask,
  reconcileStaleRunningTasks,
  startTask,
} from "@/services/tauri/commands";

const CloseGuardContext = createContext<{
  requestClose: () => Promise<void>;
} | null>(null);

export function useCloseGuard(): { requestClose: () => Promise<void> } {
  const ctx = useContext(CloseGuardContext);
  if (!ctx) {
    throw new Error("useCloseGuard must be used within CloseGuardProvider");
  }
  return ctx;
}

export function CloseGuardProvider({ children }: { children: ReactNode }) {
  const [closeDialogOpen, setCloseDialogOpen] = useState(false);
  const [closeRunningTasks, setCloseRunningTasks] = useState<CrawlTask[]>([]);
  const [closePausing, setClosePausing] = useState(false);

  const [staleDialogOpen, setStaleDialogOpen] = useState(false);
  const [staleRunningTasks, setStaleRunningTasks] = useState<CrawlTask[]>([]);
  const [staleAction, setStaleAction] = useState<null | "pause" | "resume">(
    null,
  );

  const finishClose = useCallback(async () => {
    await allowCloseOnce();
    await getCurrentWindow().close();
  }, []);

  const requestClose = useCallback(async () => {
    try {
      const tasks = await listTasks(null);
      const running = tasks.filter((t) => t.status === "running");
      if (running.length === 0) {
        await finishClose();
        return;
      }
      setCloseRunningTasks(running);
      setCloseDialogOpen(true);
    } catch (e) {
      toast.error("无法检查任务状态", {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  }, [finishClose]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen("app-close-requested", () => {
      void requestClose();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [requestClose]);

  /** 冷启动：库中仍为「运行中」的任务（多为上次异常退出遗留） */
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const tasks = await listTasks(null);
        if (cancelled) return;
        const running = tasks.filter((t) => t.status === "running");
        if (running.length === 0) return;
        setStaleRunningTasks(running);
        setStaleDialogOpen(true);
      } catch (e) {
        if (!cancelled) {
          toast.error("无法检查未完成任务状态", {
            description: e instanceof Error ? e.message : String(e),
          });
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  async function handlePauseAndClose() {
    setClosePausing(true);
    try {
      for (const t of closeRunningTasks) {
        await pauseTask(t.id);
      }
    } catch (e) {
      toast.error("暂停任务失败", {
        description: e instanceof Error ? e.message : String(e),
      });
      setClosePausing(false);
      return;
    }
    setClosePausing(false);
    setCloseDialogOpen(false);
    setCloseRunningTasks([]);
    await finishClose();
  }

  async function handleStaleReconcilePause() {
    setStaleAction("pause");
    try {
      await reconcileStaleRunningTasks();
      toast.success("已将相关任务设为暂停，并修复队列中卡住项");
      setStaleDialogOpen(false);
      setStaleRunningTasks([]);
    } catch (e) {
      toast.error("处理失败", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setStaleAction(null);
    }
  }

  async function handleStaleResume() {
    setStaleAction("resume");
    try {
      for (const t of staleRunningTasks) {
        await startTask(t.id);
      }
      toast.success("已重新启动采集调度");
      setStaleDialogOpen(false);
      setStaleRunningTasks([]);
    } catch (e) {
      toast.error("恢复采集失败", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setStaleAction(null);
    }
  }

  return (
    <CloseGuardContext.Provider value={{ requestClose }}>
      {children}
      <AlertDialog
        open={closeDialogOpen}
        onOpenChange={(open) => {
          if (!open && closePausing) return;
          setCloseDialogOpen(open);
          if (!open) setCloseRunningTasks([]);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>仍有采集任务在运行</AlertDialogTitle>
            <AlertDialogDescription asChild>
              <div className="space-y-3 text-left">
                <p>
                  关闭窗口前可先暂停这些任务，或取消关闭继续操作。
                </p>
                <ul className="max-h-40 list-inside list-disc overflow-y-auto text-sm text-muted-foreground">
                  {closeRunningTasks.map((t) => (
                    <li key={t.id}>
                      <span className="text-foreground">{t.name}</span>
                      <span className="text-muted-foreground">
                        {" "}
                        （{PLATFORM_LABELS[t.platform]}）
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={closePausing}>取消关闭</AlertDialogCancel>
            <AlertDialogAction
              disabled={closePausing}
              onClick={(e) => {
                e.preventDefault();
                void handlePauseAndClose();
              }}
            >
              {closePausing ? (
                <>
                  <Loader2Icon className="mr-2 size-4 animate-spin" />
                  正在暂停…
                </>
              ) : (
                "暂停并关闭"
              )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog
        open={staleDialogOpen}
        onOpenChange={(open) => {
          if (!open && staleAction !== null) return;
          setStaleDialogOpen(open);
          if (!open) setStaleRunningTasks([]);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>检测到未正常结束的运行中任务</AlertDialogTitle>
            <AlertDialogDescription asChild>
              <div className="space-y-3 text-left">
                <p>
                  上次可能异常退出：下列任务在数据中仍为「运行中」，但当前没有活跃的采集调度。可将它们设为暂停并修复队列，或重新启动采集。
                </p>
                <ul className="max-h-40 list-inside list-disc overflow-y-auto text-sm text-muted-foreground">
                  {staleRunningTasks.map((t) => (
                    <li key={t.id}>
                      <span className="text-foreground">{t.name}</span>
                      <span className="text-muted-foreground">
                        {" "}
                        （{PLATFORM_LABELS[t.platform]}）
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter className="flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <AlertDialogCancel
              className="mt-0 sm:mr-auto"
              disabled={staleAction !== null}
            >
              稍后处理
            </AlertDialogCancel>
            <div className="flex w-full flex-col gap-2 sm:w-auto sm:flex-row sm:justify-end">
              <Button
                type="button"
                variant="outline"
                disabled={staleAction !== null}
                className="w-full sm:w-auto"
                onClick={() => void handleStaleReconcilePause()}
              >
                {staleAction === "pause" ? (
                  <>
                    <Loader2Icon className="mr-2 size-4 animate-spin" />
                    处理中…
                  </>
                ) : (
                  "设为暂停并修复队列"
                )}
              </Button>
              <Button
                type="button"
                disabled={staleAction !== null}
                className="w-full sm:w-auto"
                onClick={() => void handleStaleResume()}
              >
                {staleAction === "resume" ? (
                  <>
                    <Loader2Icon className="mr-2 size-4 animate-spin" />
                    启动中…
                  </>
                ) : (
                  "恢复采集"
                )}
              </Button>
            </div>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </CloseGuardContext.Provider>
  );
}
