use serde::{Deserialize, Serialize};

use super::platform::Platform;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawledRecord {
    pub id: String,
    pub platform: Platform,
    pub task_name: String,
    /// 列表/评论请求上的搜索词；正文/纯评论任务可为空。
    pub keyword: String,
    /// 微博博文标识（`mblogid` 或 `mid` 等），与 `keyword` 分离。
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "blogId")]
    pub blog_id: Option<String>,
    pub content_preview: String,
    pub author: String,
    pub crawled_at: String,
    /// 与 WeiBoCrawler `BodyRecord.json_data` / 评论项一致，完整 API 或解析后的 JSON。
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "jsonData")]
    pub json_data: Option<String>,
    /// 父级 `records.id`：列表微博 → 一级评论 → 二级评论，形成树。
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "parentRecordId")]
    pub parent_record_id: Option<String>,
    /// `feed` | `comment_l1` | `comment_l2` | `body` 等，便于前端分层展示。
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "entityType")]
    pub entity_type: Option<String>,
}
