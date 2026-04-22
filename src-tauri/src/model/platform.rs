use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Weibo,
    Douyin,
    Kuaishou,
    Xiaohongshu,
    Tieba,
    Zhihu,
}

impl Platform {
    /// 与 `serde(rename_all = "lowercase")` 一致的小写 tag。
    /// 用于写入 `proxy_failure_events.platform` / 调 per-platform 风控派生。
    pub fn as_tag(self) -> &'static str {
        match self {
            Platform::Weibo => "weibo",
            Platform::Douyin => "douyin",
            Platform::Kuaishou => "kuaishou",
            Platform::Xiaohongshu => "xiaohongshu",
            Platform::Tieba => "tieba",
            Platform::Zhihu => "zhihu",
        }
    }
}
