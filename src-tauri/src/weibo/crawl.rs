//! 对齐 WeiBoCrawler 下载流程：列表 HTML → `BodyRecord` 式 JSON、正文/评论 API → 入库 `records`（`json_data` 存完整 JSON）。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use encoding_rs::Encoding;
use regex::Regex;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use rusqlite::Connection;
use serde_json::Value;
use tauri::Emitter;

use crate::db::{account_repo, record_repo, request_log_repo, task_repo};
use crate::error::AppError;
use crate::model::account::Account;
use crate::model::crawl_request::{CrawlRequest, CrawlRequestStatus, CrawlRequestType};
use crate::model::platform::Platform;
use crate::model::proxy::ProxyIp;
use crate::model::record::CrawledRecord;
use crate::model::task::{CrawlStrategy, CrawlTask, TaskType};
use crate::model::weibo_task::WeiboTaskPayload;
use crate::queue::message::{CrawlCommand, CrawlProgressEvent};
use crate::AppState;

use super::api::{self, build_body_request, build_comment_l1_request, build_comment_l2_request};
use super::list_parse::parse_list_html;
use super::qrcode::USER_AGENT;

fn log_crawl_http(
    ctx: Option<&request_log_repo::CrawlHttpLogCtx<'_>>,
    request_kind: &str,
    phase: Option<&str>,
    method: &str,
    url: &Url,
    status_code: Option<i64>,
    error_message: Option<&str>,
    duration_ms: i64,
) {
    request_log_repo::try_insert(
        ctx,
        request_kind,
        phase,
        method,
        url.as_str(),
        status_code,
        error_message,
        duration_ms,
    );
}

/// 把 `reqwest::Error` 沿 `source()` 链展开成一行可读文本。
///
/// 为什么要走 `source()`：reqwest 顶层 `Display` 通常只说 "error sending request for url (...)"，
/// 真正的 root cause（DNS / connection refused / SOCKS auth fail / TLS handshake / 不允许 CONNECT 端口 …）
/// 都藏在 `e.source()` 里。Network 错误是排查代理 / 网络问题的关键信号，少一层就抓瞎。
pub(crate) fn fmt_reqwest_error(e: &reqwest::Error) -> String {
    let mut parts: Vec<String> = vec![e.to_string()];
    let mut src: Option<&dyn std::error::Error> = std::error::Error::source(e);
    // 防御性上限：理论上不会嵌套这么深，但避免循环引用导致死循环。
    for _ in 0..10 {
        match src {
            Some(s) => {
                let msg = s.to_string();
                // 去重相邻重复（reqwest / hyper 偶尔会让同一句出现两层）。
                if parts.last().map(|p| p != &msg).unwrap_or(true) {
                    parts.push(msg);
                }
                src = s.source();
            }
            None => break,
        }
    }
    parts.join(" → ")
}

/// Sleep only the remaining gap so that the interval from `start` to wake-up equals `target`.
/// If the elapsed time already exceeds `target`, returns immediately (no negative sleep).
pub(crate) fn rate_limit_sleep(target: Duration, elapsed: Duration) {
    if let Some(remaining) = target.checked_sub(elapsed) {
        if !remaining.is_zero() {
            std::thread::sleep(remaining);
        }
    }
}

/// 与 WeiBoCrawler `pack/get_list_data.Downloader._get_request_params` 的 `range(1, 51)` 一致（最多 50 页）。
const LIST_MAX_PAGES: i32 = 50;

pub(crate) fn emit_crawl_progress(
    app: &tauri::AppHandle,
    task_id: &str,
    status: &str,
    message: impl Into<String>,
) {
    let _ = app.emit(
        "crawl-progress",
        &CrawlProgressEvent {
            task_id: task_id.to_string(),
            status: status.to_string(),
            message: message.into(),
        },
    );
}

fn log_preview_text(s: &str, max_chars: usize) -> String {
    let t: String = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        format!("{t}…")
    } else {
        t
    }
}

fn html_document_title(html: &str) -> String {
    Regex::new(r"(?i)<title[^>]*>([^<]{1,200})</title>")
        .ok()
        .and_then(|re| re.captures(html))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().replace(['\n', '\r'], " "))
        .unwrap_or_else(|| "—".into())
}

fn normalize_charset_label(label: &str) -> String {
    match label.trim().trim_matches('"').trim_matches('\'').to_ascii_lowercase()
        .as_str()
    {
        "gb2312" | "gbk" | "cp936" | "windows-936" => "gb18030".to_string(),
        "utf8" => "utf-8".to_string(),
        s => s.to_string(),
    }
}

fn charset_from_content_type(ct: &str) -> Option<String> {
    let lower = ct.to_ascii_lowercase();
    let idx = lower.find("charset=")?;
    let rest = ct[idx + "charset=".len()..].trim_start();
    let label = rest
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .trim_matches('\'');
    if label.is_empty() {
        return None;
    }
    Some(normalize_charset_label(label))
}

/// 从 HTML 前部解析 `<meta charset>` / `http-equiv=Content-Type` 中的编码（与浏览器对 HTML 的常见处理一致）。
fn sniff_html_charset(bytes: &[u8]) -> Option<String> {
    let n = bytes.len().min(49152);
    let head = String::from_utf8_lossy(&bytes[..n]);
    if let Ok(re) = Regex::new(r#"(?i)<meta[^>]+charset\s*=\s*["']?\s*([a-zA-Z0-9._-]+)"#) {
        if let Some(c) = re.captures(&head) {
            return c.get(1).map(|m| normalize_charset_label(m.as_str()));
        }
    }
    if let Ok(re) = Regex::new(
        r#"(?i)<meta[^>]+http-equiv\s*=\s*["']?\s*Content-Type["']?[^>]+content\s*=\s*["'][^"']*charset\s*=\s*([a-zA-Z0-9._-]+)"#,
    ) {
        if let Some(c) = re.captures(&head) {
            return c.get(1).map(|m| normalize_charset_label(m.as_str()));
        }
    }
    None
}

/// 将响应体按 Content-Type / HTML meta / UTF-8 合法性解码为 Unicode（修复微博页常见 charset 声明与实际不符的问题）。
fn decode_http_body_bytes(bytes: &[u8], content_type: Option<&str>) -> (String, String) {
    let ct = content_type.unwrap_or("");
    let is_html = ct.to_ascii_lowercase().contains("text/html") || ct.is_empty() && {
        let prefix = &bytes[..bytes.len().min(2048)];
        let p = String::from_utf8_lossy(prefix);
        p.to_ascii_lowercase().contains("<html") || p.contains("<!DOCTYPE")
    };
    let from_meta = if is_html {
        sniff_html_charset(bytes)
    } else {
        None
    };
    let from_ct = charset_from_content_type(ct);
    let label = from_meta
        .or(from_ct)
        .filter(|l| !l.eq_ignore_ascii_case("utf-8"));

    if let Some(ref l) = label {
        if let Some(enc) = Encoding::for_label(l.as_bytes()) {
            let (cow, _, had_errors) = enc.decode(bytes);
            if !had_errors || !cow.contains('\u{fffd}') {
                return (cow.into_owned(), l.clone());
            }
        }
    }

    if let Ok(s) = std::str::from_utf8(bytes) {
        return (s.to_string(), "utf-8".into());
    }

    let (cow, _, _) = encoding_rs::GB18030.decode(bytes);
    (cow.into_owned(), "gb18030".into())
}

/// 从阻塞 `Response` 读取字节并按编码解码为文本（勿使用 `Response::text()`，以免错误 charset 导致乱码）。
fn read_response_text_with_charset(
    resp: reqwest::blocking::Response,
) -> Result<(String, String, reqwest::Url), AppError> {
    let final_url = resp.url().clone();
    let ct = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let bytes = resp
        .bytes()
        .map_err(|e| AppError::Network(format!("读响应体: {}", fmt_reqwest_error(&e))))?;
    let (text, charset) = decode_http_body_bytes(&bytes, ct.as_deref());
    Ok((text, charset, final_url))
}

