//! 对齐 `WeiBoCrawler/request/get_cookies.py`：`signin` → `qrcode/image` → 下载二维码 → 轮询 `qrcode/check` → `login_url` 最终请求。

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::blocking::Client;
use reqwest::cookie::{CookieStore, Jar};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, REFERER};
use reqwest::redirect::Policy;
use reqwest::Proxy;
use reqwest::Url;
use serde::Deserialize;

use crate::error::AppError;
use crate::model::account::WeiboQrPollResponse;
use crate::weibo::session::WeiboLoginSession;

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0";

const WEIBO_LOGIN_URL: &str = "https://weibo.com/newlogin?tabtype=weibo&gid=102803&openLoginLayer=0&url=https%3A%2F%2Fweibo.com%2F";

/// 构建带 Cookie 存储与可选代理的客户端（Python 侧为 `httpx.Client(follow_redirects=True)`）。
/// 返回共享的 [`Jar`]，便于登录成功后序列化**完整** Cookie 写入数据库。
pub fn build_weibo_client(
    proxy_address: Option<&str>,
    proxy_type: &str,
) -> Result<(Client, Arc<Jar>), AppError> {
    let jar = Arc::new(Jar::default());
    let mut b = Client::builder()
        .cookie_provider(jar.clone())
        .user_agent(USER_AGENT)
        .redirect(Policy::limited(32));

    // 与 `crawl::http_client_with_proxy` 一致：`Direct`（如系统行 `local-direct`）为伪代理，
    // `address` 仅为展示文案（如「本机直连」），不能当作 `http://…` 主机名。
    if let Some(addr) = proxy_address.filter(|s| !s.is_empty()) {
        let pt = proxy_type.to_ascii_uppercase();
        if pt != "DIRECT" {
            let url = normalize_proxy_url(addr, proxy_type)?;
            b = b.proxy(Proxy::all(&url).map_err(|e| AppError::Http(e.to_string()))?);
        }
    }

    let client = b
        .build()
        .map_err(|e| AppError::Http(format!("failed to build HTTP client: {e}")))?;
    Ok((client, jar))
}

fn normalize_proxy_url(address: &str, proxy_type: &str) -> Result<String, AppError> {
    let t = proxy_type.to_ascii_uppercase();
    if address.contains("://") {
        return Ok(address.to_string());
    }
    match t.as_str() {
        "SOCKS5" => Ok(format!("socks5://{address}")),
        "HTTP" | _ => Ok(format!("http://{address}")),
    }
}

fn signin_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        ),
    );
    h.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6"),
    );
    h.insert("cache-control", HeaderValue::from_static("max-age=0"));
    h.insert("priority", HeaderValue::from_static("u=0, i"));
    h.insert(
        "sec-ch-ua",
        HeaderValue::from_static("\"Not A(Brand\";v=\"8\", \"Chromium\";v=\"132\", \"Microsoft Edge\";v=\"132\""),
    );
    h.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
    h.insert("sec-ch-ua-platform", HeaderValue::from_static("\"Windows\""));
    h.insert("sec-fetch-dest", HeaderValue::from_static("document"));
    h.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
    h.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
    h.insert("sec-fetch-user", HeaderValue::from_static("?1"));
    h.insert("upgrade-insecure-requests", HeaderValue::from_static("1"));
    h
}

fn xhr_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(ACCEPT, HeaderValue::from_static("application/json, text/plain, */*"));
    h.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6"),
    );
    h.insert("priority", HeaderValue::from_static("u=1, i"));
    h.insert(
        "sec-ch-ua",
        HeaderValue::from_static("\"Not A(Brand\";v=\"8\", \"Chromium\";v=\"132\", \"Microsoft Edge\";v=\"132\""),
    );
    h.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
    h.insert("sec-ch-ua-platform", HeaderValue::from_static("\"Windows\""));
    h.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
    h.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
    h.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
    h.insert("x-requested-with", HeaderValue::from_static("XMLHttpRequest"));
    h
}

