use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::model::account::{Account, AccountStatus, WeiboAccountProfile};

use super::{app_event_repo, enum_to_str, str_to_enum};

const SELECT_COLUMNS: &str = "a.id, a.platform, a.username, a.bound_ip, \
     a.bound_proxy_id, a.risk_status, a.created_at, a.last_active_at, \
     a.cookies, w.weibo_uid, w.center_weibo_name";

pub fn list(conn: &Connection, platform: Option<&str>) -> Result<Vec<Account>, AppError> {
    let (sql, values): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match platform {
        Some(p) => (
            format!(
                "SELECT {SELECT_COLUMNS} FROM accounts a \
                 LEFT JOIN weibo_account_profiles w ON a.id = w.account_id \
                 WHERE a.platform = ?1 ORDER BY a.last_active_at DESC"
            ),
            vec![Box::new(p.to_string())],
        ),
        None => (
            format!(
                "SELECT {SELECT_COLUMNS} FROM accounts a \
                 LEFT JOIN weibo_account_profiles w ON a.id = w.account_id \
                 ORDER BY a.last_active_at DESC"
            ),
            vec![],
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), |row| {
        Ok(RawAccount {
            id: row.get(0)?,
            platform: row.get(1)?,
            username: row.get(2)?,
            bound_ip: row.get(3)?,
            bound_proxy_id: row.get(4)?,
            risk_status: row.get(5)?,
            created_at: row.get(6)?,
            last_active_at: row.get(7)?,
            cookies: row.get(8)?,
            weibo_uid: row.get(9)?,
            center_weibo_name: row.get(10)?,
        })
    })?;

    rows.map(|r| {
        let raw = r?;
        raw.into_model()
    })
    .collect()
}

/// 单条账号（含微博资料 join），用于采集任务绑定账号取 Cookie。
pub fn get_by_id(conn: &Connection, id: &str) -> Result<Account, AppError> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM accounts a \
         LEFT JOIN weibo_account_profiles w ON a.id = w.account_id \
         WHERE a.id = ?1"
    );
    let raw = conn.query_row(&sql, params![id], |row| {
        Ok(RawAccount {
            id: row.get(0)?,
            platform: row.get(1)?,
            username: row.get(2)?,
            bound_ip: row.get(3)?,
            bound_proxy_id: row.get(4)?,
            risk_status: row.get(5)?,
            created_at: row.get(6)?,
            last_active_at: row.get(7)?,
            cookies: row.get(8)?,
            weibo_uid: row.get(9)?,
            center_weibo_name: row.get(10)?,
        })
    })?;
    raw.into_model()
}

pub fn insert(conn: &Connection, account: &Account) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO accounts (id, platform, username, bound_ip, bound_proxy_id, \
         risk_status, created_at, last_active_at, cookies) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            account.id,
            enum_to_str(&account.platform),
            account.username,
            account.bound_ip,
            account.bound_proxy_id,
            enum_to_str(&account.risk_status),
            account.created_at,
            account.last_active_at,
            account.cookies,
        ],
    )?;
    Ok(())
}

/// 新扫码行与已有微博账号为同一 uid：把本次登录态合并到 `existing_id`，并删除 `new_id`。
pub fn merge_new_weibo_account_into_existing(
    conn: &Connection,
    new_id: &str,
    existing_id: &str,
) -> Result<(), AppError> {
    let (risk, last, cookies, bound, bound_pid): (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn.query_row(
        "SELECT risk_status, last_active_at, cookies, bound_ip, bound_proxy_id \
         FROM accounts WHERE id = ?1",
        [new_id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
            ))
        },
    )?;

    conn.execute(
        "UPDATE accounts SET risk_status = ?1, last_active_at = ?2, cookies = ?3, \
         bound_ip = COALESCE(?4, bound_ip), bound_proxy_id = COALESCE(?5, bound_proxy_id) \
         WHERE id = ?6",
        params![risk, last, cookies, bound, bound_pid, existing_id],
    )?;

    conn.execute("DELETE FROM accounts WHERE id = ?1", [new_id])?;
    Ok(())
}

