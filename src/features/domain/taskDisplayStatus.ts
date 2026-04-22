import type { CrawlTask, TaskStatus } from "./types";

/** 与 `get_task_progress` 返回字段一致，供展示层聚合。 */
export interface TaskProgressCounts {
  pending: number;
  running: number;
  done: number;
  failed: number;
  total: number;
}

/**
 * 采集列表与首页「任务运行状态」共用。
 *
 * 队列中存在 `failed` 请求时，一律按「异常」展示；重试/清空失败并恢复为 0 后，
 * 才与库里的 `running` / `completed` 等状态一致。用户主动「已暂停」优先于失败展示。
 */
export function resolveTaskDisplayStatus(
  task: CrawlTask,
  progress: TaskProgressCounts | null | undefined,
): TaskStatus {
  if (task.status === "paused") return "paused";
  if (progress && progress.total > 0 && progress.failed > 0) return "error";
  return task.status;
}

export const TASK_STATUS_BADGE_VARIANT: Record<
  TaskStatus,
  "default" | "secondary" | "destructive" | "outline"
> = {
  running: "default",
  paused: "secondary",
  completed: "outline",
  error: "destructive",
};