/// 从 `Set-Cookie` 头解析 `X-CSRF-TOKEN`（与 Python `client.cookies.get("X-CSRF-TOKEN")` 一致）。
fn csrf_from_set_cookie(headers: &HeaderMap) -> Option<String> {
    for v in headers.get_all(reqwest::header::SET_COOKIE) {
        let s = v.to_str().ok()?;
        for part in s.split(',') {
            let p = part.trim();
            if let Some(rest) = p.strip_prefix("X-CSRF-TOKEN=") {
                let token = rest.split(';').next()?.trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct QrImageBody {
    data: Option<QrData>,
}

#[derive(Debug, Deserialize)]
struct QrData {
    qrid: Option<String>,
    image: Option<String>,
}

/// 对应 Python `get_qr_Info`：拿到二维码图与 `qrid`，并保留 `Client` + `login_signin_url` 供后续轮询。
pub fn request_weibo_login_qr(
    proxy_address: Option<&str>,
    proxy_type: &str,
) -> Result<(WeiboLoginSession, String), AppError> {
    let (client, cookie_jar) = build_weibo_client(proxy_address, proxy_type)?;

    let signin_resp = client
        .get("https://passport.weibo.com/sso/signin")
        .query(&[
            ("entry", "miniblog"),
            ("source", "miniblog"),
            ("disp", "popup"),
            ("url", WEIBO_LOGIN_URL),
            ("from", "weibopro"),
        ])
        .headers(signin_headers())
        .send()
        .map_err(|e| AppError::Http(e.to_string()))?;

    let csrf_signin = csrf_from_set_cookie(signin_resp.headers());
    let login_signin_url = signin_resp.url().to_string();
    let _ = signin_resp.text(); // 消费响应体，保证 Cookie 写入

    let mut q_headers = xhr_headers();
    q_headers.insert(
        REFERER,
        HeaderValue::from_str(&login_signin_url)
            .map_err(|e| AppError::Http(e.to_string()))?,
    );
    if let Some(ref csrf) = csrf_signin {
        q_headers.insert(
            "x-csrf-token",
            HeaderValue::from_str(csrf).map_err(|e| AppError::Http(e.to_string()))?,
        );
    }

    let qr_resp = client
        .get("https://passport.weibo.com/sso/v2/qrcode/image")
        .query(&[("entry", "miniblog"), ("size", "180")])
        .headers(q_headers)
        .send()
        .map_err(|e| AppError::Http(e.to_string()))?;

    let qr_body: QrImageBody = qr_resp.json().map_err(|e| AppError::Http(e.to_string()))?;
    let data = qr_body
        .data
        .ok_or_else(|| AppError::Internal("weibo qrcode: missing data".into()))?;
    let qrid = data
        .qrid
        .ok_or_else(|| AppError::Internal("weibo qrcode: missing qrid".into()))?;
    let image_url = data
        .image
        .ok_or_else(|| AppError::Internal("weibo qrcode: missing image url".into()))?;

    let img_bytes = client
        .get(&image_url)
        .send()
        .map_err(|e| AppError::Http(e.to_string()))?
        .bytes()
        .map_err(|e| AppError::Http(e.to_string()))?;

    let qr_data_url = bytes_to_data_url(&img_bytes);

    Ok((
        WeiboLoginSession {
            client,
            cookie_jar,
            login_signin_url,
            qrid,
            csrf_token: csrf_signin,
        },
        qr_data_url,
    ))
}

fn bytes_to_data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        "image/jpeg"
    } else {
        "image/png"
    };
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

/// 单次轮询，对应 Python `get_login_check_response` + 一条 `retcode` 分支（前端可定时调用）。
pub fn poll_weibo_qr_once(session: &WeiboLoginSession) -> Result<WeiboQrPollResponse, AppError> {
    let mut headers = xhr_headers();
    headers.insert(
        REFERER,
        HeaderValue::from_str(&session.login_signin_url)
            .map_err(|e| AppError::Http(e.to_string()))?,
    );

    if let Some(ref csrf) = session.csrf_token {
        headers.insert(
            "x-csrf-token",
            HeaderValue::from_str(csrf).map_err(|e| AppError::Http(e.to_string()))?,
        );
    }

    let check_resp = session
        .client
        .get("https://passport.weibo.com/sso/v2/qrcode/check")
        .query(&[
            ("entry", "miniblog"),
            ("source", "miniblog"),
            ("url", WEIBO_LOGIN_URL),
            ("qrid", &session.qrid),
            ("disp", "popup"),
        ])
        .headers(headers)
        .send()
        .map_err(|e| AppError::Http(e.to_string()))?;

    let v: serde_json::Value = check_resp
        .json()
        .map_err(|e| AppError::Http(e.to_string()))?;

    let retcode = v
        .get("retcode")
        .and_then(|x| x.as_i64().or_else(|| x.as_u64().map(|u| u as i64)))
        .unwrap_or(-1);

    match retcode {
        20000000 => {
            let login_url = v
                .pointer("/data/url")
                .and_then(|x| x.as_str())
                .ok_or_else(|| AppError::Internal("weibo check: missing data.url".into()))?;

            let final_resp = session
                .client
                .get(login_url)
                .send()
                .map_err(|e| AppError::Http(e.to_string()))?;

            // 优先从共享 Cookie 罐导出（含登录全流程各域），避免仅最后一次响应 Set-Cookie 为空导致 `{}`
            let cookies_json = cookies_json_from_jar(&session.cookie_jar)?;
            let cookies_json = if cookies_json.len() <= 2 {
                cookies_json_from_response(&final_resp)?
            } else {
                cookies_json
            };

            Ok(WeiboQrPollResponse {
                status: "success".into(),
                message: None,
                cookies: Some(cookies_json),
                merged_into_account_id: None,
            })
        }
        50114001 | 50114002 => {
            let msg = v
                .get("msg")
                .and_then(|m| m.as_str())
                .map(String::from);
            Ok(WeiboQrPollResponse {
                status: "waiting".into(),
                message: msg,
                cookies: None,
                merged_into_account_id: None,
            })
        }
        _ => {
            let msg = v
                .get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown retcode")
                .to_string();
            Ok(WeiboQrPollResponse {
                status: "failed".into(),
                message: Some(msg),
                cookies: None,
                merged_into_account_id: None,
            })
        }
    }
}

/// 按新浪系常用域聚合 `Cookie` 请求头中的 `name=value`，合并为 JSON 对象写入 `accounts.cookies`。
pub(crate) fn cookies_json_from_jar(jar: &Jar) -> Result<String, AppError> {
    /// 顺序影响同名 Cookie 的覆盖：靠后的域覆盖靠前的。`s.weibo.com` 放最后，避免被 `sina.com.cn` 等
    /// 合并结果冲掉搜索域所需的 `SUB` / `XSRF-TOKEN` 等。
    const URLS: &[&str] = &[
        "https://passport.weibo.com/",
        "https://login.sina.com.cn/",
        "https://weibo.com/",
        "https://www.weibo.com/",
        "https://my.sina.com.cn/",
        "https://sina.com.cn/",
        "https://s.weibo.com/",
    ];
    let mut map = serde_json::Map::new();
    for s in URLS {
        let url = Url::parse(s).map_err(|e| AppError::Internal(format!("cookie jar url: {e}")))?;
        if let Some(hv) = CookieStore::cookies(jar, &url) {
            let raw = hv
                .to_str()
                .map_err(|_| AppError::Internal("cookie header not utf-8".into()))?;
            merge_cookie_header_pairs(raw, &mut map);
        }
    }
    serde_json::to_string(&serde_json::Value::Object(map)).map_err(Into::into)
}

fn merge_cookie_header_pairs(header_value: &str, map: &mut serde_json::Map<String, serde_json::Value>) {
    for part in header_value.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((name, value)) = part.split_once('=') {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            map.insert(
                name.to_string(),
                serde_json::Value::String(value.trim().to_string()),
            );
        }
    }
}

fn cookies_json_from_response(resp: &reqwest::blocking::Response) -> Result<String, AppError> {
    let mut map = serde_json::Map::new();
    for c in resp.cookies() {
        map.insert(
            c.name().to_string(),
            serde_json::Value::String(c.value().to_string()),
        );
    }
    if map.is_empty() {
        for h in resp.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(s) = h.to_str() {
                if let Some((name, value)) = parse_set_cookie_pair(s) {
                    map.insert(name, serde_json::Value::String(value));
                }
            }
        }
    }
    serde_json::to_string(&serde_json::Value::Object(map)).map_err(Into::into)
}

fn parse_set_cookie_pair(header: &str) -> Option<(String, String)> {
    let first = header.split(';').next()?.trim();
    let (name, value) = first.split_once('=')?;
    Some((name.trim().to_string(), value.trim().to_string()))
}
