use serde::{Deserialize, Serialize};

/// 代理出口类型。`Direct` 表示「不走代理，直接用本机出口」，是 v2 引入的伪代理类型，
/// 用来把本机直连也纳入到统一的 (账号 × 代理) worker 调度 / 风控计数体系里。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyType {
    HTTP,
    SOCKS5,
    Direct,
}

/// 派生的 IP 健康档位。**v3 起不再持久化到 `proxies.status` 列**；
/// **v4 / 方案 C 起进一步拆分维度**：
/// - **全局档**（[`crate::queue::risk::derive_proxy_global_status`]）：仅 Available / Invalid，
///   表达"出口能否触达外网"，由 5 min 内 `network` 失败次数决定；
/// - **(IP, platform) 档**（[`crate::queue::risk::derive_proxy_platform_status`]）：仅
///   Available / Restricted，按平台 scope 计算，避免 weibo 5xx 把 douyin 任务上的同一 IP
///   也打成受限。
///
/// `IpStatus` 枚举本身保持三态共用：
/// - 风控状态机 [`crate::queue::risk::Verdict::proxy`]：取 max(全局, 当前任务平台档)；
/// - `command::proxy::list_proxies_health` 在 `restrictions[]` 里暴露 per-platform Restricted 项；
/// - 日志 modal 展示当前派生档位（按平台分组）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpStatus {
    Available,
    Restricted,
    Invalid,
}

/// `local-direct` 系统行的固定 id，写库 / 读取 / 前端引用都以该常量为准。
pub const LOCAL_DIRECT_PROXY_ID: &str = "local-direct";

/// 持久化的代理出口元数据。**v7 stack 重构**：
/// - 老 `proxy_latency_probes` 表已被 `migrate_proxies_stack_latency_and_merge_probed_at`
///   行转列融进 `proxies`：每条代理直接带 `cn_latency_ms` / `intl_latency_ms`
///   两个独立探针样本，不再 N×2 行外联；
/// - 老 `geo_updated_at` 与原 `probed_at` 合并为 `last_probed_at`，语义升级为
///   "该代理上次被主动刷新（geo / 延迟探针 任一）的时刻"。所有写入路径
///   （add_proxy / update_proxy(address 变) / check_all_proxies_dual_health）
///   都共享同一个时间戳：用户视角 = 一次"刷新"。
///
/// 不再存的：`status / bound_account_count / running_task_count / risk_score`
/// （详见 v5 注释）——风险系数仍由 `compute_platform_risk_score` 在线算。
///
/// `ProxyIp` 现在保留：用户可见的静态元数据（地址 / 类型 / 备注 / 地理）+
/// 系统标记 + 双探针延迟 + 统一刷新时间戳。运行时画像（最近响应、绑定账号数等）
/// 仍走 [`ProxyPlatformRow`]。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyIp {
    pub id: String,
    pub address: String,
    pub proxy_type: ProxyType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remark: Option<String>,
    /// 系统内置行（如 `local-direct`）：前端禁止删除 / 改地址 / 改类型。
    #[serde(default)]
    pub is_system: bool,

    // ── IP 实际地址信息（添加代理时通过 ip-api.com 反查并缓存） ─────────────
    // 失败 / 未查询时为 None；前端显示「—」。
    /// 国家（中文，如「中国」「美国」）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo_country: Option<String>,
    /// 行政区（省 / 州，如「广东」「California」）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo_region: Option<String>,
    /// 城市
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo_city: Option<String>,
    /// ISP（运营商，如「China Telecom」「Cloudflare」）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo_isp: Option<String>,
    /// 反查时实际命中的 IP（与 `address` 中解析出的 host 对比可发现 DNS 漂移）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo_ip: Option<String>,

    // ── 双探针延迟样本（v7 stack：从 proxy_latency_probes 行转列搬入） ──────
    /// 国内探针上次结果。`None` = 未探测；`Some(>0)` = 成功 ms；`Some(<0)` = 失败哨兵。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cn_latency_ms: Option<i64>,
    /// 国外探针上次结果。语义同 `cn_latency_ms`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intl_latency_ms: Option<i64>,

    /// 上次"主动刷新"该条代理的时间戳（`YYYY-MM-DD HH:MM:SS`）。
    /// add_proxy / update_proxy(address 变) / check_all_proxies_dual_health
    /// 三条写入路径共享同一时间戳——一次"刷新"动作只产生一个时刻。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_probed_at: Option<String>,

    /// v9：最近一次双探针（国内 + 国外）是否**至少一端成功**。
    /// `false` 表示两端均为失败哨兵（负数 ms），全局视图应视为不可用，并写入数据库。
    #[serde(default = "default_global_probe_ok")]
    pub global_probe_ok: bool,
}

fn default_global_probe_ok() -> bool {
    true
}

/// 反查结果，仅 `service::geoip::lookup` 与 `proxy_repo::update_geo` 内部流转。
#[derive(Debug, Clone, Default)]
pub struct ProxyGeoInfo {
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub isp: Option<String>,
    pub ip: Option<String>,
}

