use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::model::record::CrawledRecord;
use crate::service::record_service;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagedRecords {
    pub items: Vec<CrawledRecord>,
    pub total: i64,
}

// 统一 `async fn`，避免大数据集查询 / 导出 / 去重阻塞主线程。
// 详见 `proxy.rs` 顶部说明。

#[tauri::command]
pub async fn list_record_task_names(
    state: State<'_, AppState>,
    platform: Option<String>,
) -> Result<Vec<String>, String> {
    record_service::list_distinct_task_names(&state.db, platform.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_records(
    state: State<'_, AppState>,
    platform: Option<String>,
    keyword: Option<String>,
) -> Result<Vec<CrawledRecord>, String> {
    record_service::query_records(&state.db, platform.as_deref(), keyword.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_records_paged(
    state: State<'_, AppState>,
    platform: Option<String>,
    keyword: Option<String>,
    task_name: Option<String>,
    entity_type: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PagedRecords, String> {
    let size = page_size.unwrap_or(50).max(1).min(200);
    let p = page.unwrap_or(1).max(1);
    let offset = (p - 1) * size;
    let (items, total) = record_service::query_records_paged(
        &state.db,
        platform.as_deref(),
        keyword.as_deref(),
        task_name.as_deref(),
        entity_type.as_deref(),
        offset,
        size,
    )
    .map_err(|e| e.to_string())?;
    Ok(PagedRecords { items, total })
}

#[tauri::command]
pub async fn export_records_json(
    state: State<'_, AppState>,
    platform: Option<String>,
    keyword: Option<String>,
    task_name: Option<String>,
    entity_type: Option<String>,
) -> Result<String, String> {
    record_service::export_json(
        &state.db,
        platform.as_deref(),
        keyword.as_deref(),
        task_name.as_deref(),
        entity_type.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_records_excel(
    state: State<'_, AppState>,
    platform: Option<String>,
    keyword: Option<String>,
    task_name: Option<String>,
    entity_type: Option<String>,
) -> Result<Vec<u8>, String> {
    record_service::export_xlsx(
        &state.db,
        platform.as_deref(),
        keyword.as_deref(),
        task_name.as_deref(),
        entity_type.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn deduplicate_records(
    state: State<'_, AppState>,
    platform: Option<String>,
    keyword: Option<String>,
    task_name: Option<String>,
    entity_type: Option<String>,
) -> Result<u64, String> {
    record_service::deduplicate(
        &state.db,
        platform.as_deref(),
        keyword.as_deref(),
        task_name.as_deref(),
        entity_type.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_records_filtered(
    state: State<'_, AppState>,
    platform: Option<String>,
    keyword: Option<String>,
    task_name: Option<String>,
    entity_type: Option<String>,
) -> Result<u64, String> {
    record_service::delete_filtered(
        &state.db,
        platform.as_deref(),
        keyword.as_deref(),
        task_name.as_deref(),
        entity_type.as_deref(),
    )
    .map_err(|e| e.to_string())
}

/// 将导出内容写入用户通过「另存为」选择的路径（二进制，如 `.xlsx`）。
#[tauri::command]
pub async fn write_export_file(path: String, contents: Vec<u8>) -> Result<(), String> {
    std::fs::write(Path::new(&path), contents).map_err(|e| e.to_string())
}