/// 仅更新 Cookie JSON（例如登录后拉取资料页再合并一次 Cookie 罐）。
pub fn update_cookies(conn: &Connection, id: &str, cookies_json: &str) -> Result<(), AppError> {
    let n = conn.execute(
        "UPDATE accounts SET cookies = ?1 WHERE id = ?2",
        params![cookies_json, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("account {id}")));
    }
    Ok(())
}

/// 采集成功等路径刷新「最后活跃时间」。
pub fn touch_last_active(conn: &Connection, id: &str) -> Result<(), AppError> {
    let now = chrono::Local::now()
        .format("%Y-%m-%d %H:%M")
        .to_string();
    let n = conn.execute(
        "UPDATE accounts SET last_active_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("account {id}")));
    }
    Ok(())
}

/// 风控状态写回 `accounts.risk_status`。
/// 由 [`crate::queue::risk::evaluate`] 的 `Verdict.account` 触发；写库前调用方
/// 已比较过 `current != new` 以避免无意义写入。
pub fn update_risk_status(
    conn: &Connection,
    id: &str,
    status: AccountStatus,
) -> Result<(), AppError> {
    let n = conn.execute(
        "UPDATE accounts SET risk_status = ?1 WHERE id = ?2",
        params![enum_to_str(&status), id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("account {id}")));
    }
    Ok(())
}

/// 将 `my.sina.com.cn` 解析出的昵称写回 `accounts.username`。
pub fn update_username(conn: &Connection, id: &str, username: &str) -> Result<(), AppError> {
    let old: String = conn.query_row(
        "SELECT username FROM accounts WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if old == username {
        return Ok(());
    }
    let n = conn.execute(
        "UPDATE accounts SET username = ?1 WHERE id = ?2",
        params![username, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("account {id}")));
    }
    app_event_repo::try_insert(
        conn,
        "account",
        "update",
        "info",
        &format!("更新账号昵称：{old} → {username}（id {id}）"),
        Some("account"),
        Some(id),
        None,
    );
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
    Ok(())
}

/// 方案 B：首页账号健康卡片需要的 (platform, risk_status) 二维聚合。
/// 一次扫表 `GROUP BY platform, risk_status`，service 层再按平台桶归并三态。
/// 返回原始三元组 `(platform, risk_status, count)`，platform 与 risk_status 都是
/// 与 serde tag 对齐的小写字符串（例如 `"weibo"`、`"normal"` / `"restricted"` / `"error"`）。
pub fn count_by_platform_and_risk_status(
    conn: &Connection,
) -> Result<Vec<(String, String, i64)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT platform, risk_status, COUNT(*) FROM accounts \
         GROUP BY platform, risk_status",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn count_by_risk_status(conn: &Connection) -> Result<(i64, i64, i64), AppError> {
    let normal: i64 = conn.query_row(
        "SELECT COUNT(*) FROM accounts WHERE risk_status = 'normal'",
        [],
        |r| r.get(0),
    )?;
    let restricted: i64 = conn.query_row(
        "SELECT COUNT(*) FROM accounts WHERE risk_status = 'restricted'",
        [],
        |r| r.get(0),
    )?;
    let error: i64 = conn.query_row(
        "SELECT COUNT(*) FROM accounts WHERE risk_status = 'error'",
        [],
        |r| r.get(0),
    )?;
    Ok((normal, restricted, error))
}

struct RawAccount {
    id: String,
    platform: String,
    username: String,
    bound_ip: Option<String>,
    bound_proxy_id: Option<String>,
    risk_status: String,
    created_at: String,
    last_active_at: String,
    cookies: Option<String>,
    weibo_uid: Option<String>,
    center_weibo_name: Option<String>,
}

impl RawAccount {
    fn into_model(self) -> Result<Account, AppError> {
        let weibo_profile = self
            .weibo_uid
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|uid| WeiboAccountProfile {
                uid: uid.to_string(),
                center_weibo_name: self.center_weibo_name.clone(),
            });

        Ok(Account {
            id: self.id,
            platform: str_to_enum(&self.platform)?,
            username: self.username,
            bound_ip: self.bound_ip,
            bound_proxy_id: self.bound_proxy_id,
            risk_status: str_to_enum::<AccountStatus>(&self.risk_status)?,
            created_at: self.created_at,
            last_active_at: self.last_active_at,
            cookies: self.cookies,
            weibo_profile,
        })
    }
}
