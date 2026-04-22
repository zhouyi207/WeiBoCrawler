//! 登录成功后携带 Cookie 访问 `https://my.sina.com.cn/`，解析昵称与 `CENTER_CONFIG.uid`。

use std::path::Path;

use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, REFERER};
use scraper::{Html, Selector};

use crate::db::{account_repo, weibo_account_repo};
use crate::error::AppError;
use rusqlite::Connection;

const MY_SINA_URL: &str = "https://my.sina.com.cn/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 Edg/147.0.0.0";

/// 调试：控制台查看拉取与解析结果（`tauri dev` 终端可见）。
fn preview_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    let head: String = s.chars().take(max_chars).collect();
    if count > max_chars {
        format!("{head}… (共 {count} 字符，已截断)")
    } else {
        head
    }
}

fn debug_print_html_summary(account_id: &str, html: &str) {
    println!(
        "[weibo my.sina] account_id={account_id} url={MY_SINA_URL} html_char_len={}",
        html.chars().count()
    );
    println!(
        "[weibo my.sina] html_preview (前 1200 字符):\n{}",
        preview_chars(html, 1200)
    );
    if let Some(i) = html.find("CENTER_CONFIG") {
        let start = i.saturating_sub(20);
        println!(
            "[weibo my.sina] CENTER_CONFIG 附近片段:\n{}",
            preview_chars(&html[start..], 900)
        );
    } else {
        println!("[weibo my.sina] 未在 HTML 中找到字面量 CENTER_CONFIG");
    }
    if let Some(i) = html.find("me_name") {
        let start = i.saturating_sub(80);
        println!(
            "[weibo my.sina] 含 me_name 的片段:\n{}",
            preview_chars(&html[start..], 500)
        );
    } else {
        println!("[weibo my.sina] 未在 HTML 中找到 me_name");
    }
}

fn debug_print_parsed(account_id: &str, p: &ParsedMySina) {
    println!(
        "[weibo my.sina] 解析结果 account_id={account_id} display_name={:?} uid={:?} center_weibo_name={:?}",
        p.display_name, p.uid, p.center_weibo_name
    );
}

/// 与浏览器访问文档页一致的请求头（不含 Cookie，由带 `cookie_store` 的 Client 自动携带）。
fn my_sina_document_headers_browser_like() -> HeaderMap {
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
    h.insert("priority", HeaderValue::from_static("u=0, i"));
    h.insert(
        REFERER,
        HeaderValue::from_static("https://passport.weibo.com/"),
    );
    h.insert(
        "sec-ch-ua",
        HeaderValue::from_static(
            "\"Microsoft Edge\";v=\"147\", \"Not.A/Brand\";v=\"8\", \"Chromium\";v=\"147\"",
        ),
    );
    h.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
    h.insert("sec-ch-ua-platform", HeaderValue::from_static("\"Windows\""));
    h.insert("sec-fetch-dest", HeaderValue::from_static("document"));
    h.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
    h.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
    h.insert("upgrade-insecure-requests", HeaderValue::from_static("1"));
    h.insert(
        reqwest::header::USER_AGENT,
        HeaderValue::from_static(USER_AGENT),
    );
    h
}

pub struct ParsedMySina {
    pub display_name: Option<String>,
    pub uid: Option<String>,
    pub center_weibo_name: Option<String>,
}

/// 使用**扫码登录同一 `reqwest::Client`**（已开启 `cookie_store`）请求个人中心。
/// 若用「仅存最后一次响应里的 Cookie JSON + 新 Client 手动拼头」，会丢失登录过程中
/// 写入 Cookie 罐的跨域条目，服务器会返回未登录落地页（与浏览器不一致）。
pub fn fetch_my_sina_html_with_session_client(client: &Client) -> Result<String, AppError> {
    let headers = my_sina_document_headers_browser_like();
    let resp = client
        .get(MY_SINA_URL)
        .headers(headers)
        .send()
        .map_err(|e| AppError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(AppError::Http(format!(
            "my.sina.com.cn HTTP {}",
            resp.status()
        )));
    }
    resp.text()
        .map_err(|e| AppError::Http(format!("read body: {e}")))
}

