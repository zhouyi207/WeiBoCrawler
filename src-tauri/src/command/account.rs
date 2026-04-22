use tauri::{AppHandle, Manager, State};

use crate::db::risk_event_repo::AccountLogEntry;
use crate::model::account::{Account, GenerateQrResponse, WeiboQrPollResponse};
use crate::service::account_service;
use crate::AppState;

// 统一 `async fn`，避免命令体内的 HTTP 请求 / DB 操作阻塞主线程。
// 详见 `proxy.rs` 顶部说明。
//
// 额外约束：凡是命令体内会构造 / 拥有 `reqwest::blocking::Client`（以及它持有
// 的 inner current-thread tokio runtime）的，**必须**通过 `spawn_blocking` 切到
// blocking pool 上执行。否则当 Client 在 tokio worker 线程的 async 上下文里被
// drop 时，tokio 会拒绝在 worker 线程上 `block_on` 关停 client 内部 runtime，
// 触发 `Cannot drop a runtime in a context where blocking is not allowed` panic。

#[tauri::command]
pub async fn list_accounts(
    state: State<'_, AppState>,
    platform: Option<String>,
) -> Result<Vec<Account>, String> {
    account_service::list_accounts(&state.db, platform.as_deref()).map_err(|e| e.to_string())
}

/// 微博扫码登录入口：内部会发 HTTP 拿 qr_id，并把同 Client 存进
/// `weibo_sessions` 以便后续 poll 复用。HTTP 与 Client 的 drop 全部
/// 走 `spawn_blocking`。
#[tauri::command]
pub async fn generate_login_qr(
    app: AppHandle,
    platform: String,
    ip_id: Option<String>,
) -> Result<GenerateQrResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        account_service::generate_login_qr(&state, &platform, ip_id.as_deref())
    })
    .await
    .map_err(|e| format!("spawn_blocking join 失败: {e}"))?
    .map_err(|e| e.to_string())
}

/// 同上：poll 时会从 session 中取出 Client 发请求；如果 poll 成功并清掉
/// session，Client 也会在此命令体内 drop——必须在 blocking 池上执行。
#[tauri::command]
pub async fn poll_weibo_qr_login(
    app: AppHandle,
    account_id: String,
) -> Result<WeiboQrPollResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        account_service::poll_weibo_qr_login(&state, &account_id)
    })
    .await
    .map_err(|e| format!("spawn_blocking join 失败: {e}"))?
    .map_err(|e| e.to_string())
}

/// 删除账号时会一并把 `weibo_sessions` 里对应的 `WeiboLoginSession` drop 掉，
/// 因此包含 `reqwest::blocking::Client` 的 drop——同样必须放 blocking 池。
#[tauri::command]
pub async fn delete_account(app: AppHandle, id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        account_service::delete_account(&state, &id)
    })
    .await
    .map_err(|e| format!("spawn_blocking join 失败: {e}"))?
    .map_err(|e| e.to_string())
}

/// 拉取某账号最近 N 条失败事件。`limit` 默认 100、上限 200。
/// 对应前端「账号日志」modal，结构与 `list_proxy_logs` 对称。
#[tauri::command]
pub async fn list_account_logs(
    state: State<'_, AppState>,
    id: String,
    limit: Option<i64>,
) -> Result<Vec<AccountLogEntry>, String> {
    account_service::list_account_logs(&state.db, &id, limit).map_err(|e| e.to_string())
}
