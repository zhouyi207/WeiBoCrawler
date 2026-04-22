export const PLATFORMS = [
  "weibo",
  "douyin",
  "kuaishou",
  "xiaohongshu",
  "tieba",
  "zhihu",
] as const;

export type Platform = (typeof PLATFORMS)[number];

export const PLATFORM_LABELS: Record<Platform, string> = {
  weibo: "微博",
  douyin: "抖音",
  kuaishou: "快手",
  xiaohongshu: "小红书",
  tieba: "贴吧",
  zhihu: "知乎",
};

export type TaskStatus = "running" | "paused" | "completed" | "error";
export type TaskType =
  | "keyword"
  | "user_profile"
  | "trending"
  | "comment_level1"
  | "comment_level2";

/** 与 WeiBoCrawler `get_list_request` / `get_body_request` / `get_comment_request` 对应的结构化参数 */
export type WeiboTaskPayload =
  | {
      api: "list";
      search_for: string;
      list_kind: string;
      advanced_kind?: string | null;
      time_start?: string | null;
      time_end?: string | null;
    }
  | { api: "body"; status_ids: string[] }
  | {
      api: "comment_l1";
      pairs: { uid: string; mid: string }[];
    }
  | {
      api: "comment_l2";
      pairs: { uid: string; mid: string }[];
    };

export const TASK_TYPE_LABELS: Record<TaskType, string> = {
  keyword: "列表搜索",
  user_profile: "详细页",
  trending: "热点/榜单",
  comment_level1: "一级评论",
  comment_level2: "二级评论",
};

export const TASK_STATUS_LABELS: Record<TaskStatus, string> = {
  running: "运行中",
  paused: "已暂停",
  completed: "已完成",
  error: "异常",
};

export type CrawlStrategy = "round_robin" | "random";

/**
 * rate_limit/min 限流粒度：
 * - `per_worker`（默认）：每个 (账号,代理) worker 独立 60_000/rate_limit ms 间隔。
 * - `per_account`：同账号下的多个 worker 共享一个令牌，QPS = rate_limit/min。
 */
export type RateLimitScope = "per_worker" | "per_account";

export interface CrawlTask {
  id: string;
  platform: Platform;
  type: TaskType;
  name: string;
  status: TaskStatus;
  strategy: CrawlStrategy;
  rateLimit: number;
  rateLimitScope: RateLimitScope;
  accountPoolSize: number;
  ipPoolSize: number;
  createdAt: string;
  /** 采集任务绑定的账号 id（如微博多选） */
  boundAccountIds?: string[];
  /** 采集任务绑定的代理 id（与账号笛卡尔积决定 worker 数量） */
  boundProxyIds?: string[];
  /** 微博任务参数（与 WeiBoCrawler 各 request 对齐） */
  weiboConfig?: WeiboTaskPayload;
}

export type AccountStatus = "normal" | "restricted" | "error";
/** 微博扩展资料（表 `weibo_account_profiles`） */
export interface WeiboAccountProfile {
  uid: string;
  centerWeiboName?: string;
}

export interface Account {
  id: string;
  platform: Platform;
  username: string;
  boundIp: string | null;
  /**
   * v6：账号绑定的代理外键，对应 `proxies.id`。
   * - 走代理路径登录的账号会被回填；
   * - 仅展示用直连 IP 而无代理行时为空；
   * - 老库经一次性 backfill 也可能为空（address 不再匹配等）。
   */
  boundProxyId?: string;
  riskStatus: AccountStatus;
  /** 首次入库时间（扫码成功写入 `accounts` 时） */
  createdAt: string;
  /** 最近活跃：登录成功或采集请求成功时会刷新 */
  lastActiveAt: string;
  /** 后端登录后的 Cookie（JSON），可选 */
  cookies?: string;
  /** 仅微博账号在资料拉取成功后存在 */
  weiboProfile?: WeiboAccountProfile;
}

/** 微博扫码轮询结果，对应 `poll_weibo_qr_login` */
export interface WeiboQrPollResponse {
  status: "waiting" | "success" | "failed";
  message?: string;
  cookies?: string;
  /** 与已有微博为同一 uid，后端已合并到该账号 id，临时扫码行已删除 */
  mergedIntoAccountId?: string;
}

/** 对应后端 `generate_login_qr` */
export interface GenerateQrResponse {
  accountId: string;
  qrData: string;
}

/** `Direct` 是「本机直连」伪代理类型，对应后端 `ProxyType::Direct` 与系统内置行 `local-direct`。 */
export type ProxyType = "HTTP" | "SOCKS5" | "Direct";

