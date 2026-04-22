use serde::{Deserialize, Serialize};

use super::platform::Platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Normal,
    Restricted,
    Error,
}

/// 微博扩展字段（独立表 `weibo_account_profiles`），其它平台可将来各自建表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeiboAccountProfile {
    pub uid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center_weibo_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub platform: Platform,
    pub username: String,
    pub bound_ip: Option<String>,
    /// v6：稳定的代理外键。`bound_ip` 仍保留为「展示文本」（与 `proxies.address` 对应）；
    /// 而 IP 管理页的「绑定账号数」聚合走这个外键，避免 address 字符串 join 的歧义。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_proxy_id: Option<String>,
    pub risk_status: AccountStatus,
    /// 账号行首次写入库的时刻（扫码草稿成功入库时），对应前端「添加时间」。
    pub created_at: String,
    /// 最近一次活跃：登录成功、或采集请求成功后会刷新，对应前端「最后活跃时间」。
    pub last_active_at: String,
    /// 登录后的 Cookie（JSON），仅部分平台使用；列表接口可能为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookies: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weibo_profile: Option<WeiboAccountProfile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateQrResponse {
    pub account_id: String,
    pub qr_data: String,
}

/// 微博扫码登录轮询结果（前端可定时调用 `poll_weibo_qr_login`）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeiboQrPollResponse {
    pub status: String,
    pub message: Option<String>,
    /// 仅在 `status == "success"` 时存在，为 Cookie 键值 JSON 字符串。
    pub cookies: Option<String>,
    /// 与已有账号为同一微博 uid 时已合并到该账号，值为保留的 `accounts.id`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_into_account_id: Option<String>,
}
