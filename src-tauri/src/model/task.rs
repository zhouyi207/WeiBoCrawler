use serde::{Deserialize, Serialize};

use super::platform::Platform;
use super::weibo_task::WeiboTaskPayload;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    Paused,
    Completed,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Keyword,
    UserProfile,
    Trending,
    CommentLevel1,
    CommentLevel2,
}

/// 任务级 IP 池调度策略（并发模式下账号始终全部并行使用，
/// 该枚举仅决定把 N 个绑定代理派发到 worker 上的方式）：
/// - `RoundRobin`：按账号 × 代理顺序展开 worker。
/// - `Random`：展开后再 shuffle worker 列表。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrawlStrategy {
    RoundRobin,
    Random,
}

/// rate_limit 的限流粒度：
/// - `PerWorker`（默认）：每个 (账号,代理) worker 独立 60_000/rate_limit ms 间隔。
/// - `PerAccount`：同一账号下的多个 worker 共享一个令牌桶，每账号 QPS = rate_limit/min。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitScope {
    PerWorker,
    PerAccount,
}

impl Default for RateLimitScope {
    fn default() -> Self {
        RateLimitScope::PerWorker
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlTask {
    pub id: String,
    pub platform: Platform,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub name: String,
    pub status: TaskStatus,
    pub strategy: CrawlStrategy,
    pub rate_limit: i64,
    pub account_pool_size: i64,
    pub ip_pool_size: i64,
    pub created_at: String,
    /// 绑定的账号 id 列表（JSON），可选。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_account_ids: Option<Vec<String>>,
    /// 绑定的代理 id 列表（JSON），可选；为空时所有 worker 走裸客户端。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_proxy_ids: Option<Vec<String>>,
    /// rate_limit 的限流粒度。
    #[serde(default)]
    pub rate_limit_scope: RateLimitScope,
    /// 微博任务专用参数（与 WeiBoCrawler 各 request 对齐），其它平台为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "weiboConfig")]
    pub weibo_config: Option<WeiboTaskPayload>,
}