/**
 * IP 派生健康档位。**v3 起不再持久化在 `proxies.status`**，前端任何需要档位的
 * 场景（CreateTaskModal 禁选 / 日志 modal 摘要）都通过 `listProxiesHealth`
 * 拉派生值，不要再假设 `ProxyIp` 自带 status。
 */
export type IpStatus = "available" | "restricted" | "invalid";

/** 与后端 `model::proxy::LOCAL_DIRECT_PROXY_ID` 保持一致。 */
export const LOCAL_DIRECT_PROXY_ID = "local-direct";

/**
 * 代理基础元数据。
 *
 * **v7 stack 重构**：
 * - `proxy_latency_probes` 表已被 stack 进 `proxies` 行：每条代理直接带
 *   `cnLatencyMs` / `intlLatencyMs` 两个独立探针样本字段；
 * - 老 `geoUpdatedAt` 与 `probedAt` 合并为 `lastProbedAt`，语义升级为
 *   「该代理上次被主动刷新（geo / 双探针 任一）的时刻」。所有写入路径
 *   （add_proxy / update_proxy(address 变) / check_all_proxies_dual_health）
 *   共用同一个时间戳——一次"刷新"操作只产生一个时刻。
 *
 * `riskScore` 仍按平台 scope 在 `ProxyPlatformRow` 上动态计算，不在此结构。
 */
export interface ProxyIp {
  id: string;
  address: string;
  proxyType: ProxyType;
  /** IP 备注说明 */
  remark?: string;
  /** 系统内置行（如 `local-direct`）：前端禁删 / 禁改地址 / 禁改类型。 */
  isSystem?: boolean;
  // ── ip-api.com 反查得到的 IP 地理信息（添加代理 / 刷新并测延迟时即时反查） ──
  /** 国家 / 地区，如「中国」「United States」 */
  geoCountry?: string;
  /** 行政区（省 / 州） */
  geoRegion?: string;
  /** 城市 */
  geoCity?: string;
  /** 网络归属 / 运营商，如「China Telecom」 */
  geoIsp?: string;
  /** 反查时实际命中的 IP（与 address 中的 host 对比可识别 DNS 漂移） */
  geoIp?: string;
  // ── 双探针延迟样本（v7 stack：从 proxy_latency_probes 行转列搬入） ──
  /** 国内探针上次结果。`undefined` = 未探测；`>0` = 成功 ms；`<0` = 失败哨兵 */
  cnLatencyMs?: number;
  /** 国外探针上次结果。语义同 `cnLatencyMs` */
  intlLatencyMs?: number;
  /** 上次「主动刷新」该条代理的时间戳（YYYY-MM-DD HH:MM:SS） */
  lastProbedAt?: string;
  /**
   * v9：最近一次国内+国外双探针是否至少一端成功。均为失败时后端置 `false` 并写入库，
   * 全局健康摘要中 `globalStatus` 为不可用。
   */
  globalProbeOk?: boolean;
}

/**
 * 单次延迟探针的结果，与后端 `LatencyOutcome` 一一对应。
 *
 * 三态 discriminated union 替代原来的 `latencyMs = -1/0` 哨兵：
 * - `untested`：该 (proxy, target) 还没被探测过，前端展示「—」；
 * - `failed`：上一次探测失败（连接拒绝 / 超时 / 非 2xx）；
 * - `success`：上一次探测成功，`ms` 为耗时（毫秒）。
 *
 * v7：原始数据来自 `ProxyIp.{cn,intl}LatencyMs + lastProbedAt`，由 service
 * 层 `LatencyOutcome::from_proxy_columns` 派生。
 */
export type LatencyOutcome =
  | { kind: "untested" }
  | { kind: "failed"; probedAt: string }
  | { kind: "success"; ms: number; probedAt: string };

/**
 * IP 管理页「全局」tab 的行。基础元数据 + 双延迟。后端 `list_proxies_global`
 * /  `check_all_proxies_dual_health` 返回该结构。
 */
export interface ProxyGlobalRow extends ProxyIp {
  cnLatency: LatencyOutcome;
  intlLatency: LatencyOutcome;
}

