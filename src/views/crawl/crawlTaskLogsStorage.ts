/** localStorage 中按任务 id 持久化采集日志（与后端 `crawl-progress` 事件对应） */

const STORAGE_KEY = "ysscrawler.crawlTaskLogs.v1";

/** 单任务最多保留行数（防止占满配额） */
export const MAX_LOG_LINES_PER_TASK = 2000;

export function loadPersistedTaskLogs(): Record<string, string[]> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as unknown;
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }
    const out: Record<string, string[]> = {};
    for (const [k, v] of Object.entries(parsed)) {
      if (Array.isArray(v) && v.every((x) => typeof x === "string")) {
        out[k] = v.slice(-MAX_LOG_LINES_PER_TASK);
      }
    }
    return out;
  } catch {
    return {};
  }
}

export function persistTaskLogs(logs: Record<string, string[]>): void {
  try {
    const trimmed: Record<string, string[]> = {};
    for (const [k, v] of Object.entries(logs)) {
      trimmed[k] = v.slice(-MAX_LOG_LINES_PER_TASK);
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify(trimmed));
  } catch {
    // 配额或其它错误：忽略，内存中仍保留当前会话日志
  }
}

export function removeTaskLogsFromStorage(taskId: string): void {
  const cur = loadPersistedTaskLogs();
  if (!(taskId in cur)) return;
  delete cur[taskId];
  persistTaskLogs(cur);
}
