//! `proxy_platform_runtime`：per-(proxy, platform) 维度的"最近一次响应"画像。
//!
//! - **写**：worker 在每条请求结束后（成功或失败）调 [`upsert`]；
//! - **读**：`proxy_service::list_proxies_runtime` 拼接到 per-platform tab。
//!
//! 设计取舍：只保留"最后一次"而非历史 timeline，理由——
//! - timeline 已经由 `proxy_failure_events`（失败侧）承担；
//! - 成功事件量大且无诊断价值，再写一张表会显著放大磁盘 IO；
//! - per-platform tab 主要回答"它现在还活着吗、最近一次是谁打过去的"，
//!   单行 upsert 足够。

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::error::AppError;

/// 单条 upsert 入参。`status` 只有两种取值：
/// - `"success"`：`error_kind / http_status` 通常为 `None`；
/// - `"failure"`：`error_kind` 与 `crate::queue::risk::ErrorKind::as_tag` 对齐。
pub struct RuntimeSample<'a> {
    pub proxy_id: &'a str,
    pub platform: &'a str,
    pub account_id: &'a str,
    /// 本次请求实际耗时（毫秒）。失败时也可记录已用时间，便于用户判断"是慢失败还是快失败"。
    pub latency_ms: i64,
    pub status: &'a str,
    pub error_kind: Option<&'a str>,
    pub http_status: Option<i64>,
    /// `YYYY-MM-DD HH:MM:SS`。由调用方统一用本地时区生成，便于与日志面板对齐。
    pub responded_at: &'a str,
}

/// upsert：不存在则插入，已存在则覆盖最新一条。
pub fn upsert(conn: &Connection, sample: &RuntimeSample<'_>) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO proxy_platform_runtime (
            proxy_id, platform, last_responded_at, last_account_id,
            last_latency_ms, last_status, last_error_kind, last_http_status
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(proxy_id, platform) DO UPDATE SET
            last_responded_at = excluded.last_responded_at,
            last_account_id   = excluded.last_account_id,
            last_latency_ms   = excluded.last_latency_ms,
            last_status       = excluded.last_status,
            last_error_kind   = excluded.last_error_kind,
            last_http_status  = excluded.last_http_status",
        params![
            sample.proxy_id,
            sample.platform,
            sample.responded_at,
            sample.account_id,
            sample.latency_ms,
            sample.status,
            sample.error_kind,
            sample.http_status,
        ],
    )?;
    Ok(())
}

/// 读取某 (proxy, platform) 的最近一条画像。`None` 表示该组合从未跑过。
#[derive(Debug, Clone, Default)]
pub struct RuntimeSnapshot {
    pub last_responded_at: Option<String>,
    pub last_account_id: Option<String>,
    pub last_latency_ms: Option<i64>,
    pub last_status: Option<String>,
    pub last_error_kind: Option<String>,
    pub last_http_status: Option<i64>,
}

/// 批量版：一次扫表把指定 platform 的所有 (proxy_id → snapshot) 拉成 map。
/// 用于 `proxy_service::list_proxies_runtime` 消除 N+1。
pub fn list_by_platform(
    conn: &Connection,
    platform: &str,
) -> Result<HashMap<String, RuntimeSnapshot>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT proxy_id, last_responded_at, last_account_id, last_latency_ms,
                last_status, last_error_kind, last_http_status
           FROM proxy_platform_runtime
          WHERE platform = ?1",
    )?;
    let rows = stmt.query_map(params![platform], |r| {
        Ok((
            r.get::<_, String>(0)?,
            RuntimeSnapshot {
                last_responded_at: r.get(1).ok(),
                last_account_id: r.get(2).ok(),
                last_latency_ms: r.get(3).ok(),
                last_status: r.get(4).ok(),
                last_error_kind: r.get(5).ok(),
                last_http_status: r.get(6).ok(),
            },
        ))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (pid, snap) = row?;
        out.insert(pid, snap);
    }
    Ok(out)
}

pub fn get(
    conn: &Connection,
    proxy_id: &str,
    platform: &str,
) -> Result<Option<RuntimeSnapshot>, AppError> {
    let row = conn
        .query_row(
            "SELECT last_responded_at, last_account_id, last_latency_ms,
                    last_status, last_error_kind, last_http_status
               FROM proxy_platform_runtime
              WHERE proxy_id = ?1 AND platform = ?2",
            params![proxy_id, platform],
            |r| {
                Ok(RuntimeSnapshot {
                    last_responded_at: r.get(0).ok(),
                    last_account_id: r.get(1).ok(),
                    last_latency_ms: r.get(2).ok(),
                    last_status: r.get(3).ok(),
                    last_error_kind: r.get(4).ok(),
                    last_http_status: r.get(5).ok(),
                })
            },
        )
        .optional_internal()?;
    Ok(row)
}

// rusqlite 的 OptionalExtension 名字与本仓库一些业务模块中的 `Option` 用法
// 偶发冲突，这里用一个本地小 trait 把 `QueryReturnedNoRows` 转 `None`，避免
// 在大文件里再多一次 use 语句。
trait QueryOptional<T> {
    fn optional_internal(self) -> Result<Option<T>, AppError>;
}

impl<T> QueryOptional<T> for Result<T, rusqlite::Error> {
    fn optional_internal(self) -> Result<Option<T>, AppError> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
