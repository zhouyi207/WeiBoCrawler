use serde::Serialize;
use tauri::State;

use crate::db::request_log_repo::{self, RequestLogListItem};
use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogsPage {
    pub items: Vec<RequestLogListItem>,
    pub total: i64,
}

#[tauri::command]
pub fn list_request_logs(
    state: State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<RequestLogsPage, String> {
    let conn = state.db.conn();
    let items = request_log_repo::list_recent(&conn, limit.unwrap_or(100), offset.unwrap_or(0))
        .map_err(app_err)?;
    let total = request_log_repo::count(&conn).map_err(app_err)?;
    Ok(RequestLogsPage { items, total })
}

#[tauri::command]
pub fn clear_request_logs(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.conn();
    request_log_repo::clear_all(&conn).map_err(app_err)
}

fn app_err(e: AppError) -> String {
    e.to_string()
}
