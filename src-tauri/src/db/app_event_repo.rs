//! 应用级事件日志：账号 / 代理 / 任务 CRUD、任务状态迁移、采集过程中的风控状态变化等。
//! 供首页「最近日志」与审计使用。

use rusqlite::{params, Connection};

use crate::error::AppError;

/// 写入一条事件。主流程应使用 [`try_insert`]，避免日志失败影响业务。
pub fn insert(
    conn: &Connection,
    scope: &str,
    action: &str,
    level: &str,
    message: &str,
    subject_type: Option<&str>,
    subject_id: Option<&str>,
    task_id: Option<&str>,
    context_json: Option<&str>,
) -> Result<(), AppError> {
    let occurred_at = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    conn.execute(
        "INSERT INTO app_event_log (occurred_at, scope, action, level, message, \
         context_json, subject_type, subject_id, task_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            occurred_at,
            scope,
            action,
            level,
            message,
            context_json,
            subject_type,
            subject_id,
            task_id,
        ],
    )?;
    Ok(())
}

pub fn try_insert(
    conn: &Connection,
    scope: &str,
    action: &str,
    level: &str,
    message: &str,
    subject_type: Option<&str>,
    subject_id: Option<&str>,
    task_id: Option<&str>,
) {
    if let Err(e) = insert(
        conn,
        scope,
        action,
        level,
        message,
        subject_type,
        subject_id,
        task_id,
        None,
    ) {
        log::warn!("[app_event_log] insert failed: {e}");
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEventListItem {
    pub time: String,
    pub scope: String,
    pub action: String,
    pub level: String,
    pub message: String,
}

pub fn list_recent(conn: &Connection, limit: i64) -> Result<Vec<AppEventListItem>, AppError> {
    let lim = limit.clamp(1, 200);
    let mut stmt = conn.prepare(
        "SELECT occurred_at, scope, action, level, message \
           FROM app_event_log \
          ORDER BY id DESC \
          LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![lim], |row| {
        Ok(AppEventListItem {
            time: row.get(0)?,
            scope: row.get(1)?,
            action: row.get(2)?,
            level: row.get(3)?,
            message: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