pub(crate) struct WeiboStoredCookies {
    /// `Cookie` 请求头全文（**已应用白名单裁剪**）
    pub(crate) header: String,
    /// Cookie 名 `XSRF-TOKEN`（多用于 weibo.com / s.weibo.com）
    pub(crate) xsrf_token: Option<String>,
    /// Cookie 名 `X-CSRF-TOKEN`（passport 流程；与 XSRF 可能不同，勿混用单一字段）
    pub(crate) csrf_token: Option<String>,
    /// 白名单保留的 Cookie 个数（不含 XSRF / CSRF 字段抽取，单纯按 name 计）
    pub(crate) kept: usize,
    /// 命中黑名单 / 不在白名单被丢弃的 Cookie 个数
    pub(crate) dropped: usize,
}

/// 微博会话 / 反爬指纹相关的 Cookie 名白名单（**大小写不敏感**，前缀匹配除外）。
///
/// 保留：
/// - 主会话：`SUB` / `SUBP` / `SUL` / `SCF` / `WBPSESS` / `ALF` / `ALC` / `appkey` / `un` / `PC_TOKEN`
/// - CSRF：`XSRF-TOKEN` / `X-CSRF-TOKEN`
/// - SSO / passport：`SSOLoginState` / `login_sid_t` / `cross_origin_proto` / `wvr`
/// - h5 / m.weibo.cn：`_T_WM` / `MLOGIN` / `M_WEIBOCN_PARAMS` / `WEIBOCN_FROM` / `H5_NEW_USER`
/// - 设备 / 访客指纹（反爬可能校验）：`SINAGLOBAL` / `ULV` / `UOR`
///
/// **不**保留：广告 / 统计 / 第三方追踪 Cookie（`Hm_*` / `_ga*` / `__bid_n` / `sensorsdata*`
/// / `_uab_*` / `gr_*` / `__utm*`），它们在浏览器里可以攒到几 KB，是 Cookie 头膨胀
/// 触发 CDN 8KB 请求行限制（HTTP 414）的主因。
const WEIBO_COOKIE_KEEP: &[&str] = &[
    "SUB",
    "SUBP",
    "SUL",
    "SCF",
    "WBPSESS",
    "ALF",
    "ALC",
    "appkey",
    "un",
    "PC_TOKEN",
    "XSRF-TOKEN",
    "X-CSRF-TOKEN",
    "SSOLoginState",
    "login_sid_t",
    "cross_origin_proto",
    "wvr",
    "_T_WM",
    "MLOGIN",
    "M_WEIBOCN_PARAMS",
    "WEIBOCN_FROM",
    "H5_NEW_USER",
    "SINAGLOBAL",
    "ULV",
    "UOR",
];

fn is_weibo_essential_cookie(name: &str) -> bool {
    WEIBO_COOKIE_KEEP
        .iter()
        .any(|k| k.eq_ignore_ascii_case(name))
}

pub(crate) fn weibo_cookies_from_json(cookies_json: &str) -> Result<WeiboStoredCookies, AppError> {
    let map: serde_json::Map<String, Value> = serde_json::from_str(cookies_json).map_err(|e| {
        AppError::Internal(format!("解析账号 Cookie JSON 失败: {e}"))
    })?;
    let mut parts = Vec::new();
    let mut xsrf_token = None;
    let mut csrf_token = None;
    let mut kept = 0usize;
    let mut dropped = 0usize;
    for (k, v) in map {
        let s = match v {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => {
                if b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            _ => continue,
        };
        // XSRF / CSRF 即使不在白名单也要抽出来作为 header 用，但是否进入 Cookie 头
        // 仍受白名单控制（这俩本身就在白名单里）。
        if k.eq_ignore_ascii_case("XSRF-TOKEN") {
            xsrf_token = Some(s.clone());
        }
        if k.eq_ignore_ascii_case("X-CSRF-TOKEN") {
            csrf_token = Some(s.clone());
        }
        if is_weibo_essential_cookie(&k) {
            parts.push(format!("{k}={s}"));
            kept += 1;
        } else {
            dropped += 1;
        }
    }
    if parts.is_empty() {
        return Err(AppError::Internal(
            "账号 Cookie 白名单裁剪后为空，请重新扫码登录微博账号".into(),
        ));
    }
    Ok(WeiboStoredCookies {
        header: parts.join("; "),
        xsrf_token,
        csrf_token,
        kept,
        dropped,
    })
}

pub(crate) fn merge_weibo_stored_cookies(mut base: HeaderMap, c: &WeiboStoredCookies) -> Result<HeaderMap, AppError> {
    base.insert(
        reqwest::header::COOKIE,
        HeaderValue::from_str(&c.header).map_err(|e| AppError::Internal(format!("Cookie header invalid: {e}")))?,
    );
    let for_xsrf = c.xsrf_token.as_deref().or(c.csrf_token.as_deref());
    let for_csrf = c.csrf_token.as_deref().or(c.xsrf_token.as_deref());
    if let Some(t) = for_xsrf {
        base.insert(
            HeaderName::from_static("x-xsrf-token"),
            HeaderValue::from_str(t).map_err(|e| AppError::Internal(format!("XSRF header invalid: {e}")))?,
        );
    }
    if let Some(t) = for_csrf {
        base.insert(
            HeaderName::from_static("x-csrf-token"),
            HeaderValue::from_str(t).map_err(|e| AppError::Internal(format!("CSRF header invalid: {e}")))?,
        );
    }
    Ok(base)
}

pub(crate) fn http_client() -> Result<Client, AppError> {
    http_client_with_proxy(None)
}

/// Build an HTTP client optionally tunneling through a proxy.
///
/// 当 worker 拿到自己的 `(账号, 代理)` 对时，调用此函数为该 worker 构造**专用**
/// `reqwest::Client`：connection pool 与 proxy 一并固定下来，避免不同 worker
/// 串扰。`proxy` 为 `None` 时与 [`http_client`] 行为一致。
pub(crate) fn http_client_with_proxy(proxy: Option<&ProxyIp>) -> Result<Client, AppError> {
    http_client_with_proxy_and_timeout(proxy, Duration::from_secs(120))
}

/// 与 [`http_client_with_proxy`] 同构，但允许指定超时。代理健康检测会用更短的
/// 8s 超时，避免「本来代理就挂了」的探测拖住整个刷新按钮 ~120s。
pub(crate) fn http_client_with_proxy_and_timeout(
    proxy: Option<&ProxyIp>,
    timeout: Duration,
) -> Result<Client, AppError> {
    let mut builder = Client::builder().user_agent(USER_AGENT).timeout(timeout);
    if let Some(p) = proxy {
        let scheme = match p.proxy_type {
            crate::model::proxy::ProxyType::HTTP => Some("http"),
            crate::model::proxy::ProxyType::SOCKS5 => Some("socks5"),
            // Direct 是 v2 引入的伪代理类型：worker 拿到后**不挂代理**，
            // 走系统默认出口；目的是让直连路径也能被风控统计 / 调度模型涵盖。
            crate::model::proxy::ProxyType::Direct => None,
        };
        if let Some(scheme) = scheme {
            let url = if p.address.contains("://") {
                p.address.clone()
            } else {
                format!("{scheme}://{}", p.address)
            };
            let proxy = reqwest::Proxy::all(&url)
                .map_err(|e| AppError::Internal(format!("Proxy URL invalid: {e}")))?;
            builder = builder.proxy(proxy);
        }
    }
    builder
        .build()
        .map_err(|e| AppError::Internal(format!("http client: {e}")))
}

fn truncate_preview(s: &str, max: usize) -> String {
    let t: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        format!("{t}…")
    } else {
        t
    }
}

/// 列表解析项中的博文标识（`mblogid` 优先，否则 `mid`）。
fn feed_blog_id_from_item(item: &Value) -> Option<String> {
    item.get("mblogid")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            item.get("mid")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
}

