import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountLogEntry,
  CrawledRecord,
  CrawlTask,
  DashboardStats,
  GenerateQrResponse,
  ProxyGlobalRow,
  ProxyHealthBrief,
  ProxyIp,
  ProxyLogEntry,
  ProxyPlatformRow,
  ProxyProbeSettings,
  WeiboQrPollResponse,
  WorkerBackoffSettings,
} from "@/features/domain/types";

/**
 * 首页一次性返回任务 / 账号 / IP 三组聚合统计 + 最近日志。
 * 后端在 `command::home::get_dashboard_stats` 内复用 `list_proxies_health`
 * 等服务，前端无需再分别调用。
 */
export function getDashboardStats(): Promise<DashboardStats> {
  return invoke<DashboardStats>("get_dashboard_stats");
}

export function listAccounts(platform?: string | null): Promise<Account[]> {
  return invoke<Account[]>("list_accounts", { platform: platform ?? null });
}

/** 删除账号。后端会同时清掉运行时持有的扫码会话。 */
export function deleteAccount(id: string): Promise<void> {
  return invoke<void>("delete_account", { id });
}

/**
 * 拉取某账号最近 N 条失败事件（来自 `account_failure_events`）。
 * 默认 100 条，后端上限 200。结构与 `listProxyLogs` 对称。
 */
export function listAccountLogs(
  id: string,
  limit?: number,
): Promise<AccountLogEntry[]> {
  return invoke<AccountLogEntry[]>("list_account_logs", {
    id,
    limit: limit ?? null,
  });
}

export function listProxies(): Promise<ProxyIp[]> {
  return invoke<ProxyIp[]>("list_proxies");
}

/** 新增代理；`proxyType` 仅支持 `"HTTP" | "SOCKS5"`，`Direct` 由系统内置行 `local-direct` 占位。 */
export function addProxy(payload: {
  address: string;
  proxyType: "HTTP" | "SOCKS5";
  remark?: string | null;
}): Promise<ProxyIp> {
  return invoke<ProxyIp>("add_proxy", {
    address: payload.address,
    proxyType: payload.proxyType,
    remark: payload.remark ?? null,
  });
}

export function deleteProxy(id: string): Promise<void> {
  return invoke<void>("delete_proxy", { id });
}

/** 编辑代理：仅 address / proxyType / remark 可改；系统行后端会拒绝。 */
export function updateProxy(payload: {
  id: string;
  address: string;
  proxyType: "HTTP" | "SOCKS5";
  remark?: string | null;
}): Promise<ProxyIp> {
  return invoke<ProxyIp>("update_proxy", {
    id: payload.id,
    address: payload.address,
    proxyType: payload.proxyType,
    remark: payload.remark ?? null,
  });
}

/**
 * 拉取某代理最近 N 条日志事件（来自 `proxy_failure_events`）。
 * 默认 100 条，后端上限 200。
 */
export function listProxyLogs(id: string, limit?: number): Promise<ProxyLogEntry[]> {
  return invoke<ProxyLogEntry[]>("list_proxy_logs", { id, limit: limit ?? null });
}

/**
 * 批量返回每条代理的派生健康档位（available / restricted / invalid）。
 * 由后端按 5 min 滑窗实时算，不读 DB 持久状态。
 */
export function listProxiesHealth(): Promise<ProxyHealthBrief[]> {
  return invoke<ProxyHealthBrief[]>("list_proxies_health");
}

// ── IP 管理页 v5：全局 / per-platform 视图 ────────────────────────

/** 全局 tab：基础元数据 + 双延迟。**不**触发新探测，仅读现有 probe 表。 */
export function listProxiesGlobal(): Promise<ProxyGlobalRow[]> {
  return invoke<ProxyGlobalRow[]>("list_proxies_global");
}

/** per-platform tab：最后一次响应 + 绑定 / 运行账号数 + 派生状态 + 风险系数。 */
export function listProxiesRuntime(platform: string): Promise<ProxyPlatformRow[]> {
  return invoke<ProxyPlatformRow[]>("list_proxies_runtime", { platform });
}

