use std::sync::Arc;

use reqwest::cookie::Jar;

/// 与 Python `get_qr_Info` 返回的 `(httpx.Client, login_signin_url, qrid)` 对应：
/// 在内存中保持同一 `reqwest::Client` 以复用 Cookie，供轮询 `/sso/v2/qrcode/check` 使用。
/// `csrf_token` 对应 `X-CSRF-TOKEN` Cookie，轮询时需带 `x-csrf-token` 请求头。
/// `cookie_jar` 与 `client` 共享，用于登录成功后导出完整 Cookie 写入数据库。
#[derive(Clone)]
pub struct WeiboLoginSession {
    pub client: reqwest::blocking::Client,
    pub cookie_jar: Arc<Jar>,
    pub login_signin_url: String,
    pub qrid: String,
    pub csrf_token: Option<String>,
}
