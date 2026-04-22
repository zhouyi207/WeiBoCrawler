//! 通用 KV 配置表 `app_settings`。
//!
//! 已知用途：IP 代理双探针 URL（cn / intl）、Worker 熔断按平台退避秒数（JSON），由
//! `service::settings_service` 包装为强类型 API。
//! 之所以用纯 KV：
//! - 配置项数量少且稀疏，新增/删除项不想每次写迁移；
//! - 取值天然字符串（URL、bool 字面量、数字），上层按 key 自行解析；
//! - 与业务表零耦合，迁移成本低。

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppError;

/// 取单个 key；不存在返回 `None`。
pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
    let v: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v)
}

/// 写入或更新。前端「保存设置」走这里。
pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
