use serde::Deserialize;
use tauri::State;

use crate::model::crawl_request::TaskProgress;
use crate::model::task::CrawlTask;
use crate::service::task_service;
use crate::AppState;

/// 与前端 `invoke('create_task', { … })` 对齐：JSON 使用 camelCase（Tauri 2 IPC 约定）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskArgs {
    pub platform: String,
    pub task_type: String,
    pub name: String,
    pub strategy: String,
    pub rate_limit: i64,
    pub account_ids: Option<Vec<String>>,
    pub proxy_ids: Option<Vec<String>>,
    pub rate_limit_scope: Option<String>,
    pub weibo_config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskArgs {
    pub id: String,
    pub name: String,
    pub strategy: String,
    pub rate_limit: i64,
    pub account_ids: Option<Vec<String>>,
    pub proxy_ids: Option<Vec<String>>,
    pub rate_limit_scope: Option<String>,
    pub weibo_config: Option<serde_json::Value>,
}

// 统一 `async fn`，避免命令在主线程上执行时阻塞窗口。详见 `proxy.rs` 顶部说明。

#[tauri::command]
pub async fn list_tasks(
    state: State<'_, AppState>,
    platform: Option<String>,
) -> Result<Vec<CrawlTask>, String> {
    task_service::list_tasks(&state.db, platform.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_task(
    state: State<'_, AppState>,
    args: CreateTaskArgs,
) -> Result<CrawlTask, String> {
    let weibo: Option<crate::model::weibo_task::WeiboTaskPayload> = match args.weibo_config {
        None => None,
        Some(v) => Some(serde_json::from_value(v).map_err(|e| e.to_string())?),
    };
    task_service::create_task(
        &state.db,
        &args.platform,
        &args.task_type,
        &args.name,
        &args.strategy,
        args.rate_limit,
        args.account_ids,
        args.proxy_ids,
        args.rate_limit_scope.as_deref(),
        weibo,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_task(
    state: State<'_, AppState>,
    args: UpdateTaskArgs,
) -> Result<CrawlTask, String> {
    let weibo: Option<crate::model::weibo_task::WeiboTaskPayload> = match args.weibo_config {
        None => None,
        Some(v) => Some(serde_json::from_value(v).map_err(|e| e.to_string())?),
    };
    task_service::update_task(
        &state.db,
        &args.id,
        &args.name,
        &args.strategy,
        args.rate_limit,
        args.account_ids,
        args.proxy_ids,
        args.rate_limit_scope.as_deref(),
        weibo,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    task_service::delete_task(&state.db, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    task_service::start_task(&state.db, &state.queue_tx, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    task_service::pause_task(&state.db, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restart_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    task_service::restart_task(&state.db, &state.queue_tx, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_task_progress(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<TaskProgress, String> {
    task_service::get_task_progress(&state.db, &task_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn retry_failed_requests(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<u64, String> {
    task_service::retry_failed_requests(&state.db, &state.queue_tx, &task_id)
        .map_err(|e| e.to_string())
}
