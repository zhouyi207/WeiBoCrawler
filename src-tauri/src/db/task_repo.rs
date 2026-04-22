use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::model::task::{CrawlStrategy, CrawlTask, RateLimitScope, TaskStatus, TaskType};
use crate::model::weibo_task::WeiboTaskPayload;

use super::{app_event_repo, enum_to_str, str_to_enum};

const SELECT_COLUMNS: &str =
    "id, platform, task_type, name, status, strategy, rate_limit, \
     account_pool_size, ip_pool_size, created_at, bound_account_ids, task_config, \
     bound_proxy_ids, rate_limit_scope";

fn row_to_raw(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawTask> {
    Ok(RawTask {
        id: row.get(0)?,
        platform: row.get(1)?,
        task_type: row.get(2)?,
        name: row.get(3)?,
        status: row.get(4)?,
        strategy: row.get(5)?,
        rate_limit: row.get(6)?,
        account_pool_size: row.get(7)?,
        ip_pool_size: row.get(8)?,
        created_at: row.get(9)?,
        bound_account_ids: row.get(10)?,
        task_config: row.get(11)?,
        bound_proxy_ids: row.get(12)?,
        rate_limit_scope: row.get(13)?,
    })
}

pub fn list(conn: &Connection, platform: Option<&str>) -> Result<Vec<CrawlTask>, AppError> {
    let (sql, values): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match platform {
        Some(p) => (
            format!(
                "SELECT {SELECT_COLUMNS} FROM tasks WHERE platform = ?1 ORDER BY created_at DESC"
            ),
            vec![Box::new(p.to_string())],
        ),
        None => (
            format!("SELECT {SELECT_COLUMNS} FROM tasks ORDER BY created_at DESC"),
            vec![],
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), row_to_raw)?;

    rows.map(|r| {
        let raw = r?;
        raw.into_model()
    })
    .collect()
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<CrawlTask, AppError> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM tasks WHERE id = ?1");
    let raw = conn.query_row(&sql, params![id], row_to_raw)?;
    raw.into_model()
}

pub fn insert(conn: &Connection, task: &CrawlTask) -> Result<(), AppError> {
    let bound_json = json_string(&task.bound_account_ids)?;
    let proxy_json = json_string(&task.bound_proxy_ids)?;
    let config_json: Option<String> = task
        .weibo_config
        .as_ref()
        .map(|v| serde_json::to_string(v))
        .transpose()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    conn.execute(
        "INSERT INTO tasks (id, platform, task_type, name, status, strategy, \
         rate_limit, account_pool_size, ip_pool_size, created_at, bound_account_ids, task_config, \
         bound_proxy_ids, rate_limit_scope) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            task.id,
            enum_to_str(&task.platform),
            enum_to_str(&task.task_type),
            task.name,
            enum_to_str(&task.status),
            enum_to_str(&task.strategy),
            task.rate_limit,
            task.account_pool_size,
            task.ip_pool_size,
            task.created_at,
            bound_json,
            config_json,
            proxy_json,
            enum_to_str(&task.rate_limit_scope),
        ],
    )?;
    Ok(())
}

fn json_string(v: &Option<Vec<String>>) -> Result<Option<String>, AppError> {
    v.as_ref()
        .map(|inner| serde_json::to_string(inner))
        .transpose()
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub fn update_status(conn: &Connection, id: &str, status: TaskStatus) -> Result<(), AppError> {
    let (old_status, name): (String, String) = conn.query_row(
        "SELECT status, name FROM tasks WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let new_s = enum_to_str(&status);
    if old_status == new_s {
        return Ok(());
    }
    let affected = conn.execute(
        "UPDATE tasks SET status = ?1 WHERE id = ?2",
        params![new_s, id],
    )?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Task {id}")));
    }
    let msg = format!(
        "任务「{}」状态：{} → {}",
        name,
        task_status_label_zh(&old_status),
        task_status_label_zh(new_s.as_str()),
    );
    let level = if new_s == "error" { "error" } else { "info" };
    app_event_repo::try_insert(
        conn,
        "task",
        "status_change",
        level,
        &msg,
        Some("task"),
        Some(id),
        Some(id),
    );
    Ok(())
}

fn task_status_label_zh(s: &str) -> &'static str {
    match s {
        "running" => "运行中",
        "paused" => "已暂停",
        "completed" => "已完成",
        "error" => "异常",
        _ => "其它",
    }
}

