mod command;
mod db;
mod error;
mod model;
mod queue;
mod service;
mod weibo;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use db::Database;
use model::account::Account;
use queue::message::CrawlCommand;
use queue::registry::WorkerRegistry;
use queue::runtime_buffer::RuntimeBuffer;
use tauri::Manager;

pub struct AppState {
    pub db: Database,
    pub queue_tx: tokio::sync::mpsc::Sender<CrawlCommand>,
    /// 微博扫码登录会话：`account_id` → 保持 Cookie 的 HTTP 客户端（见 WeiBoCrawler `get_cookies.py`）。
    pub weibo_sessions: Mutex<HashMap<String, weibo::WeiboLoginSession>>,
    /// 「待扫码」账号草稿：generate_login_qr 时入内存，扫码成功才写库；
    /// 这样失败 / 用户关弹窗都不会在 `accounts` 表里留下未完成扫码的脏行。
    /// key = 与 `weibo_sessions` 同源的 account_id。
    pub pending_accounts: Mutex<HashMap<String, Account>>,
    /// `my.sina.com.cn` 拉取的完整 HTML 落盘目录（便于用浏览器打开检查）。
    pub weibo_my_sina_debug_dir: PathBuf,
    /// 运行中的 (proxy_id, platform) → set<account_id> 注册表，纯内存。
    /// 由 `queue::worker` 在每个 worker 启动 / 退出时维护，
    /// 供 `proxy_service::list_proxies_runtime` 拼出"运行账号数"列。
    pub worker_registry: Arc<WorkerRegistry>,
    /// "最后一次响应"画像内存合并缓冲：worker push，1s tick 批量 upsert，
    /// 详见 [`queue::runtime_buffer`]。
    pub runtime_buffer: Arc<RuntimeBuffer>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 日志初始化：默认 INFO，前端用 `RUST_LOG=ysscrawler_lib=debug` 调高。
    // try_init 防止在 mobile / 嵌入场景下被 host 提前 init 的 logger 覆盖。
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .format_timestamp_millis()
    .try_init();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    builder
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            let weibo_my_sina_debug_dir = data_dir.join("debug").join("weibo_my_sina");
            std::fs::create_dir_all(&weibo_my_sina_debug_dir)?;

            let db_path = data_dir.join("ysscrawler.db");
            let database =
                Database::open(db_path.to_str().expect("invalid db path"))?;

            {
                let conn = database.conn();
                db::migration::run(&conn)?;
            }

            let (tx, rx) = tokio::sync::mpsc::channel::<CrawlCommand>(256);

            // RuntimeBuffer flusher 用一条独立 connection；失败时退化为「不合并 + 不落盘」，
            // 不阻塞主流程。worker 仍然会 push 到 buffer，只是 buffer 永远不被 flush。
            let runtime_buffer = Arc::new(RuntimeBuffer::new());
            match database.open_crawl_connection() {
                Ok(c) => runtime_buffer.clone().start_flusher(c),
                Err(e) => log::warn!(
                    "[startup] runtime_buffer flusher disabled, open dedicated connection failed: {e}"
                ),
            }

            app.manage(AppState {
                db: database,
                queue_tx: tx,
                weibo_sessions: Mutex::new(HashMap::new()),
                pending_accounts: Mutex::new(HashMap::new()),
                weibo_my_sina_debug_dir,
                worker_registry: Arc::new(WorkerRegistry::new()),
                runtime_buffer,
            });

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                queue::worker::run(rx, handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // home
            command::home::get_dashboard_stats,
            // task
            command::task::list_tasks,
            command::task::create_task,
            command::task::update_task,
            command::task::delete_task,
            command::task::start_task,
            command::task::pause_task,
            command::task::restart_task,
            command::task::get_task_progress,
            command::task::retry_failed_requests,
            // account
            command::account::list_accounts,
            command::account::generate_login_qr,
            command::account::poll_weibo_qr_login,
            command::account::delete_account,
            command::account::list_account_logs,
            // proxy
            command::proxy::list_proxies,
            command::proxy::add_proxy,
            command::proxy::update_proxy,
            command::proxy::delete_proxy,
            command::proxy::list_proxy_logs,
            command::proxy::list_proxies_health,
            command::proxy::list_proxies_global,
            command::proxy::list_proxies_runtime,
            command::proxy::check_all_proxies_dual_health,
            command::proxy::get_proxy_probe_settings,
            command::proxy::update_proxy_probe_settings,
            command::proxy::get_worker_backoff_settings,
            command::proxy::update_worker_backoff_settings,
            // record
            command::record::list_record_task_names,
            command::record::query_records,
            command::record::query_records_paged,
            command::record::export_records_json,
            command::record::export_records_excel,
            command::record::deduplicate_records,
            command::record::delete_records_filtered,
            command::record::write_export_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