/**
 * 全局 tab「刷新并测延迟」按钮：批量同步刷新所有代理的 geo + cn / intl 双延迟
 * （每条代理三件事并行，最长 ~10s/条），写回 `proxies` 行，返回组装后的最新全局行。
 */
export function checkAllProxiesDualHealth(): Promise<ProxyGlobalRow[]> {
  return invoke<ProxyGlobalRow[]>("check_all_proxies_dual_health");
}

/** 读取双探针目标 URL（设置面板用）。 */
export function getProxyProbeSettings(): Promise<ProxyProbeSettings> {
  return invoke<ProxyProbeSettings>("get_proxy_probe_settings");
}

/** 写入双探针目标 URL；后端会校验 `http(s)://` 前缀。 */
export function updateProxyProbeSettings(payload: {
  cnTarget: string;
  intlTarget: string;
}): Promise<ProxyProbeSettings> {
  return invoke<ProxyProbeSettings>("update_proxy_probe_settings", {
    cnTarget: payload.cnTarget,
    intlTarget: payload.intlTarget,
  });
}

/** 读取各平台 Worker 熔断退避秒数（采集管理「采集熔断退避」弹窗）。 */
export function getWorkerBackoffSettings(): Promise<WorkerBackoffSettings> {
  return invoke<WorkerBackoffSettings>("get_worker_backoff_settings");
}

/** 写入各平台退避秒数（1–3600）；与采集任务 `platform` 对应。 */
export function updateWorkerBackoffSettings(
  secondsByPlatform: Record<string, number>,
): Promise<WorkerBackoffSettings> {
  // Tauri 2：Rust 侧参数名为 `payload`，invoke 必须包一层 `payload`。
  return invoke<WorkerBackoffSettings>("update_worker_backoff_settings", {
    payload: { secondsByPlatform },
  });
}

/** `ipId`：`proxies.id`，二维码请求经该代理出口发出。 */
export function generateLoginQr(payload: {
  platform: string;
  ipId: string;
}): Promise<GenerateQrResponse> {
  return invoke<GenerateQrResponse>("generate_login_qr", {
    platform: payload.platform,
    ipId: payload.ipId,
  });
}

/** 微博扫码轮询；`success` 时后端会写入 Cookie 并更新账号 */
export function pollWeiboQrLogin(accountId: string): Promise<WeiboQrPollResponse> {
  return invoke<WeiboQrPollResponse>("poll_weibo_qr_login", {
    accountId,
  });
}

export function listTasks(platform?: string | null): Promise<CrawlTask[]> {
  return invoke<CrawlTask[]>("list_tasks", { platform: platform ?? null });
}

export function createTask(payload: {
  platform: string;
  task_type: string;
  name: string;
  strategy: string;
  rate_limit: number;
  account_ids?: string[] | null;
  proxy_ids?: string[] | null;
  /** "per_worker" | "per_account"；省略时后端默认 per_worker */
  rate_limit_scope?: string | null;
  /** 微博任务专用，结构与后端 `WeiboTaskPayload` 一致 */
  weibo_config?: unknown | null;
}): Promise<CrawlTask> {
  return invoke<CrawlTask>("create_task", {
    args: {
      platform: payload.platform,
      taskType: payload.task_type,
      name: payload.name,
      strategy: payload.strategy,
      rateLimit: payload.rate_limit,
      accountIds: payload.account_ids ?? null,
      proxyIds: payload.proxy_ids ?? null,
      rateLimitScope: payload.rate_limit_scope ?? null,
      weiboConfig: payload.weibo_config ?? null,
    },
  });
}

export function updateTask(payload: {
  id: string;
  name: string;
  strategy: string;
  rate_limit: number;
  account_ids?: string[] | null;
  proxy_ids?: string[] | null;
  rate_limit_scope?: string | null;
  weibo_config?: unknown | null;
}): Promise<CrawlTask> {
  return invoke<CrawlTask>("update_task", {
    args: {
      id: payload.id,
      name: payload.name,
      strategy: payload.strategy,
      rateLimit: payload.rate_limit,
      accountIds: payload.account_ids ?? null,
      proxyIds: payload.proxy_ids ?? null,
      rateLimitScope: payload.rate_limit_scope ?? null,
      weiboConfig: payload.weibo_config ?? null,
    },
  });
}

