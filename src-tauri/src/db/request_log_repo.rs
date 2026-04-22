//! 网络请求日志：`logs` 表。写入失败不得影响采集主流程。

use rusqlite::{params, Connection};

use crate::error::AppError;

const MAX_URL_CHARS: usize = 4096;
const MAX_ERR_CHARS: usize = 2000;
const MAX_ROWS: i64 = 80_000;
const TRIM_BATCH: i64 = 15_000;

/// 单次 HTTP 记录所需的任务上下文（持久队列或一次性任务均可）。
#[derive(Clone, Copy)]
pub struct CrawlHttpLogCtx<'a> {
    pub conn: &'a Connection,
    pub platform_tag: &'a str,
    pub task_id: &'a str,
    pub crawl_request_id: Option<&'a str>,
    pub account_id: Option<&'a str>,
    pub proxy_id: Option<&'a str>,
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

fn maybe_trim(conn: &Connection) {
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM logs", [], |r| r.get(0))
        .unwrap_or(0);
    if n <= MAX_ROWS {
        return;
    }
    let _ = conn.execute(
        "DELETE FROM logs WHERE id IN (
            SELECT id FROM logs ORDER BY id ASC LIMIT ?
        )",
        [TRIM_BATCH],
    );
}

pub fn try_insert(
    ctx: Option<&CrawlHttpLogCtx<'_>>,
    request_kind: &str,
    phase: Option<&str>,
    method: &str,
    url: &str,
    status_code: Option<i64>,
    error_message: Option<&str>,
    duration_ms: i64,
) {
    let Some(c) = ctx else {
        return;
    };
    let occurred_at = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let url = truncate_chars(url, MAX_URL_CHARS);
    let err = error_message.map(|e| truncate_chars(e, MAX_ERR_CHARS));
    let res = c.conn.execute(
        "INSERT INTO logs (occurred_at, platform, task_id, crawl_request_id, account_id, \
         proxy_id, request_kind, phase, method, url, status_code, duration_ms, error_message) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            occurred_at,
            c.platform_tag,
            c.task_id,
            c.crawl_request_id,
            c.account_id,
            c.proxy_id,
            request_kind,
            phase,
            method,
            url,
            status_code,
            duration_ms,
            err,
        ],
    );
    if let Err(e) = res {
        log::warn!("[logs] insert failed: {e}");
        return;
    }
    maybe_trim(c.conn);
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogListItem {
    pub id: i64,
    pub time: String,
    pub platform: String,
    pub task_id: Option<String>,
    pub task_name: Option<String>,
    pub crawl_request_id: Option<String>,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub proxy_id: Option<String>,
    pub proxy_address: Option<String>,
    pub request_kind: String,
    pub phase: Option<String>,
    pub method: String,
    pub url: String,
    pub status_code: Option<i64>,
    pub duration_ms: i64,
    pub error_message: Option<String>,
}

pub fn list_recent(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<RequestLogListItem>, AppError> {
    let lim = limit.clamp(1, 500);
    let off = offset.max(0);
    let mut stmt = conn.prepare(
        "SELECT l.id, l.occurred_at, l.platform, l.task_id, l.crawl_request_id, l.account_id, \
         l.proxy_id, l.request_kind, l.phase, l.method, l.url, l.status_code, l.duration_ms, \
         l.error_message, t.name AS task_name, a.username AS account_name, p.address AS proxy_address \
         FROM logs l \
         LEFT JOIN tasks t ON t.id = l.task_id \
         LEFT JOIN accounts a ON a.id = l.account_id \
         LEFT JOIN proxies p ON p.id = l.proxy_id \
         ORDER BY l.id DESC LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt.query_map(params![lim, off], |r| {
        Ok(RequestLogListItem {
            id: r.get(0)?,
            time: r.get(1)?,
            platform: r.get(2)?,
            task_id: r.get(3)?,
            crawl_request_id: r.get(4)?,
            account_id: r.get(5)?,
            proxy_id: r.get(6)?,
            request_kind: r.get(7)?,
            phase: r.get(8)?,
            method: r.get(9)?,
            url: r.get(10)?,
            status_code: r.get(11)?,
            duration_ms: r.get(12)?,
            error_message: r.get(13)?,
            task_name: r.get(14)?,
            account_name: r.get(15)?,
            proxy_address: r.get(16)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn count(conn: &Connection) -> Result<i64, AppError> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM logs", [], |r| r.get(0))?;
    Ok(n)
}

pub fn clear_all(conn: &Connection) -> Result<(), AppError> {
    conn.execute("DELETE FROM logs", [])?;
    Ok(())
}