/**
 * IP 管理页 per-platform tab 的行。后端 `list_proxies_runtime(platform)` 返回。
 *
 * v6：直接 `extends ProxyIp`，与 `ProxyGlobalRow` 对称（后端用 `#[serde(flatten)]`
 * 一次注入 id / address / geo* 等全部基础元数据），消除原本前端手抄字段后
 * 漏跟新的风险。
 *
 * 行专属字段语义：
 * - `lastResponded*`：worker 在该 (proxy, platform) 上跑过的最近一次请求；
 *   `lastStatus = "success" | "failure"`，失败时附带 `lastErrorKind` / `lastHttpStatus`；
 * - `boundAccountCount`：v7 起从「任务规划」维度反查——遍历该 platform 下所有任务，
 *   把 `tasks.bound_proxy_ids × bound_account_ids` 笛卡尔展开后按 (proxy, platform)
 *   去重 count；含义是「**有多少账号被任务规划要在该 IP 上跑**」（不论任务当前
 *   running / paused / completed / error 一视同仁）。bound_proxy_ids 为空的任务
 *   会落到「本机直连」行；
 * - `runningAccountCount`：来自后端 in-memory `WorkerRegistry` 的实时快照（worker
 *   注册 / 退出时 RAII 维护，重启清零）；UI 是手动刷新的，需点右上角「刷新并测延迟」
 *   或切换 tab 触发重新拉取；
 * - `status`：派生自 `derive_proxy_*_status_now`，max(全局, 当前平台)；
 * - `riskScore`：5 min 内 (proxy, platform) 归责到该 IP 的失败次数 × 10，封顶 100。
 */
export interface ProxyPlatformRow extends ProxyIp {
  lastRespondedAt?: string;
  lastAccountId?: string;
  lastAccountName?: string;
  lastLatencyMs?: number;
  /** "success" | "failure" */
  lastStatus?: string;
  /** 与后端 `risk::ErrorKind::as_tag` 一致 */
  lastErrorKind?: string;
  lastHttpStatus?: number;

  boundAccountCount: number;
  runningAccountCount: number;

  status: IpStatus;
  riskScore: number;
}

/** 双探针目标 URL 配置，对应后端 `ProxyProbeSettings`。 */
export interface ProxyProbeSettings {
  cnTarget: string;
  intlTarget: string;
  /** 后端固化的默认值；前端「恢复默认」按钮直接读，避免双方硬编码不一致 */
  defaultCnTarget: string;
  defaultIntlTarget: string;
}

/** Worker 连续失败熔断后的退避秒数（按任务平台），对应后端 `WorkerBackoffSettings`。 */
export interface WorkerBackoffSettings {
  /** 键为 `Platform` 的 tag（`weibo`、`douyin` 等） */
  secondsByPlatform: Record<string, number>;
  defaultBackoffSeconds: number;
}

/**
 * 后端 `command::proxy::list_proxy_logs` 返回的单条日志。每条对应
 * `proxy_failure_events` 表中一行，由采集 worker 在请求失败时通过
 * `risk::record` 写入。
 */
export interface ProxyLogEntry {
  id: string;
  /** 触发该次失败的任务 id；手动重置等场景可能为 null */
  taskId?: string | null;
  requestId?: string | null;
  /** 与后端 `risk::ErrorKind::as_tag` 一致 */
  errorKind: "network" | "http_status" | "login_required" | "business_reject" | "other" | string;
  /** errorKind === "http_status" 时填，其他为 null */
  httpStatus?: number | null;
  message?: string | null;
  /** RFC3339 时间戳 */
  occurredAt: string;
  /**
   * v4 / 方案 C：失败发生时所属任务的平台。老库回填后绝大多数有值，
   * 仍可能为 null（事件没有 task_id 关联）；前端展示时把 null 当成「全平台」即可。
   */
  platform?: Platform | string | null;
}

/**
 * 后端 `command::account::list_account_logs` 返回的单条账号日志。每条对应
 * `account_failure_events` 表中一行，结构与 `ProxyLogEntry` 对称。
 */
export interface AccountLogEntry {
  id: string;
  /** 触发该次失败的任务 id；手动重置等场景可能为 null */
  taskId?: string | null;
  requestId?: string | null;
  /** 与后端 `risk::ErrorKind::as_tag` 一致 */
  errorKind:
    | "network"
    | "http_status"
    | "login_required"
    | "business_reject"
    | "other"
    | string;
  /** errorKind === "http_status" 时填，其他为 null */
  httpStatus?: number | null;
  message?: string | null;
  /** RFC3339 时间戳 */
  occurredAt: string;
}

/**
 * 单条 (proxy, platform) 维度的受限项，仅在派生为 `restricted` 时存在。
 * 从 v4 / 方案 C 起，代理是否「受限」按平台 scope 算，避免 weibo 5xx 把
 * douyin 任务上的同一 IP 也打成受限。
 */
export interface ProxyRestriction {
  platform: Platform | string;
  status: IpStatus;
}

/**
 * 后端 `command::proxy::list_proxies_health` 返回的派生健康档位。
 *
 * - `globalStatus`：全局档（仅 `available` 或 `invalid`）。`invalid` 表示近窗内
 *   网络类失败（超时、无法连接、DNS 等）已达阈值，出口暂视为不可用；
 * - `restrictions`：(IP, platform) 维度被判 `restricted` 的平台列表，长度通常 0~2。
 *   全局已经 `invalid` 时为空（已被全局档兜底）。
 *
 * 用法：
 * - CreateTaskModal：`globalStatus === "invalid"` 或 `restrictions.some(r => r.platform === effectivePlatform)`
 *   都表示当前任务不能用该 IP；
 * - IpPage 卡片：`available` = global 非 invalid 且 restrictions 为空，`restricted`
 *   = restrictions 非空，`invalid` = global invalid；
 * - IpPage 行内：用 `restrictions` 渲染聚合「⚠ N 平台」徽章。
 */
