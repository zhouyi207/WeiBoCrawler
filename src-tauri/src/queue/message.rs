use serde::Serialize;

use crate::model::weibo_task::WeiboTaskPayload;

#[derive(Debug, Clone)]
pub struct CrawlCommand {
    pub task_id: String,
    pub platform: String,
    pub task_type: String,
    pub weibo_config: Option<WeiboTaskPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlProgressEvent {
    pub task_id: String,
    pub status: String,
    pub message: String,
}