pub fn update(conn: &Connection, task: &CrawlTask) -> Result<(), AppError> {
    let bound_json = json_string(&task.bound_account_ids)?;
    let proxy_json = json_string(&task.bound_proxy_ids)?;
    let config_json: Option<String> = task
        .weibo_config
        .as_ref()
        .map(|v| serde_json::to_string(v))
        .transpose()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let affected = conn.execute(
        "UPDATE tasks SET name = ?1, strategy = ?2, rate_limit = ?3, account_pool_size = ?4, \
         bound_account_ids = ?5, task_config = ?6, ip_pool_size = ?7, bound_proxy_ids = ?8, \
         rate_limit_scope = ?9 WHERE id = ?10",
        params![
            task.name,
            enum_to_str(&task.strategy),
            task.rate_limit,
            task.account_pool_size,
            bound_json,
            config_json,
            task.ip_pool_size,
            proxy_json,
            enum_to_str(&task.rate_limit_scope),
            task.id,
        ],
    )?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Task {}", task.id)));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    Ok(())
}

/// v7：从「任务规划」维度反查 (proxy → 该 proxy 上规划要跑的账号集合)。
///
/// 实现：扫指定 platform 下的全部 tasks（不论 status），把每条任务的
/// `bound_proxy_ids × bound_account_ids` 笛卡尔展开后并集到 map。
///
/// 与 `worker.rs::build_worker_specs` 的 fallback 对齐：当某条任务
/// `bound_proxy_ids` 为空，会落到 [`crate::model::proxy::LOCAL_DIRECT_PROXY_ID`]
/// 行——这样直连任务的"绑定账号数"在 IP 管理页的「直连」行也能看到。
///
/// 给 `proxy_service::list_proxies_runtime` 的「绑定账号数量」列用：
/// 含义是「**有多少账号被任务规划要在该 IP 上跑**」，而不是「账号扫码登录时绑了哪条代理」。
pub fn group_planned_account_ids_by_proxy(
    conn: &Connection,
    platform: &str,
) -> Result<HashMap<String, HashSet<String>>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT bound_account_ids, bound_proxy_ids FROM tasks WHERE platform = ?1",
    )?;
    let rows = stmt.query_map(params![platform], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
        ))
    })?;

    let mut map: HashMap<String, HashSet<String>> = HashMap::new();
    for row in rows {
        let (acc_raw, prx_raw) = row?;
        let accounts = parse_id_list(acc_raw.as_deref())?.unwrap_or_default();
        if accounts.is_empty() {
            continue;
        }
        let proxies = parse_id_list(prx_raw.as_deref())?.unwrap_or_default();
        let effective: Vec<String> = if proxies.is_empty() {
            vec![crate::model::proxy::LOCAL_DIRECT_PROXY_ID.to_string()]
        } else {
            proxies
        };
        for pid in &effective {
            let set = map.entry(pid.clone()).or_default();
            for aid in &accounts {
                set.insert(aid.clone());
            }
        }
    }
    Ok(map)
}

pub fn count_by_status(conn: &Connection) -> Result<(i64, i64, i64), AppError> {
    let running: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'running'",
        [],
        |r| r.get(0),
    )?;
    let paused: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'paused'",
        [],
        |r| r.get(0),
    )?;
    let error: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'error'",
        [],
        |r| r.get(0),
    )?;
    Ok((running, paused, error))
}

struct RawTask {
    id: String,
    platform: String,
    task_type: String,
    name: String,
    status: String,
    strategy: String,
    rate_limit: i64,
    account_pool_size: i64,
    ip_pool_size: i64,
    created_at: String,
    bound_account_ids: Option<String>,
    task_config: Option<String>,
    bound_proxy_ids: Option<String>,
    rate_limit_scope: Option<String>,
}

impl RawTask {
    fn into_model(self) -> Result<CrawlTask, AppError> {
        let bound_account_ids = parse_id_list(self.bound_account_ids.as_deref())?;
        let bound_proxy_ids = parse_id_list(self.bound_proxy_ids.as_deref())?;
        let weibo_config = match self.task_config.as_deref() {
            None | Some("") => None,
            Some(s) => Some(serde_json::from_str::<WeiboTaskPayload>(s).map_err(|e| AppError::Internal(e.to_string()))?),
        };
        let rate_limit_scope = match self.rate_limit_scope.as_deref() {
            None | Some("") => RateLimitScope::default(),
            Some(s) => str_to_enum::<RateLimitScope>(s)?,
        };
        Ok(CrawlTask {
            id: self.id,
            platform: str_to_enum(&self.platform)?,
            task_type: str_to_enum::<TaskType>(&self.task_type)?,
            name: self.name,
            status: str_to_enum::<TaskStatus>(&self.status)?,
            strategy: str_to_enum::<CrawlStrategy>(&self.strategy)?,
            rate_limit: self.rate_limit,
            account_pool_size: self.account_pool_size,
            ip_pool_size: self.ip_pool_size,
            created_at: self.created_at,
            bound_account_ids,
            bound_proxy_ids,
            rate_limit_scope,
            weibo_config,
        })
    }
}

fn parse_id_list(raw: Option<&str>) -> Result<Option<Vec<String>>, AppError> {
    match raw {
        None | Some("") => Ok(None),
        Some(s) => serde_json::from_str(s)
            .map(Some)
            .map_err(|e| AppError::Internal(e.to_string())),
    }
}