export interface ProxyHealthBrief {
  id: string;
  globalStatus: IpStatus;
  restrictions: ProxyRestriction[];
}

export type IpStrategy = "round_robin" | "weighted" | "bind_priority";

export interface CrawledRecord {
  id: string;
  platform: Platform;
  taskName: string;
  /** 搜索关键词（列表/评论链路上的 search_for）；正文类可为空。 */
  keyword: string;
  /** 微博博文 id（mblogid / mid 等），与 keyword 分离 */
  blogId?: string | null;
  contentPreview: string;
  author: string;
  crawledAt: string;
  /** 与 WeiBoCrawler `json_data` 对齐的完整 JSON 字符串 */
  jsonData?: string;
  /** 父级记录 id：微博 feed → 一级评论 → 二级评论 */
  parentRecordId?: string;
  /** feed | comment_l1 | comment_l2 | body */
  entityType?: string;
}

/** 后端 `logs` 表 / `list_request_logs` 单条。 */
export interface RequestLogEntry {
  id: number;
  time: string;
  platform: string;
  taskId?: string | null;
  taskName?: string | null;
  crawlRequestId?: string | null;
  accountId?: string | null;
  accountName?: string | null;
  proxyId?: string | null;
  proxyAddress?: string | null;
  requestKind: string;
  phase?: string | null;
  method: string;
  url: string;
  statusCode?: number | null;
  durationMs: number;
  errorMessage?: string | null;
}

export type PageId =
  | "home"
  | "crawl"
  | "account"
  | "ip"
  | "database"
  | "requestLogs";

// ── Dashboard ────────────────────────────────────────────────────

/** 与后端 `model::stats::TaskStats` 对齐 */
export interface TaskStats {
  running: number;
  paused: number;
  error: number;
  total: number;
}

/** 与后端 `model::stats::AccountStats` 对齐（全局聚合，保留给老消费方）。 */
export interface AccountStats {
  normal: number;
  restricted: number;
  error: number;
  total: number;
}

/** 与后端 `model::stats::IpStats` 对齐 */
export interface IpStats {
  available: number;
  restricted: number;
  invalid: number;
  total: number;
}

/**
 * 首页「最近日志」单条，对应后端 `app_event_log`（CRUD、任务状态、采集风控等）。
 * `level`：`info` | `warn` | `error`。
 * `scope`：`account` | `proxy` | `task` | `risk` | `legacy` 等。
 */
export interface DashboardLogEntry {
  time: string;
  level: string;
  message: string;
  scope: string;
  action: string;
}

/**
 * 与后端 `model::stats::PlatformOverview` 对齐。首页「平台健康概览」一行的数据。
 *
 * **IP 三个计数的语义（有重叠，请勿做加和判断）**：
 * - `ipInvalid`：`global_status == invalid` 的代理（出口本身不通）。
 *   **每个**平台行都会把它计入，因为这种 IP 对所有平台任务都不可用；
 * - `ipRestricted`：全局非 invalid 且 (IP, 该平台) 维度被判 restricted；
 * - `ipAvailable`：全局非 invalid 且 (IP, 该平台) 不在 restricted 列表里。
 *
 * 因此对每行都满足
 * `ipAvailable + ipRestricted + ipInvalid == 全库代理总数`，
 * 但不同平台行之间 `ipAvailable / ipRestricted` 可能不同。
 *
 * `platform` 与后端 Platform 枚举的 serde tag (snake_case) 一致，可直接用于
 * `PLATFORM_LABELS` 取中文名。
 */
export interface PlatformOverview {
  platform: Platform | string;
  accountNormal: number;
  accountRestricted: number;
  accountError: number;
  accountTotal: number;
  ipAvailable: number;
  ipRestricted: number;
  ipInvalid: number;
}

/** 与后端 `model::stats::DashboardStats` 对齐，由 `get_dashboard_stats` 一次性返回。 */
export interface DashboardStats {
  taskStats: TaskStats;
  accountStats: AccountStats;
  ipStats: IpStats;
  /**
   * 方案 B 融合后：按平台维度的「账号 × IP」二维健康汇总。
   * 后端只返回**实际有账号**的平台（无账号的平台不会出现在数组里）。
   */
  perPlatform: PlatformOverview[];
  recentLogs: DashboardLogEntry[];
}
