pub mod account_repo;
pub mod app_event_repo;
pub mod crawl_request_repo;
pub mod migration;
pub mod request_log_repo;
pub mod proxy_repo;
pub mod proxy_runtime_repo;
pub mod record_repo;
pub mod risk_event_repo;
pub mod settings_repo;
pub mod task_repo;
pub mod weibo_account_repo;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::AppError;

pub struct Database {
    path: PathBuf,
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self, AppError> {
        let path_buf = PathBuf::from(path);
        let conn = Connection::open(&path_buf)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=8000;",
        )?;
        Ok(Self {
            path: path_buf,
            conn: Mutex::new(conn),
        })
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("database mutex poisoned")
    }

    /// 长时间运行的采集任务使用独立连接，**不占用** [`Self::conn`] 的互斥锁，
    /// 避免在 HTTP 等待期间阻塞其它 Tauri 命令（列表任务、查库等）。
    pub fn open_crawl_connection(&self) -> Result<Connection, AppError> {
        let c = Connection::open(&self.path)?;
        c.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=8000;",
        )?;
        Ok(c)
    }
}

/// Serialize a serde enum value to its string tag (e.g. `Platform::Weibo` → `"weibo"`).
pub fn enum_to_str<T: serde::Serialize>(val: &T) -> String {
    serde_json::to_value(val)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// Deserialize a string tag back into a serde enum (e.g. `"weibo"` → `Platform::Weibo`).
pub fn str_to_enum<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, AppError> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|e| AppError::Internal(format!("Failed to parse enum: {e}")))
}
