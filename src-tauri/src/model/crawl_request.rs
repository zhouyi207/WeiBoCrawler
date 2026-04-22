use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrawlRequestStatus {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrawlRequestType {
    ListPage,
    Body,
    CommentL1,
    CommentL2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlRequest {
    pub id: String,
    pub task_id: String,
    pub request_type: CrawlRequestType,
    /// JSON describing how to build the HTTP request (no account/proxy info).
    pub request_params: String,
    pub status: CrawlRequestStatus,
    pub account_id: Option<String>,
    pub proxy_id: Option<String>,
    pub error_message: Option<String>,
    pub response_summary: Option<String>,
    /// Full parsed response content (JSON): list items array, body JSON, comment array, etc.
    pub response_data: Option<String>,
    pub parent_request_id: Option<String>,
    pub retry_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Progress counters for a single task, returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub pending: i64,
    pub running: i64,
    pub done: i64,
    pub failed: i64,
    pub total: i64,
}