impl ProxyIp {
    /// 构造 `local-direct` 系统行模板。仅迁移 seed 时使用。
    pub fn local_direct_template() -> Self {
        Self {
            id: LOCAL_DIRECT_PROXY_ID.to_string(),
            address: "本机直连".to_string(),
            proxy_type: ProxyType::Direct,
            remark: Some("不走代理，直接用本机出口".to_string()),
            is_system: true,
            geo_country: None,
            geo_region: None,
            geo_city: None,
            geo_isp: None,
            geo_ip: None,
            cn_latency_ms: None,
            intl_latency_ms: None,
            last_probed_at: None,
            global_probe_ok: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 派生视图：仅供 service / command 层读，不存盘，不允许写。
// ─────────────────────────────────────────────────────────────────────────────

/// 单次延迟探针的结果。三态 discriminated union，比早期 `latency_ms = -1/0` 哨兵更
/// 难写错（前端 / 后端任一边把 0 当成 0ms 都会立刻报类型错）。
///
/// v7 stack 后，原始数据来自 `proxies.{cn,intl}_latency_ms` (Option<i64>) +
/// `proxies.last_probed_at` (Option<String>)：
/// - `ms = None`                 → `Untested`（add_proxy 后还没跑过双探针）；
/// - `ms = Some(>0)` + 有时间戳 → `Success { ms, probed_at }`；
/// - `ms = Some(<0)` + 有时间戳 → `Failed { probed_at }`；
/// - 任一字段缺失（不该发生）  → `Untested` 兜底。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LatencyOutcome {
    /// 从未探测；前端展示「—」。
    Untested,
    /// 上一次探测失败（连接拒绝 / 超时 / 非 2xx）。
    Failed { probed_at: String },
    /// 上一次探测成功，耗时 `ms` 毫秒。
    Success { ms: u32, probed_at: String },
}

impl Default for LatencyOutcome {
    fn default() -> Self {
        LatencyOutcome::Untested
    }
}

impl LatencyOutcome {
    /// 从 `proxies` 一行里的 `(latency_ms, last_probed_at)` 派生。
    /// 取代 v7 之前的 `from_row`（基于 `proxy_latency_probes` 单独行）。
    pub fn from_proxy_columns(ms: Option<i64>, last_probed_at: Option<&str>) -> Self {
        match (ms, last_probed_at) {
            (Some(m), Some(at)) if m > 0 => LatencyOutcome::Success {
                ms: m.min(u32::MAX as i64) as u32,
                probed_at: at.to_string(),
            },
            (Some(m), Some(at)) if m < 0 => LatencyOutcome::Failed {
                probed_at: at.to_string(),
            },
            _ => LatencyOutcome::Untested,
        }
    }
}

/// 「全局」tab 里一行所需的全部数据：基础元数据 + 双探针延迟。前端不再单独
/// 调 `list_proxies` + `list_probes`，由后端拼好直接返回。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyGlobalRow {
    #[serde(flatten)]
    pub proxy: ProxyIp,
    /// 国内目标延迟结果（discriminated union）。
    pub cn_latency: LatencyOutcome,
    /// 国外目标延迟结果（discriminated union）。
    pub intl_latency: LatencyOutcome,
}

/// 「微博 / 抖音 / …」per-platform tab 里一行所需的全部数据。
///
/// - 「最后一次响应」字段按用户口径：**含失败**，由 worker 在每条请求结束后
///   upsert 到 `proxy_platform_runtime`；
/// - `bound_account_count`：`accounts` 表中 `bound_ip = proxy.address` 且
///   `platform = ?` 的行数（关系视图，不落表）；
/// - `running_account_count`：in-memory `WorkerRegistry` 当前注册到
///   (proxy_id, platform) 的账号数（运行时量，应用重启会归零）；
/// - `status`：复用 `derive_proxy_*_status_now`，取 max(全局, 当前 platform)；
/// - `risk_score`：5 min (proxy, platform) 滑窗内"失败占比 × 100"，
///   纯函数 `compute_platform_risk_score` 算出。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyPlatformRow {
    /// 嵌入基础元数据。前端 `ProxyPlatformRow extends ProxyIp` 直接拿 id / address / geo* 等字段。
    /// 这里用 `flatten` 与 `ProxyGlobalRow` 对称，避免手抄字段易漂移。
    #[serde(flatten)]
    pub proxy: ProxyIp,

    // 最后一次响应（含失败）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_responded_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_account_id: Option<String>,
    /// 最后一次响应账号的展示名（`accounts.username`），方便前端不再二次请求。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_account_name: Option<String>,
    /// 最后一次响应耗时（毫秒）；成功则 > 0，失败时 worker 也会记录请求经过的时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_latency_ms: Option<i64>,
    /// `"success" | "failure"`；与 `last_error_kind` 配合表达"成功 / 失败(network/...)"。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<String>,
    /// 失败时填，与 `risk::ErrorKind::as_tag` 对齐；成功时 None。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_http_status: Option<i64>,

    pub bound_account_count: i64,
    pub running_account_count: i64,
    pub status: IpStatus,
    /// 0 ~ 100。无任何请求时为 0；前端可按阈值上色。
    pub risk_score: i64,
}
