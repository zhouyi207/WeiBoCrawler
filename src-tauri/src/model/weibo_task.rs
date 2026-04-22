//! 微博采集任务参数，与 WeiBoCrawler `request/get_list_request.py`、`get_body_request.py`、`get_comment_request.py` 的入参一致。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WeiboUidMidPair {
    pub uid: String,
    pub mid: String,
}

/// 与 Python 侧 `api` 对应的结构化配置（存入 `tasks.task_config`）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "api", rename_all = "snake_case")]
pub enum WeiboTaskPayload {
    /// `build_list_params` / 列表搜索
    List {
        search_for: String,
        /// 综合 | 实时 | 高级
        list_kind: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        advanced_kind: Option<String>,
        /// 高级搜索：起始时间，格式 `YYYY-MM-DD` 或 `YYYY-MM-DD-H`
        #[serde(default, skip_serializing_if = "Option::is_none")]
        time_start: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        time_end: Option<String>,
    },
    /// `build_body_params` / 详细页（`status_ids` 为微博短 id，空格分隔录入后拆成数组）
    Body {
        status_ids: Vec<String>,
    },
    CommentL1 {
        pairs: Vec<WeiboUidMidPair>,
    },
    CommentL2 {
        pairs: Vec<WeiboUidMidPair>,
    },
}
