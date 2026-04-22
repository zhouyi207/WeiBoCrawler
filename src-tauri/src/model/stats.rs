use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub task_stats: TaskStats,
    pub account_stats: AccountStats,
    pub ip_stats: IpStats,
    /// 方案 B：首页「平台健康概览」按平台维度的账号 + IP 二维聚合。
    /// 顶层全局聚合 (`account_stats` / `ip_stats`) 仍保留兼容老消费方；
    /// 本字段是把同一份原始数据按 platform 切开，并把账号桶与按平台 scope 派生的 IP 桶
    /// 在 service 层 join 起来——前端拿来直接铺一张表。
    pub per_platform: Vec<PlatformOverview>,
    pub recent_logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskStats {
    pub running: i64,
    pub paused: i64,
    pub error: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountStats {
    pub normal: i64,
    pub restricted: i64,
    pub error: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IpStats {
    pub available: i64,
    pub restricted: i64,
    pub invalid: i64,
    pub total: i64,
}

/// 单一平台维度的账号 + IP 三态汇总，用于首页「平台健康概览」表格的一行。
///
/// 关于 IP 计数的语义（**注意三个 IP 计数会重叠到不同平台行**）：
/// - `ip_invalid`：`global_status == Invalid` 的代理（出口本身不通），
///   **每个**平台行都会把它计入——因为这种 IP 对所有平台任务都不可用；
/// - `ip_restricted`：`global_status != Invalid` 且 `restrictions[]` 含本 platform；
/// - `ip_available`：`global_status != Invalid` 且 `restrictions[]` 不含本 platform。
///
/// 因此对每个 PlatformOverview 都满足
/// `ip_available + ip_restricted + ip_invalid == 全库代理总数`，
/// 但不同平台行的 `ip_available / ip_restricted` 通常不一样。
///
/// `platform` 与后端 `Platform` 枚举的 serde tag (snake_case) 一致。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformOverview {
    pub platform: String,
    pub account_normal: i64,
    pub account_restricted: i64,
    pub account_error: i64,
    pub account_total: i64,
    pub ip_available: i64,
    pub ip_restricted: i64,
    pub ip_invalid: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time: String,
    pub level: String,
    pub message: String,
    /// 业务域：`account` / `proxy` / `task` / `risk` / `legacy` 等。
    pub scope: String,
    /// 动作：`create` / `update` / `delete` / `status_change` / `risk_change` / `log` 等。
    pub action: String,
}
