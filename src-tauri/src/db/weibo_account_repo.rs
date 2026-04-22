use rusqlite::{params, Connection};

use crate::error::AppError;

/// 按微博数字 uid 查找已绑定的账号（用于去重）。
pub fn find_account_id_by_weibo_uid(conn: &Connection, weibo_uid: &str) -> Result<Option<String>, AppError> {
    let r = conn.query_row(
        "SELECT account_id FROM weibo_account_profiles WHERE weibo_uid = ?1",
        [weibo_uid],
        |row| row.get::<_, String>(0),
    );
    match r {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn upsert(
    conn: &Connection,
    account_id: &str,
    weibo_uid: &str,
    center_weibo_name: Option<&str>,
) -> Result<(), AppError> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    conn.execute(
        "INSERT INTO weibo_account_profiles (account_id, weibo_uid, center_weibo_name, updated_at) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(account_id) DO UPDATE SET \
           weibo_uid = excluded.weibo_uid, \
           center_weibo_name = excluded.center_weibo_name, \
           updated_at = excluded.updated_at",
        params![account_id, weibo_uid, center_weibo_name, now],
    )?;
    Ok(())
}