/// `keyword`：搜索词；`blog_id`：博文 id（与 keyword 分离入库）。
fn insert_record(
    conn: &Connection,
    task: &CrawlTask,
    keyword: &str,
    blog_id: Option<&str>,
    preview: &str,
    author: &str,
    json: &Value,
    parent_record_id: Option<&str>,
    entity_type: Option<&str>,
) -> Result<String, AppError> {
    let json_s = serde_json::to_string(json).map_err(|e| AppError::Internal(e.to_string()))?;
    let crawled = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let id = uuid::Uuid::new_v4().to_string();
    let r = CrawledRecord {
        id: id.clone(),
        platform: task.platform,
        task_name: task.name.clone(),
        keyword: keyword.to_string(),
        blog_id: blog_id.map(String::from),
        content_preview: truncate_preview(preview, 500),
        author: author.to_string(),
        crawled_at: crawled,
        json_data: Some(json_s),
        parent_record_id: parent_record_id.map(String::from),
        entity_type: entity_type.map(String::from),
    };
    record_repo::insert(conn, &r)?;
    Ok(id)
}

fn crawl_request_ts() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn new_pending_crawl_request(
    task_id: &str,
    request_type: CrawlRequestType,
    request_params: &Value,
    parent_request_id: Option<&str>,
) -> CrawlRequest {
    let now = crawl_request_ts();
    CrawlRequest {
        id: uuid::Uuid::new_v4().to_string(),
        task_id: task_id.to_string(),
        request_type,
        request_params: request_params.to_string(),
        status: CrawlRequestStatus::Pending,
        account_id: None,
        proxy_id: None,
        error_message: None,
        response_summary: None,
        response_data: None,
        parent_request_id: parent_request_id.map(String::from),
        retry_count: 0,
        created_at: now.clone(),
        updated_at: now,
    }
}