export function deleteTask(id: string): Promise<void> {
  return invoke<void>("delete_task", { id });
}

export function startTask(id: string): Promise<void> {
  return invoke<void>("start_task", { id });
}

export function pauseTask(id: string): Promise<void> {
  return invoke<void>("pause_task", { id });
}

export function restartTask(id: string): Promise<void> {
  return invoke<void>("restart_task", { id });
}

export interface TaskProgress {
  pending: number;
  running: number;
  done: number;
  failed: number;
  total: number;
}

export function getTaskProgress(taskId: string): Promise<TaskProgress> {
  return invoke<TaskProgress>("get_task_progress", { taskId });
}

export function retryFailedRequests(taskId: string): Promise<number> {
  return invoke<number>("retry_failed_requests", { taskId });
}

// ── Records ──────────────────────────────────────────────────────

/** 与数据列表相同的筛选维度（平台 / 搜索 / 任务 / 类型）。 */
export interface RecordListFilter {
  platform?: string | null;
  keyword?: string | null;
  taskName?: string | null;
  entityType?: string | null;
}

/** `records` 表中去重后的任务名（按当前平台筛选）；任务删除后仍可出现在列表中。 */
export function listRecordTaskNames(platform?: string | null): Promise<string[]> {
  return invoke<string[]>("list_record_task_names", {
    platform: platform ?? null,
  });
}

export function queryRecords(
  platform?: string | null,
  keyword?: string | null,
): Promise<CrawledRecord[]> {
  return invoke<CrawledRecord[]>("query_records", {
    platform: platform ?? null,
    keyword: keyword ?? null,
  });
}

export interface PagedRecords {
  items: CrawledRecord[];
  total: number;
}

export function queryRecordsPaged(
  platform?: string | null,
  keyword?: string | null,
  page?: number,
  pageSize?: number,
  taskName?: string | null,
  entityType?: string | null,
): Promise<PagedRecords> {
  return invoke<PagedRecords>("query_records_paged", {
    platform: platform ?? null,
    keyword: keyword ?? null,
    taskName: taskName ?? null,
    entityType: entityType ?? null,
    page: page ?? 1,
    pageSize: pageSize ?? 50,
  });
}

/** 导出为 JSON 文本（当前筛选下的完整记录，含 `jsonData`）。 */
export function exportRecordsJson(filter?: RecordListFilter | null): Promise<string> {
  const f = filter ?? {};
  return invoke<string>("export_records_json", {
    platform: f.platform ?? null,
    keyword: f.keyword ?? null,
    taskName: f.taskName ?? null,
    entityType: f.entityType ?? null,
  });
}

/** 导出为 Excel `.xlsx`（仅当前筛选条件下的记录）。 */
export function exportRecordsExcel(filter?: RecordListFilter | null): Promise<Uint8Array> {
  const f = filter ?? {};
  return invoke<number[]>("export_records_excel", {
    platform: f.platform ?? null,
    keyword: f.keyword ?? null,
    taskName: f.taskName ?? null,
    entityType: f.entityType ?? null,
  }).then((arr) => new Uint8Array(arr));
}

/** 将导出内容写入用户通过系统「另存为」选定的路径（二进制）。 */
export function writeExportFile(path: string, contents: Uint8Array): Promise<void> {
  return invoke<void>("write_export_file", {
    path,
    contents: Array.from(contents),
  });
}

/** 清除重复（仅在当前筛选结果内按内容键去重）。 */
export function deduplicateRecords(filter?: RecordListFilter | null): Promise<number> {
  const f = filter ?? {};
  return invoke<number>("deduplicate_records", {
    platform: f.platform ?? null,
    keyword: f.keyword ?? null,
    taskName: f.taskName ?? null,
    entityType: f.entityType ?? null,
  });
}

/** 删除当前筛选条件下的全部记录（与列表/导出维度一致）。 */
export function deleteRecordsFiltered(filter?: RecordListFilter | null): Promise<number> {
  const f = filter ?? {};
  return invoke<number>("delete_records_filtered", {
    platform: f.platform ?? null,
    keyword: f.keyword ?? null,
    taskName: f.taskName ?? null,
    entityType: f.entityType ?? null,
  });
}
