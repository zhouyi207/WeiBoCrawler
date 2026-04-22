use tauri::{AppHandle, Manager, State};

use crate::db::risk_event_repo::ProxyLogEntry;
use crate::model::proxy::{ProxyGlobalRow, ProxyIp, ProxyPlatformRow};
use crate::service::proxy_service::{self, ProxyHealthBrief};
use crate::service::settings_service::{
    self, ProxyProbeSettings, UpdateWorkerBackoffPayload, WorkerBackoffSettings,
};
use crate::AppState;

// 所有命令统一声明为 `async fn`：Tauri 2 中同步命令会在主线程执行，
// 一旦命令体内有 DB / 网络 / 线程 join 等阻塞调用，就会冻结窗口消息泵
// （表现为按钮转圈期间无法拖动 / 缩放窗口）。`async fn` 会被 Tauri 调度
// 到 async 运行时的 worker 上，主线程始终保持响应。

#[tauri::command]
pub async fn list_proxies(state: State<'_, AppState>) -> Result<Vec<ProxyIp>, String> {
    proxy_service::list_proxies(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_proxy(
    state: State<'_, AppState>,
    address: String,
    proxy_type: String,
    remark: Option<String>,
) -> Result<ProxyIp, String> {
    proxy_service::add_proxy(&state.db, &address, &proxy_type, remark)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_proxy(state: State<'_, AppState>, id: String) -> Result<(), String> {
    proxy_service::delete_proxy(&state.db, &id).map_err(|e| e.to_string())
}

/// 编辑代理的 address / proxy_type / remark。**地址变更**时会同步并行重跑
/// geo + cn 双探针 + intl 双探针（≤ ~10s，与 add_proxy 一致）；address 没变则
/// 仅写 remark / proxy_type。系统行 / Direct 在 service 层被拒。
#[tauri::command]
pub async fn update_proxy(
    state: State<'_, AppState>,
    id: String,
    address: String,
    proxy_type: String,
    remark: Option<String>,
) -> Result<ProxyIp, String> {
    proxy_service::update_proxy(&state.db, &id, &address, &proxy_type, remark)
        .map_err(|e| e.to_string())
}

/// 拉取某代理最近 N 条日志事件。`limit` 默认 100、上限 200。
#[tauri::command]
pub async fn list_proxy_logs(
    state: State<'_, AppState>,
    id: String,
    limit: Option<i64>,
) -> Result<Vec<ProxyLogEntry>, String> {
    proxy_service::list_proxy_logs(&state.db, &id, limit).map_err(|e| e.to_string())
}

/// 批量返回每条代理的派生健康状态（available / restricted / invalid）。
/// CreateTaskModal 用此结果做"失效代理禁选"的展示与判定。
#[tauri::command]
pub async fn list_proxies_health(
    state: State<'_, AppState>,
) -> Result<Vec<ProxyHealthBrief>, String> {
    proxy_service::list_proxies_health(&state.db).map_err(|e| e.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// 全局 / per-platform 视图（IP 管理页 Tabs 直接消费）
// ─────────────────────────────────────────────────────────────────────────────

/// 全局 tab 装配：每条代理拉一次基础元数据 + 从同一行 stack 字段派生的双延迟。
/// **不**触发新探测，想要刷新请显式调 [`check_all_proxies_dual_health`]。
#[tauri::command]
pub async fn list_proxies_global(
    state: State<'_, AppState>,
) -> Result<Vec<ProxyGlobalRow>, String> {
    proxy_service::list_proxies_global(&state.db).map_err(|e| e.to_string())
}

/// per-platform tab 装配：基础元数据 + 最后一次响应 + 绑定 / 运行账号数 +
/// 派生状态 + 风险系数。详见 `proxy_service::list_proxies_runtime`。
#[tauri::command]
pub async fn list_proxies_runtime(
    state: State<'_, AppState>,
    platform: String,
) -> Result<Vec<ProxyPlatformRow>, String> {
    proxy_service::list_proxies_runtime(&state.db, &state.worker_registry, &platform)
        .map_err(|e| e.to_string())
}

/// 批量探测所有代理的 geo + cn / intl 双延迟（三件事并行），写回 `proxies` 行
/// （cn_latency_ms / intl_latency_ms / geo_* / last_probed_at），并返回最新组装的
/// 「全局」行列表。单条代理最长 ~10s（geo 5+5、cn 5+5、intl 5+5 并行 → max ≈ 10s）。
#[tauri::command]
pub async fn check_all_proxies_dual_health(
    state: State<'_, AppState>,
) -> Result<Vec<ProxyGlobalRow>, String> {
    proxy_service::check_all_proxies_dual_health(&state.db).map_err(|e| e.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// 探针目标设置（IP 管理页 → 设置入口或全局设置面板共用）
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_proxy_probe_settings(
    state: State<'_, AppState>,
) -> Result<ProxyProbeSettings, String> {
    settings_service::get_proxy_probe_settings(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_proxy_probe_settings(
    state: State<'_, AppState>,
    cn_target: String,
    intl_target: String,
) -> Result<ProxyProbeSettings, String> {
    settings_service::update_proxy_probe_settings(
        &state.db,
        Some(cn_target.as_str()),
        Some(intl_target.as_str()),
    )
    .map_err(|e| e.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Worker 熔断退避（按任务平台，与采集进度条「连续失败…退避…秒」一致）
// ─────────────────────────────────────────────────────────────────────────────
//
// 读写在 `spawn_blocking` 中执行：主 `Database::conn()` 为 Mutex，若与采集线程
// 争用锁，在 async worker 上长时间阻塞会拖住 Tauri 调度，表现为保存时界面卡死。

#[tauri::command]
pub async fn get_worker_backoff_settings(
    app: AppHandle,
) -> Result<WorkerBackoffSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        settings_service::get_worker_backoff_settings(&state.db)
    })
    .await
    .map_err(|e| format!("spawn_blocking join 失败: {e}"))?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_worker_backoff_settings(
    app: AppHandle,
    payload: UpdateWorkerBackoffPayload,
) -> Result<WorkerBackoffSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        settings_service::update_worker_backoff_settings(&state.db, payload.seconds_by_platform)
    })
    .await
    .map_err(|e| format!("spawn_blocking join 失败: {e}"))?
    .map_err(|e| e.to_string())
}