/// 列表项中用于评论接口的 uid + 微博 mid（`buildComments` 的 `id` / `uid`）。
fn weibo_uid_mid_from_list_item(item: &Value) -> Option<(String, String)> {
    let uid = item
        .get("uid")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let mid = item
        .get("mid")
        .or_else(|| item.get("mblogid"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    Some((uid.to_string(), mid.to_string()))
}

fn comment_id_string(c: &Value) -> Option<String> {
    if let Some(s) = c.get("idstr").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    c.get("id").map(|v| {
        if let Some(s) = v.as_str() {
            s.to_string()
        } else {
            v.to_string()
        }
    })
}

/// 二级评论挂到对应一级评论记录下；无法解析时挂到 feed 根。
fn parent_record_for_l2_item(
    item: &Value,
    feed_record_id: &str,
    l1_map: &HashMap<String, String>,
) -> String {
    if let Some(reply) = item.get("reply_comment") {
        if let Some(cid) = comment_id_string(reply) {
            if let Some(rid) = l1_map.get(&cid) {
                return rid.clone();
            }
        }
    }
    if let Some(cid) = item.get("reply_id").and_then(|v| v.as_str()) {
        if let Some(rid) = l1_map.get(cid) {
            return rid.clone();
        }
    }
    if let Some(cid) = item.get("rootid").and_then(|v| v.as_str()) {
        if let Some(rid) = l1_map.get(cid) {
            return rid.clone();
        }
    }
    feed_record_id.to_string()
}

pub(crate) fn pick_account(conn: &Connection, task: &CrawlTask) -> Result<Account, AppError> {
    let ids = task.bound_account_ids.as_ref().filter(|v| !v.is_empty()).ok_or_else(|| {
        AppError::Internal(
            "任务未绑定账号：请在任务配置中绑定至少一个微博账号".into(),
        )
    })?;
    for id in ids {
        let acc = account_repo::get_by_id(conn, id)?;
        if acc.platform != Platform::Weibo {
            continue;
        }
        if acc.cookies.as_ref().map(|c| c.len() > 2).unwrap_or(false) {
            return Ok(acc);
        }
    }
    Err(AppError::Internal(
        "任务绑定的微博账号中无有效 Cookie：请重新登录对应账号或更换绑定".into(),
    ))
}

/// 首屏解析不到条目时写入日志，便于区分登录态 / 反爬 / 纯前端壳页。
fn diagnose_list_page(html: &str, final_url: &str) -> String {
    let len = html.len();
    let feed = html.contains("feed_list_item");
    let pl = html.contains("pl_feedlist_index");
    let card = html.contains("card-wrap");
    let login = html.contains("passport.weibo.com")
        || html.contains("请登录")
        || html.contains("登录微博");
    let gate =
        html.contains("验证码") || html.contains("验证身份") || html.contains("安全验证");
    let title = html_document_title(html);
    format!(
        "诊断：final_url={final_url} html_len={len} title={title} \
         token_feed_list_item={feed} token_pl_feedlist={pl} token_card_wrap={card} \
         hint_login={login} hint_captcha={gate}。\
         若 hint_login/hint_captcha 为真请重新登录或完成验证；\
         若 HTML 中无 feed 相关标记，可能是搜索页已改为纯前端渲染，需换采集方式。\
         列表页多策略尝试明细见 `crawl_requests.response_data.list_fetch_attempts`。"
    )
}

/// 首页优先 WeiBoCrawler：`body_headers` + 默认 referer；失败再试导航头与其它 XHR 组合。翻页：`body_headers` + 上一页 URL，与 Python 一致。
/// 每次 HTTP 尝试的明细写入返回的 `attempts`（供 `crawl_requests.response_data` 落库），**不再**写入 `records`。
fn fetch_list_html_multi(
    log_ctx: Option<&request_log_repo::CrawlHttpLogCtx<'_>>,
    bound_account_id: &str,
    client: &Client,
    url: &Url,
    stored: &WeiboStoredCookies,
    page: i32,
    prev_list_url: Option<&Url>,
) -> Result<(String, String, Vec<Value>), AppError> {
    let strategies: Vec<(&'static str, HeaderMap)> = if page == 1 {
        vec![
            (
                "list_p1_body_toml_referer",
                api::list_headers_weibo_python_page1(),
            ),
            (
                "list_p1_nav_from_weibo_com",
                api::list_headers_page1_from_weibo(),
            ),
            ("list_p1_nav_direct", api::list_headers_page1_direct()),
            ("list_p1_xhr_no_referer", api::list_xhr_headers(None)),
            (
                "list_p1_xhr_weibo_root",
                api::list_xhr_headers(Some("https://weibo.com/")),
            ),
        ]
    } else {
        let prev = prev_list_url
            .map(|u| u.to_string())
            .ok_or_else(|| AppError::Internal("列表翻页缺少上一页 URL".into()))?;
        vec![
            ("list_follow_xhr_prev", api::list_xhr_headers(Some(&prev))),
            ("list_follow_nav_prev", api::list_headers_followup(&prev)),
        ]
    };

    let mut last_html = String::new();
    let mut last_url = String::new();
    let mut attempts: Vec<Value> = Vec::with_capacity(strategies.len());
    for (si, (strategy_key, hdr)) in strategies.iter().enumerate() {
        let h = merge_weibo_stored_cookies(hdr.clone(), stored)?;
        let diag = ReqDiag::snapshot(url, &h);
        let t0 = Instant::now();
        let resp = match client.get(url.as_str()).headers(h).send() {
            Ok(r) => r,
            Err(e) => {
                let msg = fmt_reqwest_error(&e);
                log_crawl_http(
                    log_ctx,
                    "list_html",
                    Some(strategy_key),
                    "GET",
                    url,
                    None,
                    Some(&msg),
                    t0.elapsed().as_millis() as i64,
                );
                return Err(AppError::Network(msg));
            }
        };
        let status = resp.status();
        let code_i = status.as_u16() as i64;
        let read_res = read_response_text_with_charset(resp);
        let dur_ms = t0.elapsed().as_millis() as i64;
        match &read_res {
            Ok(_) => log_crawl_http(
                log_ctx,
                "list_html",
                Some(strategy_key),
                "GET",
                url,
                Some(code_i),
                None,
                dur_ms,
            ),
            Err(e) => log_crawl_http(
                log_ctx,
                "list_html",
                Some(strategy_key),
                "GET",
                url,
                Some(code_i),
                Some(&e.to_string()),
                dur_ms,
            ),
        }
        let (html, charset_used, final_u) = read_res?;
        last_url = final_u.to_string();
        let parsed_n = parse_list_html(&html).len();
        let page_title = html_document_title(&html);
        let html_char_len = html.chars().count();
        let trace_label = format!(
            "列表·第{}页·第{}次·{}",
            page,
            si + 1,
            match *strategy_key {
                "list_p1_body_toml_referer" => "body+默认referer",
                "list_p1_nav_from_weibo_com" => "导航·从weibo.com",
                "list_p1_nav_direct" => "导航·无referer",
                "list_p1_xhr_no_referer" => "XHR·无referer",
                "list_p1_xhr_weibo_root" => "XHR·weibo根",
                "list_follow_xhr_prev" => "XHR·上一页",
                "list_follow_nav_prev" => "导航·上一页",
                _ => strategy_key,
            }
        );
        let trace_json = serde_json::json!({
            "trace": true,
            "kind": "list_html",
            "trace_label": trace_label,
            "bound_account_id": bound_account_id,
            "strategy_key": strategy_key,
            "page": page,
            "attempt_index": si + 1,
            "request_url": url.as_str(),
            "http_status": status.as_u16(),
            "final_url": last_url,
            "page_title": page_title,
            "parsed_item_count": parsed_n,
            "cookie_header_non_empty": !stored.header.is_empty(),
            "has_xsrf_cookie": stored.xsrf_token.is_some(),
            "has_csrf_cookie": stored.csrf_token.is_some(),
            "charset_decoded": charset_used,
            "html_char_len": html_char_len,
            "html": html.clone(),
        });
        attempts.push(trace_json);

        if !status.is_success() {
            diag.warn(&format!("列表页·第{}页·策略{}", page, si + 1), status.as_u16());
            return Err(AppError::HttpStatus {
                code: status.as_u16(),
                body_excerpt: diag.embed(html_excerpt(&html, 256)),
            });
        }
        last_html = html.clone();
        if parsed_n > 0 {
            return Ok((html, last_url, attempts));
        }
    }
    Ok((last_html, last_url, attempts))
}

/// 提取首屏 HTML 摘要，给 [`AppError::HttpStatus`] 留一段诊断文本。
fn html_excerpt(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// 单次 HTTP 请求的「体格」快照。在 `client.send()` 之前用 [`ReqDiag::snapshot`]
/// 抓一份，失败时通过 [`ReqDiag::embed`] 把关键数字塞进 [`AppError::HttpStatus`] 的
/// `body_excerpt`（及前端 crawl-progress），便于排查 HTTP 414 / 413 等「请求太大」类问题。
///
/// 注意 reqwest 用 `consume` 风格交付 headers，所以必须在 `.headers(h).send()`
/// 之前抓快照（送进 send 之后 HeaderMap 已被 move）。
pub(crate) struct ReqDiag {
    pub url_len: usize,
    pub cookie_len: usize,
    pub header_count: usize,
    /// 请求行 + 头部 + CRLF 的近似字节数，用于和 CDN 8KB 上限对比。
    pub request_size_approx: usize,
    /// URL 前缀（截断）；曾用于控制台 `warn`，现仅保留字段供后续诊断扩展。
    #[allow(dead_code)]
    pub url_head: String,
}

impl ReqDiag {
    pub fn snapshot(url: &Url, headers: &HeaderMap) -> Self {
        let url_str = url.as_str();
        let url_len = url_str.len();
        let cookie_len = headers
            .get(reqwest::header::COOKIE)
            .map(|v| v.as_bytes().len())
            .unwrap_or(0);
        let header_count = headers.len();
        // "Header-Name: value\r\n" 的近似累加；多值同名头会单独成行，所以 iter 即可。
        let header_bytes: usize = headers
            .iter()
            .map(|(n, v)| n.as_str().len() + v.as_bytes().len() + 4)
            .sum();
        // 请求行约 "GET <path?query> HTTP/1.1\r\n"，留 32 字节裕量。
        let request_size_approx = url_len + header_bytes + 32;
        let url_head = if url_len <= 200 {
            url_str.to_string()
        } else {
            format!("{}…", &url_str[..200])
        };
        ReqDiag {
            url_len,
            cookie_len,
            header_count,
            request_size_approx,
            url_head,
        }
    }

    /// 把摘要折成一行短串，前缀进 `body_excerpt`，便于前端日志一眼看到关键数字。
    pub fn embed(&self, body: String) -> String {
        format!(
            "[diag url={}b cookie={}b hdrs={}/{}b] {}",
            self.url_len, self.cookie_len, self.header_count, self.request_size_approx, body
        )
    }

    /// 预留钩子：不再默认 `warn!` 打控制台（易刷屏）；需要时可改为 `log::debug!` 并设 `RUST_LOG`。
    pub fn warn(&self, _label: &str, _code: u16) {}
}

/// 列表页 HTML 是否命中典型登录拦截特征。`final_url` 跳到 `passport.weibo.com`
/// 是最强信号；HTML 中的中文文案是兜底兜底，避免空白页直接被算成账号风险。
fn list_html_indicates_login_required(html: &str, final_url: &str) -> bool {
    if final_url.contains("passport.weibo.com") {
        return true;
    }
    html.contains("passport.weibo.com")
        || html.contains("请登录")
        || html.contains("登录微博")
}

/// 微博 JSON 响应若 `ok != 1` 或 `errno != 0`，按业务受限处理。
/// `ok` 字段更常见于评论 / Feed 接口；`errno` 多出现在风控 / 限流响应。
/// 返回 `Some(AppError::BusinessReject)` 表示业务拒绝；`None` 表示放行。
fn check_business_reject(v: &Value) -> Option<AppError> {
    if let Some(ok) = v.get("ok").and_then(|x| x.as_i64()) {
        if ok != 1 {
            let msg = v
                .get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("ok != 1")
                .to_string();
            let errno = v.get("errno").and_then(|x| x.as_i64());
            return Some(AppError::BusinessReject { errno, msg });
        }
    }
    if let Some(errno) = v.get("errno").and_then(|x| x.as_i64()) {
        if errno != 0 {
            let msg = v
                .get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("errno != 0")
                .to_string();
            return Some(AppError::BusinessReject { errno: Some(errno), msg });
        }
    }
    None
}

fn run_list(
    conn: &Connection,
    client: &Client,
    task: &CrawlTask,
    bound_account_id: &str,
    stored: &WeiboStoredCookies,
    search_for: &str,
    list_kind: &str,
    advanced_kind: Option<&str>,
    time_start: Option<&str>,
    time_end: Option<&str>,
    rate_limit: i64,
    app: &tauri::AppHandle,
    task_id: &str,
) -> Result<(usize, Option<String>), AppError> {
    let delay_ms = (60_000 / rate_limit.max(1)) as u64;
    let mut total = 0usize;
    let mut zero_diag: Option<String> = None;
    for page in 1..=LIST_MAX_PAGES {
        emit_crawl_progress(
            app,
            task_id,
            "progress",
            format!("列表 第 {page} 页：请求页面…"),
        );
        let url = api::build_list_url(
            search_for,
            page,
            list_kind,
            advanced_kind,
            time_start,
            time_end,
        )
        .map_err(AppError::Http)?;
        let prev = if page > 1 {
            Some(
                api::build_list_url(
                    search_for,
                    page - 1,
                    list_kind,
                    advanced_kind,
                    time_start,
                    time_end,
                )
                .map_err(AppError::Http)?,
            )
        } else {
            None
        };
        let log_ctx = request_log_repo::CrawlHttpLogCtx {
            conn,
            platform_tag: task.platform.as_tag(),
            task_id,
            crawl_request_id: None,
            account_id: Some(bound_account_id),
            proxy_id: None,
        };
        let (html, final_url, _) = fetch_list_html_multi(
            Some(&log_ctx),
            bound_account_id,
            client,
            &url,
            stored,
            page,
            prev.as_ref(),
        )?;
        let items = parse_list_html(&html);
        if items.is_empty() {
            emit_crawl_progress(
                app,
                task_id,
                "progress",
                format!("列表 第 {page} 页：解析 0 条，停止翻页"),
            );
            if page == 1 {
                zero_diag = Some(diagnose_list_page(&html, &final_url));
            }
            break;
        }
        let page_n = items.len();
        for item in &items {
            let preview = item
                .get("content_all")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let author = item
                .get("personal_name")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            insert_record(
                conn,
                task,
                search_for,
                feed_blog_id_from_item(item).as_deref(),
                preview,
                author,
                item,
                None,
                Some("feed"),
            )?;
            total += 1;
            let pv = log_preview_text(preview, 100);
            emit_crawl_progress(
                app,
                task_id,
                "progress",
                format!("[列表·第{page}页] @{author} {pv}"),
            );
        }
        emit_crawl_progress(
            app,
            task_id,
            "progress",
            format!("列表 第 {page} 页：本页 {page_n} 条，累计 {total} 条"),
        );
        std::thread::sleep(Duration::from_millis(delay_ms));
    }
    let hint = if total == 0 { zero_diag } else { None };
    Ok((total, hint))
}

fn run_body(
    conn: &Connection,
    client: &Client,
    task: &CrawlTask,
    bound_account_id: &str,
    stored: &WeiboStoredCookies,
    status_ids: &[String],
    rate_limit: i64,
    app: &tauri::AppHandle,
    task_id: &str,
) -> Result<usize, AppError> {
    let delay_ms = (60_000 / rate_limit.max(1)) as u64;
    let mut total = 0usize;
    for id in status_ids {
        let (url, mut headers) = build_body_request(id);
        headers = merge_weibo_stored_cookies(headers, stored)?;
        let log_ctx = request_log_repo::CrawlHttpLogCtx {
            conn,
            platform_tag: task.platform.as_tag(),
            task_id,
            crawl_request_id: None,
            account_id: Some(bound_account_id),
            proxy_id: None,
        };
        let t0 = Instant::now();
        let resp = match client.get(url.as_str()).headers(headers).send() {
            Ok(r) => r,
            Err(e) => {
                let msg = fmt_reqwest_error(&e);
                log_crawl_http(
                    Some(&log_ctx),
                    "body",
                    None,
                    "GET",
                    &url,
                    None,
                    Some(&msg),
                    t0.elapsed().as_millis() as i64,
                );
                return Err(AppError::Network(msg));
            }
        };
        let status = resp.status();
        let code = status.as_u16();
        if !status.is_success() {
            let body_excerpt = resp.text().ok().map(|t| html_excerpt(&t, 256)).unwrap_or_default();
            log_crawl_http(
                Some(&log_ctx),
                "body",
                None,
                "GET",
                &url,
                Some(code as i64),
                None,
                t0.elapsed().as_millis() as i64,
            );
            return Err(AppError::HttpStatus { code, body_excerpt });
        }
        let v: Value = match resp.json() {
            Ok(v) => v,
            Err(e) => {
                log_crawl_http(
                    Some(&log_ctx),
                    "body",
                    None,
                    "GET",
                    &url,
                    Some(code as i64),
                    Some(&format!("正文 JSON: {e}")),
                    t0.elapsed().as_millis() as i64,
                );
                return Err(AppError::Internal(format!("正文 JSON: {e}")));
            }
        };
        log_crawl_http(
            Some(&log_ctx),
            "body",
            None,
            "GET",
            &url,
            Some(code as i64),
            None,
            t0.elapsed().as_millis() as i64,
        );
        if let Some(err) = check_business_reject(&v) {
            return Err(err);
        }
        let preview = v
            .get("text")
            .or_else(|| v.get("text_raw"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let author = v
            .pointer("/user/screen_name")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        insert_record(conn, task, "", Some(id.as_str()), preview, author, &v, None, Some("body"))?;
        total += 1;
        let pv = log_preview_text(preview, 80);
        emit_crawl_progress(
            app,
            task_id,
            "progress",
            format!("[正文] id={id} @{author} {pv}"),
        );
        std::thread::sleep(Duration::from_millis(delay_ms));
    }
    Ok(total)
}

fn run_comments(
    conn: &Connection,
    client: &Client,
    task: &CrawlTask,
    bound_account_id: &str,
    stored: &WeiboStoredCookies,
    pairs: &[(String, String)],
    level2: bool,
    rate_limit: i64,
    app: &tauri::AppHandle,
    task_id: &str,
) -> Result<usize, AppError> {
    let delay_ms = (60_000 / rate_limit.max(1)) as u64;
    let mut total = 0usize;
    for (uid, mid) in pairs {
        let path = format!("/{uid}/{mid}");
        let (url, headers) = if level2 {
            build_comment_l2_request(uid, mid, None, &path)
        } else {
            build_comment_l1_request(uid, mid, None, &path)
        };
        let headers = merge_weibo_stored_cookies(headers, stored)?;
        let log_ctx = request_log_repo::CrawlHttpLogCtx {
            conn,
            platform_tag: task.platform.as_tag(),
            task_id,
            crawl_request_id: None,
            account_id: Some(bound_account_id),
            proxy_id: None,
        };
        let kind = if level2 { "comment_l2" } else { "comment_l1" };
        let t0 = Instant::now();
        let resp = match client.get(url.as_str()).headers(headers).send() {
            Ok(r) => r,
            Err(e) => {
                let msg = fmt_reqwest_error(&e);
                log_crawl_http(
                    Some(&log_ctx),
                    kind,
                    None,
                    "GET",
                    &url,
                    None,
                    Some(&msg),
                    t0.elapsed().as_millis() as i64,
                );
                return Err(AppError::Network(msg));
            }
        };
        let status = resp.status();
        let code = status.as_u16();
        if !status.is_success() {
            let body_excerpt = resp.text().ok().map(|t| html_excerpt(&t, 256)).unwrap_or_default();
            log_crawl_http(
                Some(&log_ctx),
                kind,
                None,
                "GET",
                &url,
                Some(code as i64),
                None,
                t0.elapsed().as_millis() as i64,
            );
            return Err(AppError::HttpStatus { code, body_excerpt });
        }
        let root: Value = match resp.json() {
            Ok(v) => v,
            Err(e) => {
                log_crawl_http(
                    Some(&log_ctx),
                    kind,
                    None,
                    "GET",
                    &url,
                    Some(code as i64),
                    Some(&format!("评论 JSON: {e}")),
                    t0.elapsed().as_millis() as i64,
                );
                return Err(AppError::Internal(format!("评论 JSON: {e}")));
            }
        };
        log_crawl_http(
            Some(&log_ctx),
            kind,
            None,
            "GET",
            &url,
            Some(code as i64),
            None,
            t0.elapsed().as_millis() as i64,
        );
        if let Some(err) = check_business_reject(&root) {
            return Err(err);
        }
        let arr = root
            .get("data")
            .and_then(|d| d.as_array())
            .cloned()
            .unwrap_or_default();
        let batch_n = arr.len();
        for mut item in arr {
            if let Some(obj) = item.as_object_mut() {
                obj.insert("f_uid".into(), Value::String(uid.clone()));
                obj.insert("f_mid".into(), Value::String(mid.clone()));
            }
            let preview = item
                .get("text")
                .or_else(|| item.get("text_raw"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let author = item
                .pointer("/user/screen_name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let et = if level2 {
                "comment_l2"
            } else {
                "comment_l1"
            };
            insert_record(
                conn,
                task,
                "",
                Some(mid.as_str()),
                preview,
                &author,
                &item,
                None,
                Some(et),
            )?;
            total += 1;
        }
        let label = if level2 { "二级评论" } else { "一级评论" };
        emit_crawl_progress(
            app,
            task_id,
            "progress",
            format!(
                "[{label}] {uid}/{mid} 本批入库 {batch_n} 条，累计 {total} 条",
            ),
        );
        std::thread::sleep(Duration::from_millis(delay_ms));
    }
    Ok(total)
}

/// 执行微博采集并写入 `records` 表；返回 `(写入条数, 列表任务且无数据时的诊断文案)`。
pub fn run_weibo_crawl(
    state: &AppState,
    cmd: &CrawlCommand,
    app: &tauri::AppHandle,
) -> Result<(usize, Option<String>), AppError> {
    let conn = state.db.open_crawl_connection()?;
    let task = task_repo::get_by_id(&conn, &cmd.task_id)?;
    if !matches!(task.platform, Platform::Weibo) {
        return Ok((0, None));
    }

    if matches!(task.task_type, TaskType::Trending) {
        return Ok((0, None));
    }

    let cfg = task
        .weibo_config
        .as_ref()
        .ok_or_else(|| AppError::Internal("微博采集任务缺少 weiboConfig".into()))?;

    let account = pick_account(&conn, &task)?;
    let cookies_json = account
        .cookies
        .as_ref()
        .ok_or_else(|| AppError::Internal("账号无 Cookie，请重新登录".into()))?;
    let stored = weibo_cookies_from_json(cookies_json)?;
    let client = http_client()?;
    let rate = task.rate_limit;

    emit_crawl_progress(
        app,
        &cmd.task_id,
        "progress",
        format!("开始采集「{}」", task.name),
    );

    let (n, list_hint) = match (&task.task_type, cfg) {
        (
            TaskType::Keyword,
            WeiboTaskPayload::List {
                search_for,
                list_kind,
                advanced_kind,
                time_start,
                time_end,
            },
        ) => run_list(
            &conn,
            &client,
            &task,
            &account.id,
            &stored,
            search_for,
            list_kind,
            advanced_kind.as_deref(),
            time_start.as_deref(),
            time_end.as_deref(),
            rate,
            app,
            &cmd.task_id,
        )?,
        (TaskType::UserProfile, WeiboTaskPayload::Body { status_ids }) => (
            run_body(
                &conn,
                &client,
                &task,
                &account.id,
                &stored,
                status_ids.as_slice(),
                rate,
                app,
                &cmd.task_id,
            )?,
            None,
        ),
        (TaskType::CommentLevel1, WeiboTaskPayload::CommentL1 { pairs }) => {
            let p: Vec<(String, String)> = pairs
                .iter()
                .map(|x| (x.uid.clone(), x.mid.clone()))
                .collect();
            (
                run_comments(
                    &conn,
                    &client,
                    &task,
                    &account.id,
                    &stored,
                    &p,
                    false,
                    rate,
                    app,
                    &cmd.task_id,
                )?,
                None,
            )
        }
        (TaskType::CommentLevel2, WeiboTaskPayload::CommentL2 { pairs }) => {
            let p: Vec<(String, String)> = pairs
                .iter()
                .map(|x| (x.uid.clone(), x.mid.clone()))
                .collect();
            (
                run_comments(
                    &conn,
                    &client,
                    &task,
                    &account.id,
                    &stored,
                    &p,
                    true,
                    rate,
                    app,
                    &cmd.task_id,
                )?,
                None,
            )
        }
        _ => {
            return Err(AppError::Internal(
                "任务类型与 weiboConfig 不匹配，无法采集".into(),
            ));
        }
    };

    Ok((n, list_hint))
}

/// 在阻塞线程中执行采集并向前端发事件；失败时将任务标为 `error`。
pub fn process_crawl_job(state: &AppState, app: &tauri::AppHandle, cmd: &CrawlCommand) {
    let task_id = cmd.task_id.clone();
    let r = run_weibo_crawl(state, cmd, app);
    match r {
        Ok((n, list_hint)) => {
            let message = if n == 0 {
                match list_hint {
                    Some(h) => format!(
                        "采集完成：未解析到微博条目（0 条）；列表多策略尝试见 crawl_requests.response_data.list_fetch_attempts。{h}"
                    ),
                    None => "采集完成：未写入微博条目；若为列表任务，请查看 crawl_requests 对应请求的 response_data。".into(),
                }
            } else {
                format!("采集完成，已写入 {n} 条到 records 表（含 json_data）")
            };
            let _ = app.emit(
                "crawl-progress",
                &CrawlProgressEvent {
                    task_id: task_id.clone(),
                    status: "done".into(),
                    message,
                },
            );
        }
        Err(e) => {
            let msg = e.to_string();
            let conn = state.db.conn();
            let _ = task_repo::update_status(&conn, &task_id, crate::model::task::TaskStatus::Error);
            let _ = app.emit(
                "crawl-progress",
                &CrawlProgressEvent {
                    task_id,
                    status: "error".into(),
                    message: msg,
                },
            );
        }
    }
}

// ---------------------------------------------------------------------------
//  Single-request executors (used by the RequestScheduler)
// ---------------------------------------------------------------------------

/// Result of executing a single `CrawlRequest`.
pub(crate) struct SingleRequestResult {
    /// Number of records inserted.
    pub records_inserted: usize,
    /// Brief metadata JSON for `crawl_requests.response_summary`.
    pub response_summary: Option<String>,
    /// Full parsed response content for `crawl_requests.response_data`.
    pub response_data: Option<String>,
    /// Derived child requests to insert (e.g. list page → body ids).
    pub derived_requests: Vec<CrawlRequest>,
}

/// Execute a single list-page request.
pub(crate) fn execute_list_page(
    conn: &Connection,
    task: &CrawlTask,
    client: &Client,
    stored: &WeiboStoredCookies,
    req: &CrawlRequest,
) -> Result<SingleRequestResult, AppError> {
    let params: serde_json::Value = serde_json::from_str(&req.request_params)?;
    let search_for = params["search_for"].as_str().unwrap_or("");
    let page = params["page"].as_i64().unwrap_or(1) as i32;
    let list_kind = params["list_kind"].as_str().unwrap_or("综合");
    let advanced_kind = params["advanced_kind"].as_str();
    let time_start = params["time_start"].as_str();
    let time_end = params["time_end"].as_str();

    let url = api::build_list_url(search_for, page, list_kind, advanced_kind, time_start, time_end)
        .map_err(AppError::Http)?;

    let prev = if page > 1 {
        Some(
            api::build_list_url(search_for, page - 1, list_kind, advanced_kind, time_start, time_end)
                .map_err(AppError::Http)?,
        )
    } else {
        None
    };

    let bound_account_id = req.account_id.as_deref().unwrap_or("");

    let log_ctx = request_log_repo::CrawlHttpLogCtx {
        conn,
        platform_tag: task.platform.as_tag(),
        task_id: &task.id,
        crawl_request_id: Some(req.id.as_str()),
        account_id: req.account_id.as_deref(),
        proxy_id: req.proxy_id.as_deref(),
    };
    let (html, final_url, list_fetch_attempts) = fetch_list_html_multi(
        Some(&log_ctx),
        bound_account_id,
        client,
        &url,
        stored,
        page,
        prev.as_ref(),
    )?;

    let items = parse_list_html(&html);
    if items.is_empty() && list_html_indicates_login_required(&html, &final_url) {
        return Err(AppError::LoginRequired(format!(
            "列表页跳登录: final_url={final_url}"
        )));
    }
    let mut total = 0usize;
    let mut derived_requests: Vec<CrawlRequest> = Vec::new();
    for item in &items {
        let preview = item.get("content_all").and_then(|x| x.as_str()).unwrap_or("");
        let author = item.get("personal_name").and_then(|x| x.as_str()).unwrap_or("");
        let feed_id = insert_record(
            conn,
            task,
            search_for,
            feed_blog_id_from_item(item).as_deref(),
            preview,
            author,
            item,
            None,
            Some("feed"),
        )?;
        total += 1;

        let comment_count = item.get("comment_num").and_then(|v| v.as_i64()).unwrap_or(0);
        if comment_count == 0 {
            continue;
        }

        if let Some((uid, mid)) = weibo_uid_mid_from_list_item(item) {
            let referer_path = format!("/{uid}/{mid}");
            let mblogid = item
                .get("mblogid")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let record_keyword = mblogid.map(String::from).unwrap_or_else(|| mid.clone());
            let params = serde_json::json!({
                "uid": uid,
                "mid": mid,
                "root_record_id": feed_id,
                "search_for": search_for,
                "referer_path": referer_path,
                "record_keyword": record_keyword,
            });
            derived_requests.push(new_pending_crawl_request(
                &task.id,
                CrawlRequestType::CommentL1,
                &params,
                Some(&req.id),
            ));
        }
    }

    let summary = serde_json::json!({
        "page": page,
        "final_url": final_url,
        "items_parsed": items.len(),
        "records_inserted": total,
        "derived_comment_l1": derived_requests.len(),
    });

    let data = serde_json::json!({
        "html": html,
        "parsed_items": items,
        "list_fetch_attempts": list_fetch_attempts,
    });

    Ok(SingleRequestResult {
        records_inserted: total,
        response_summary: Some(summary.to_string()),
        response_data: Some(data.to_string()),
        derived_requests,
    })
}

/// Execute a single body (detail page) request.
/// After inserting the body record, derives L1 comment requests when `comments_count > 0`.
pub(crate) fn execute_body(
    conn: &Connection,
    task: &CrawlTask,
    client: &Client,
    stored: &WeiboStoredCookies,
    req: &CrawlRequest,
) -> Result<SingleRequestResult, AppError> {
    let params: serde_json::Value = serde_json::from_str(&req.request_params)?;
    let status_id = params["status_id"].as_str().unwrap_or("");

    let (url, mut headers) = build_body_request(status_id);
    headers = merge_weibo_stored_cookies(headers, stored)?;
    let diag = ReqDiag::snapshot(&url, &headers);
    let log_ctx = request_log_repo::CrawlHttpLogCtx {
        conn,
        platform_tag: task.platform.as_tag(),
        task_id: &task.id,
        crawl_request_id: Some(req.id.as_str()),
        account_id: req.account_id.as_deref(),
        proxy_id: req.proxy_id.as_deref(),
    };
    let t0 = Instant::now();
    let resp = match client.get(url.as_str()).headers(headers).send() {
        Ok(r) => r,
        Err(e) => {
            let msg = fmt_reqwest_error(&e);
            log_crawl_http(
                Some(&log_ctx),
                "body",
                None,
                "GET",
                &url,
                None,
                Some(&msg),
                t0.elapsed().as_millis() as i64,
            );
            return Err(AppError::Network(msg));
        }
    };
    let status = resp.status();
    let code = status.as_u16();
    if !status.is_success() {
        diag.warn(&format!("正文 id={status_id}"), code);
        let body_excerpt = resp.text().ok().map(|t| html_excerpt(&t, 256)).unwrap_or_default();
        log_crawl_http(
            Some(&log_ctx),
            "body",
            None,
            "GET",
            &url,
            Some(code as i64),
            None,
            t0.elapsed().as_millis() as i64,
        );
        return Err(AppError::HttpStatus {
            code,
            body_excerpt: diag.embed(body_excerpt),
        });
    }
    let v: Value = match resp.json() {
        Ok(v) => v,
        Err(e) => {
            log_crawl_http(
                Some(&log_ctx),
                "body",
                None,
                "GET",
                &url,
                Some(code as i64),
                Some(&format!("正文 JSON: {e}")),
                t0.elapsed().as_millis() as i64,
            );
            return Err(AppError::Internal(format!("正文 JSON: {e}")));
        }
    };
    log_crawl_http(
        Some(&log_ctx),
        "body",
        None,
        "GET",
        &url,
        Some(code as i64),
        None,
        t0.elapsed().as_millis() as i64,
    );
    if let Some(err) = check_business_reject(&v) {
        return Err(err);
    }
    let preview = v.get("text").or_else(|| v.get("text_raw")).and_then(|x| x.as_str()).unwrap_or("");
    let author = v.pointer("/user/screen_name").and_then(|x| x.as_str()).unwrap_or("");
    let body_record_id = insert_record(
        conn,
        task,
        "",
        Some(status_id),
        preview,
        author,
        &v,
        None,
        Some("body"),
    )?;

    let mut derived_requests: Vec<CrawlRequest> = Vec::new();

    let comments_count = v.get("comments_count").and_then(|c| c.as_u64()).unwrap_or(0);
    if comments_count > 0 {
        let uid = v
            .pointer("/user/idstr")
            .and_then(|u| u.as_str())
            .or_else(|| v.pointer("/user/id").map(|u| if u.is_string() { u.as_str().unwrap_or("") } else { "" }))
            .filter(|s| !s.is_empty());
        let mid = v
            .get("mid")
            .or_else(|| v.get("id"))
            .map(|m| if let Some(s) = m.as_str() { s.to_string() } else { m.to_string() })
            .filter(|s| !s.is_empty());

        if let (Some(uid), Some(mid)) = (uid, mid) {
            let referer_path = format!("/{uid}/{mid}");
            let record_keyword = v
                .get("mblogid")
                .and_then(|x| x.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| mid.clone());
            let l1_params = serde_json::json!({
                "uid": uid,
                "mid": mid,
                "root_record_id": body_record_id,
                "referer_path": referer_path,
                "record_keyword": record_keyword,
            });
            derived_requests.push(new_pending_crawl_request(
                &task.id,
                CrawlRequestType::CommentL1,
                &l1_params,
                Some(&req.id),
            ));
        }
    }

    let summary = serde_json::json!({
        "status_id": status_id,
        "author": author,
        "comments_count": comments_count,
        "derived_comment_l1": derived_requests.len(),
    });

    let data_str = serde_json::to_string(&v).ok();

    Ok(SingleRequestResult {
        records_inserted: 1,
        response_summary: Some(summary.to_string()),
        response_data: data_str,
        derived_requests,
    })
}

/// 从评论 JSON 对象提取该评论自身的 mid（字符串形式）。
/// 评论类记录写入 `records.blog_id`（请求参数里仍叫 `record_keyword`）：优先 `mblogid`，否则 `mid`。
fn record_keyword_from_comment_params(params: &Value, api_mid: &str) -> String {
    if let Some(s) = params
        .get("record_keyword")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return s.to_string();
    }
    if let Some(s) = params
        .get("mblogid")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return s.to_string();
    }
    api_mid.to_string()
}

fn comment_mid_string(c: &Value) -> Option<String> {
    if let Some(s) = c.get("mid").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
        return Some(s.to_string());
    }
    c.get("mid").and_then(|v| {
        if v.is_number() {
            Some(v.to_string())
        } else {
            None
        }
    })
}

const MAX_COMMENT_FAILED_PAGES: usize = 20;

/// Execute a single comment (L1 or L2) request **with pagination**.
///
/// Pagination mirrors WeiBoCrawler:
///   - first page: no `max_id`
///   - response root contains `max_id`, `total_number`, `data[]`
///   - keep requesting with `max_id` until `count >= total_number` or consecutive empty pages
///
/// L2 param fix (per diagram):
///   - `uid` = feed author uid (carried as `uid` in request_params)
///   - `id` (mid in API) = L1 comment's own `mid` — **one L2 request per L1 comment**
pub(crate) fn execute_comment(
    conn: &Connection,
    task: &CrawlTask,
    client: &Client,
    stored: &WeiboStoredCookies,
    req: &CrawlRequest,
    level2: bool,
    page_delay: Duration,
) -> Result<SingleRequestResult, AppError> {
    let params: serde_json::Value = serde_json::from_str(&req.request_params)?;
    let uid = params["uid"].as_str().unwrap_or("");
    let mid = params["mid"].as_str().unwrap_or("");
    let referer = params
        .get("referer_path")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("/{uid}/{mid}"));

    let root_record_id = params.get("root_record_id").and_then(|v| v.as_str());

    let l1_parent_record_id = if level2 {
        params.get("l1_record_id").and_then(|v| v.as_str())
    } else {
        None
    };

    let record_kw = record_keyword_from_comment_params(&params, mid);
    let kw_search = params
        .get("search_for")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut total = 0usize;
    let mut collected_items: Vec<Value> = Vec::new();
    let mut derived_requests: Vec<CrawlRequest> = Vec::new();
    let mut l1_cid_to_record: HashMap<String, String> = HashMap::new();

    let mut current_max_id: Option<String> = None;
    let mut count_data_number: usize = 0;
    let mut api_total_number: usize = 0;
    let mut failed_pages: usize = 0;
    let mut is_first_page = true;

    let log_ctx = request_log_repo::CrawlHttpLogCtx {
        conn,
        platform_tag: task.platform.as_tag(),
        task_id: &task.id,
        crawl_request_id: Some(req.id.as_str()),
        account_id: req.account_id.as_deref(),
        proxy_id: req.proxy_id.as_deref(),
    };
    let kind = if level2 { "comment_l2" } else { "comment_l1" };

    loop {
        let page_start = Instant::now();
        let (url, headers) = if level2 {
            build_comment_l2_request(uid, mid, current_max_id.as_deref(), &referer)
        } else {
            build_comment_l1_request(uid, mid, current_max_id.as_deref(), &referer)
        };
        let headers = merge_weibo_stored_cookies(headers, stored)?;
        let diag = ReqDiag::snapshot(&url, &headers);
        let phase = current_max_id.as_deref();
        let t0 = Instant::now();
        let resp = match client.get(url.as_str()).headers(headers).send() {
            Ok(r) => r,
            Err(e) => {
                let msg = fmt_reqwest_error(&e);
                log_crawl_http(
                    Some(&log_ctx),
                    kind,
                    phase,
                    "GET",
                    &url,
                    None,
                    Some(&msg),
                    t0.elapsed().as_millis() as i64,
                );
                return Err(AppError::Network(msg));
            }
        };
        let status = resp.status();
        let code = status.as_u16();
        if !status.is_success() {
            let level_label = if level2 { "二级评论" } else { "一级评论" };
            diag.warn(
                &format!("{level_label} uid={uid} mid={mid}"),
                code,
            );
            log_crawl_http(
                Some(&log_ctx),
                kind,
                phase,
                "GET",
                &url,
                Some(code as i64),
                None,
                t0.elapsed().as_millis() as i64,
            );
            if is_first_page {
                let body_excerpt = resp.text().ok().map(|t| html_excerpt(&t, 256)).unwrap_or_default();
                return Err(AppError::HttpStatus {
                    code,
                    body_excerpt: diag.embed(body_excerpt),
                });
            }
            failed_pages += 1;
            if failed_pages >= MAX_COMMENT_FAILED_PAGES {
                break;
            }
            continue;
        }
        is_first_page = false;

        let root: Value = match resp.json() {
            Ok(v) => v,
            Err(e) => {
                log_crawl_http(
                    Some(&log_ctx),
                    kind,
                    phase,
                    "GET",
                    &url,
                    Some(code as i64),
                    Some(&format!("评论 JSON: {e}")),
                    t0.elapsed().as_millis() as i64,
                );
                return Err(AppError::Internal(format!("评论 JSON: {e}")));
            }
        };
        log_crawl_http(
            Some(&log_ctx),
            kind,
            phase,
            "GET",
            &url,
            Some(code as i64),
            None,
            t0.elapsed().as_millis() as i64,
        );
        if let Some(err) = check_business_reject(&root) {
            return Err(err);
        }

        let resp_max_id = root
            .get("max_id")
            .map(|v| if let Some(s) = v.as_str() { s.to_string() } else { v.to_string() });
        let resp_total = root.get("total_number").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if resp_total > api_total_number {
            api_total_number = resp_total;
        }

        let arr = root.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();
        let page_n = arr.len();

        if page_n == 0 {
            failed_pages += 1;
            if failed_pages >= MAX_COMMENT_FAILED_PAGES {
                break;
            }
            if let Some(ref m) = resp_max_id {
                current_max_id = Some(m.clone());
            } else {
                break;
            }
            continue;
        }
        failed_pages = 0;

        for mut item in arr {
            if let Some(obj) = item.as_object_mut() {
                obj.insert("f_uid".into(), Value::String(uid.to_string()));
                obj.insert("f_mid".into(), Value::String(mid.to_string()));
            }
            let preview = item
                .get("text")
                .or_else(|| item.get("text_raw"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let author = item
                .pointer("/user/screen_name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();

            if level2 {
                let parent = l1_parent_record_id
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| root_record_id.map(str::to_string));
                insert_record(
                    conn,
                    task,
                    kw_search,
                    Some(record_kw.as_str()),
                    preview,
                    &author,
                    &item,
                    parent.as_deref(),
                    Some("comment_l2"),
                )?;
            } else {
                let rid = insert_record(
                    conn,
                    task,
                    kw_search,
                    Some(record_kw.as_str()),
                    preview,
                    &author,
                    &item,
                    root_record_id,
                    Some("comment_l1"),
                )?;
                let cid = comment_id_string(&item);
                let comment_mid = comment_mid_string(&item);
                let comment_total = item
                    .get("total_number")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if let Some(ref c) = cid {
                    l1_cid_to_record.insert(c.clone(), rid.clone());
                }
                if comment_total > 0 {
                    if let Some(cm) = comment_mid {
                        let mut l2_params = serde_json::json!({
                            "uid": uid,
                            "mid": cm,
                            "root_record_id": root_record_id.unwrap_or(""),
                            "l1_record_id": rid,
                            "referer_path": referer,
                            "record_keyword": record_kw.clone(),
                        });
                        if let Some(sf) = params.get("search_for") {
                            l2_params["search_for"] = sf.clone();
                        }
                        derived_requests.push(new_pending_crawl_request(
                            &task.id,
                            CrawlRequestType::CommentL2,
                            &l2_params,
                            Some(&req.id),
                        ));
                    }
                }
            }
            collected_items.push(item);
            total += 1;
        }

        count_data_number += page_n;
        if count_data_number >= api_total_number {
            break;
        }
        match resp_max_id {
            Some(ref m) if !m.is_empty() && m != "0" => {
                current_max_id = Some(m.clone());
            }
            _ => break,
        }
        rate_limit_sleep(page_delay, page_start.elapsed());
    }

    let label = if level2 { "comment_l2" } else { "comment_l1" };
    let summary = serde_json::json!({
        "type": label,
        "uid": uid,
        "mid": mid,
        "records_inserted": total,
        "api_total_number": api_total_number,
        "pages_fetched": count_data_number,
        "derived_requests": derived_requests.len(),
    });

    let data = serde_json::to_string(&collected_items).ok();

    Ok(SingleRequestResult {
        records_inserted: total,
        response_summary: Some(summary.to_string()),
        response_data: data,
        derived_requests,
    })
}

/// Dispatch a single `CrawlRequest` – called by the scheduler.
pub(crate) fn execute_single_request(
    conn: &Connection,
    task: &CrawlTask,
    client: &Client,
    stored: &WeiboStoredCookies,
    req: &CrawlRequest,
    rate_limit: i64,
) -> Result<SingleRequestResult, AppError> {
    let page_delay = Duration::from_millis((60_000 / rate_limit.max(1)) as u64);
    match req.request_type {
        CrawlRequestType::ListPage => execute_list_page(conn, task, client, stored, req),
        CrawlRequestType::Body => execute_body(conn, task, client, stored, req),
        CrawlRequestType::CommentL1 => execute_comment(conn, task, client, stored, req, false, page_delay),
        CrawlRequestType::CommentL2 => execute_comment(conn, task, client, stored, req, true, page_delay),
    }
}
