use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::model::crawl_request::{CrawlRequest, CrawlRequestStatus, CrawlRequestType, TaskProgress};

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn status_str(s: CrawlRequestStatus) -> &'static str {
    match s {
        CrawlRequestStatus::Pending => "pending",
        CrawlRequestStatus::Running => "running",
        CrawlRequestStatus::Done => "done",
        CrawlRequestStatus::Failed => "failed",
    }
}

fn parse_status(s: &str) -> CrawlRequestStatus {
    match s {
        "running" => CrawlRequestStatus::Running,
        "done" => CrawlRequestStatus::Done,
        "failed" => CrawlRequestStatus::Failed,
        _ => CrawlRequestStatus::Pending,
    }
}

fn type_str(t: CrawlRequestType) -> &'static str {
    match t {
        CrawlRequestType::ListPage => "list_page",
        CrawlRequestType::Body => "body",
        CrawlRequestType::CommentL1 => "comment_l1",
        CrawlRequestType::CommentL2 => "comment_l2",
    }
}

fn parse_type(s: &str) -> CrawlRequestType {
    match s {
        "body" => CrawlRequestType::Body,
        "comment_l1" => CrawlRequestType::CommentL1,
        "comment_l2" => CrawlRequestType::CommentL2,
        _ => CrawlRequestType::ListPage,
    }
}

fn row_to_request(row: &rusqlite::Row<'_>) -> rusqlite::Result<CrawlRequest> {
    let rt: String = row.get("request_type")?;
    let st: String = row.get("status")?;
    Ok(CrawlRequest {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        request_type: parse_type(&rt),
        request_params: row.get("request_params")?,
        status: parse_status(&st),
        account_id: row.get("account_id")?,
        proxy_id: row.get("proxy_id")?,
        error_message: row.get("error_message")?,
        response_summary: row.get("response_summary")?,
        response_data: row.get("response_data")?,
        parent_request_id: row.get("parent_request_id")?,
        retry_count: row.get("retry_count")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn insert_batch(conn: &Connection, requests: &[CrawlRequest]) -> Result<(), AppError> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO crawl_requests
            (id, task_id, request_type, request_params, status,
             account_id, proxy_id, error_message, response_summary,
             response_data, parent_request_id, retry_count, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
    )?;
    for r in requests {
        stmt.execute(params![
            r.id,
            r.task_id,
            type_str(r.request_type),
            r.request_params,
            status_str(r.status),
            r.account_id,
            r.proxy_id,
            r.error_message,
            r.response_summary,
            r.response_data,
            r.parent_request_id,
            r.retry_count,
            r.created_at,
            r.updated_at,
        ])?;
    }
    Ok(())
}

/// Atomically flip up to `limit` pending rows to running and return them.
pub fn take_pending(
    conn: &Connection,
    task_id: &str,
    limit: i64,
) -> Result<Vec<CrawlRequest>, AppError> {
    let now = now_iso();
    conn.execute(
        "UPDATE crawl_requests
         SET status = 'running', updated_at = ?1
         WHERE id IN (
             SELECT id FROM crawl_requests
             WHERE task_id = ?2 AND status = 'pending'
             ORDER BY created_at ASC
             LIMIT ?3
         )",
        params![now, task_id, limit],
    )?;
    let mut stmt = conn.prepare_cached(
        "SELECT * FROM crawl_requests
         WHERE task_id = ?1 AND status = 'running' AND updated_at = ?2
         ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![task_id, now], row_to_request)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 多 worker 并发版本的「领取一条」：用 `UPDATE ... RETURNING` 原子 claim
/// 一条 pending 请求，并把 `account_id` / `proxy_id` 一并写库。
///
/// 返回 `Ok(None)` 表示当前没有 pending 行（队列已空）。
pub fn claim_one(
    conn: &Connection,
    task_id: &str,
    account_id: &str,
    proxy_id: Option<&str>,
) -> Result<Option<CrawlRequest>, AppError> {
    let now = now_iso();
    let mut stmt = conn.prepare_cached(
        "UPDATE crawl_requests
         SET status = 'running', updated_at = ?1, account_id = ?2, proxy_id = ?3
         WHERE id = (
             SELECT id FROM crawl_requests
             WHERE task_id = ?4 AND status = 'pending'
             ORDER BY created_at ASC
             LIMIT 1
         )
         RETURNING *",
    )?;
    let mut rows = stmt.query(params![now, account_id, proxy_id, task_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_request(row)?)),
        None => Ok(None),
    }
}

pub fn mark_done(
    conn: &Connection,
    id: &str,
    response_summary: Option<&str>,
    response_data: Option<&str>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE crawl_requests SET status = 'done', response_summary = ?1, response_data = ?2, updated_at = ?3 WHERE id = ?4",
        params![response_summary, response_data, now_iso(), id],
    )?;
    Ok(())
}

pub fn mark_failed(
    conn: &Connection,
    id: &str,
    error_message: &str,
    retry_count: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE crawl_requests SET status = 'failed', error_message = ?1, retry_count = ?2, updated_at = ?3 WHERE id = ?4",
        params![error_message, retry_count, now_iso(), id],
    )?;
    Ok(())
}

pub fn reset_failed_to_pending(conn: &Connection, task_id: &str) -> Result<u64, AppError> {
    let n = conn.execute(
        "UPDATE crawl_requests SET status = 'pending', error_message = NULL, updated_at = ?1 WHERE task_id = ?2 AND status = 'failed'",
        params![now_iso(), task_id],
    )?;
    Ok(n as u64)
}

pub fn count_by_status(conn: &Connection, task_id: &str) -> Result<TaskProgress, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT status, COUNT(*) as cnt FROM crawl_requests WHERE task_id = ?1 GROUP BY status",
    )?;
    let mut pending = 0i64;
    let mut running = 0i64;
    let mut done = 0i64;
    let mut failed = 0i64;
    let rows = stmt.query_map(params![task_id], |row| {
        let st: String = row.get(0)?;
        let cnt: i64 = row.get(1)?;
        Ok((st, cnt))
    })?;
    for row in rows {
        let (st, cnt) = row?;
        match st.as_str() {
            "pending" => pending = cnt,
            "running" => running = cnt,
            "done" => done = cnt,
            "failed" => failed = cnt,
            _ => {}
        }
    }
    Ok(TaskProgress {
        pending,
        running,
        done,
        failed,
        total: pending + running + done + failed,
    })
}

pub fn delete_by_task(conn: &Connection, task_id: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM crawl_requests WHERE task_id = ?1",
        params![task_id],
    )?;
    Ok(())
}

pub fn has_any(conn: &Connection, task_id: &str) -> Result<bool, AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM crawl_requests WHERE task_id = ?1 LIMIT 1",
        params![task_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Reset any `running` rows back to `pending` (for crash recovery on startup).
pub fn reset_running_to_pending(conn: &Connection, task_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE crawl_requests SET status = 'pending', updated_at = ?1 WHERE task_id = ?2 AND status = 'running'",
        params![now_iso(), task_id],
    )?;
    Ok(())
}
