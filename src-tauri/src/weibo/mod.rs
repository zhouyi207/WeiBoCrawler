//! 微博登录与二维码（对齐 WeiBoCrawler/WeiBoCrawler/request/get_cookies.py）

pub mod api;
mod crawl;
mod list_parse;
mod qrcode;
mod session;
mod sina_profile;

pub(crate) use qrcode::cookies_json_from_jar;
pub use qrcode::{poll_weibo_qr_once, request_weibo_login_qr};
pub use session::WeiboLoginSession;
pub use sina_profile::enrich_account_from_my_sina_session;

pub(crate) use crawl::{
    execute_single_request, fmt_reqwest_error, http_client, http_client_with_proxy,
    http_client_with_proxy_and_timeout, rate_limit_sleep, weibo_cookies_from_json,
};