fn parse_me_name(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let primary = Selector::parse(
        "body > div.wrap.body_wrap.clearfix > div.wrap_l > div.me_w > p.me_name",
    )
    .ok()?;
    if let Some(el) = doc.select(&primary).next() {
        let t = el.text().collect::<Vec<_>>().join("").trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    let fallback = Selector::parse("div.me_w p.me_name").ok()?;
    doc.select(&fallback)
        .next()
        .map(|el| el.text().collect::<Vec<_>>().join("").trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_center_config(html: &str) -> (Option<String>, Option<String>) {
    // 注意：`CENTER_CONFIG` 内可能含嵌套 `{ }`，不能用单次 `[^}]*` 截断。
    let block_re = match Regex::new(
        r"(?s)CENTER_CONFIG\s*=\s*\{",
    ) {
        Ok(r) => r,
        Err(_) => return (None, None),
    };
    let Some(m) = block_re.find(html) else {
        println!("[weibo my.sina] parse_center_config: 正则无匹配（无 CENTER_CONFIG = {{）");
        return (None, None);
    };
    let after_brace = m.end();
    let rest = &html[after_brace..];
    let mut depth = 1usize;
    let mut end = 0usize;
    for (i, ch) in rest.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        println!(
            "[weibo my.sina] parse_center_config: 大括号未闭合 depth={depth}，取前 400 字符尝试子正则"
        );
        return (None, None);
    }
    let block = &rest[..end];

    let uid = Regex::new(r#"uid:\s*['"](\d+)['"]"#)
        .ok()
        .and_then(|re| {
            re.captures(block)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        });

    let weibo_name = Regex::new(r#"weibo_name:\s*['"]([^'"]*)['"]"#)
        .ok()
        .and_then(|re| {
            re.captures(block)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        })
        .filter(|s| !s.is_empty());

    println!(
        "[weibo my.sina] parse_center_config: block_len={} block_preview={:?}",
        block.chars().count(),
        preview_chars(block, 400)
    );

    (uid, weibo_name)
}

pub fn parse_my_sina_page(html: &str) -> ParsedMySina {
    let display_name = parse_me_name(html);
    let (uid, center_weibo_name) = parse_center_config(html);
    ParsedMySina {
        display_name,
        uid,
        center_weibo_name,
    }
}

/// 将完整 HTML 写入应用数据目录下的 `debug/weibo_my_sina/`，可用浏览器直接打开检查。
fn save_my_sina_html_dump(dir: &Path, account_id: &str, html: &str) -> Result<std::path::PathBuf, std::io::Error> {
    std::fs::create_dir_all(dir)?;
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let safe_id: String = account_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let file_name = format!("my_sina_{safe_id}_{ts}.html");
    let path = dir.join(file_name);
    std::fs::write(&path, html.as_bytes())?;
    Ok(path)
}

/// 写入 `weibo_account_profiles` 并更新 `accounts.username`（解析成功时）。
/// `session_client` 须为扫码流程中的同一 `Client`，以保证 Cookie 罐与浏览器一致。
/// `html_dump_dir` 若提供，则将完整响应 HTML 保存为 `.html` 便于本地检查。
/// 若与已有账号为同一微博 uid，合并到旧账号并删除新行，返回 `Some(保留的 account_id)`。
pub fn enrich_account_from_my_sina_session(
    conn: &Connection,
    account_id: &str,
    session_client: &Client,
    html_dump_dir: Option<&Path>,
) -> Result<Option<String>, AppError> {
    let html = fetch_my_sina_html_with_session_client(session_client)?;

    if let Some(dir) = html_dump_dir {
        match save_my_sina_html_dump(dir, account_id, &html) {
            Ok(path) => {
                println!(
                    "[weibo my.sina] 网页已保存到本地，可用浏览器打开: {}",
                    path.display()
                );
            }
            Err(e) => {
                log::warn!("[weibo my.sina] 保存 HTML 到本地失败: {e}");
            }
        }
    }

    debug_print_html_summary(account_id, &html);
    let parsed = parse_my_sina_page(&html);
    debug_print_parsed(account_id, &parsed);

    let platform: String = conn.query_row(
        "SELECT platform FROM accounts WHERE id = ?1",
        [account_id],
        |r| r.get(0),
    )?;

    if platform == "weibo" {
        if let Some(ref uid) = parsed.uid.clone().filter(|u| !u.is_empty()) {
            if let Some(existing_id) = weibo_account_repo::find_account_id_by_weibo_uid(conn, uid)? {
                if existing_id != account_id {
                    account_repo::merge_new_weibo_account_into_existing(conn, account_id, &existing_id)?;
                    if let Some(ref name) = parsed.display_name {
                        if !name.trim().is_empty() {
                            account_repo::update_username(conn, &existing_id, name.trim())?;
                        }
                    }
                    weibo_account_repo::upsert(
                        conn,
                        &existing_id,
                        uid,
                        parsed.center_weibo_name.as_deref(),
                    )?;
                    println!(
                        "[weibo my.sina] 重复微博 uid={uid}，已合并到已有账号 {existing_id}，已删除新行 {account_id}"
                    );
                    return Ok(Some(existing_id));
                }
            }
        }
    }

    if let Some(ref name) = parsed.display_name {
        if !name.trim().is_empty() {
            account_repo::update_username(conn, account_id, name.trim())?;
        }
    }

    if let Some(ref uid) = parsed.uid {
        if !uid.is_empty() {
            let center = parsed.center_weibo_name.as_deref();
            weibo_account_repo::upsert(conn, account_id, uid, center)?;
        }
    }

    Ok(None)
}
