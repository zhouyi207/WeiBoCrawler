//! 账号 / 代理失败事件持久化。
//!
//! 每次 worker 在请求失败后，会按 [`crate::queue::risk::Attribution`] 将事件
//! 落到 `account_failure_events` 或 `proxy_failure_events`。
//! 这些事件用作滑动窗口（默认 5 min），驱动 `risk::evaluate` 的状态升级 / 回落判定；
//! 跨任务持久，供风控历史分析。
//!
//! `purge_older_than` 在 `run_scheduler` 退出前调用，清理 24h 之前的事件，
//! 防止表无限增长。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::AppError;

/// 单条失败事件参数。`task_id` / `request_id` 允许空（手动重置等场景）；
/// `http_status` 仅 [`crate::queue::risk::ErrorKind::HttpStatus`] 时填。
///
/// `platform`：v4 / 方案 C 起，所有真正由 worker 触发的事件都会带平台；
/// 仅 `account_failure_events` 不消费此字段（账号是单平台资源，没必要再 scope 一次）。
/// 历史 `proxy_failure_events` 行允许 NULL，新版本写入路径强制带值。
pub struct FailureEvent<'a> {
    pub task_id: Option<&'a str>,
    pub request_id: Option<&'a str>,
    pub error_kind: &'a str,
    pub http_status: Option<i64>,
    pub message: Option<&'a str>,
    pub platform: Option<&'a str>,
}

/// 代理日志事件读视图。供 `command::proxy::list_proxy_logs` → 前端日志 modal 使用。
/// 不包含账号字段：业务上 IP 与账号只在「任务配置」做笛卡尔积，运行期不再做反向关联。
///
/// `platform`：v4 起记录该次失败发生在哪个平台 scope 上。老库回填后多数为 `Some`，
/// 仍可能为 `None`（没关联 task_id 的极少数手动事件）；前端展示时把 `None` 当成
/// 「全平台」即可。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyLogEntry {
    pub id: String,
    pub task_id: Option<String>,
    pub request_id: Option<String>,
    pub error_kind: String,
    pub http_status: Option<i64>,
    pub message: Option<String>,
    pub occurred_at: String,
    pub platform: Option<String>,
}

/// 账号日志事件读视图。供 `command::account::list_account_logs` → 前端账号日志 modal 使用。
/// 字段集合与 `ProxyLogEntry` 对齐，方便前端复用展示组件结构。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLogEntry {
    pub id: String,
    pub task_id: Option<String>,
    pub request_id: Option<String>,
    pub error_kind: String,
    pub http_status: Option<i64>,
    pub message: Option<String>,
    pub occurred_at: String,
}

pub fn insert_account_failure(
    conn: &Connection,
    account_id: &str,
    evt: &FailureEvent<'_>,
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    let occurred_at = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO account_failure_events
            (id, account_id, task_id, request_id, error_kind, http_status, message, occurred_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            account_id,
            evt.task_id,
            evt.request_id,
            evt.error_kind,
            evt.http_status,
            evt.message,
            occurred_at,
        ],
    )?;
    Ok(())
}

/// 写入一条代理失败事件。
/// 注：早期版本曾尝试把触发的 `account_id` 一并写入，已废弃；
/// 物理列若历史 DB 上仍存在（v3 引入），新版本一律写入 NULL，避免再次产生 IP↔账号关联。
///
/// v4：写入 `platform` 列；调用方（worker）始终能拿到 `task.platform`。
pub fn insert_proxy_failure(
    conn: &Connection,
    proxy_id: &str,
    evt: &FailureEvent<'_>,
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    let occurred_at = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO proxy_failure_events
            (id, proxy_id, task_id, request_id, error_kind, http_status, message, occurred_at, platform)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            proxy_id,
            evt.task_id,
            evt.request_id,
            evt.error_kind,
            evt.http_status,
            evt.message,
            occurred_at,
            evt.platform,
        ],
    )?;
    Ok(())
}

/// 拉取某账号最近 `limit` 条日志，按 `occurred_at` 降序。
/// 给「账号日志 modal」用：和代理日志对称，只看 timeline。
pub fn list_account_logs(
    conn: &Connection,
    account_id: &str,
    limit: i64,
) -> Result<Vec<AccountLogEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, request_id, error_kind, http_status, message, occurred_at
           FROM account_failure_events
          WHERE account_id = ?1
          ORDER BY occurred_at DESC
          LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![account_id, limit], |row| {
        Ok(AccountLogEntry {
            id: row.get(0)?,
            task_id: row.get(1)?,
            request_id: row.get(2)?,
            error_kind: row.get(3)?,
            http_status: row.get(4)?,
            message: row.get(5)?,
            occurred_at: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// 拉取某代理最近 `limit` 条日志，按 `occurred_at` 降序。
/// 给「IP 日志 modal」用：要看的就是 timeline，没必要分页过深。
pub fn list_proxy_logs(
    conn: &Connection,
    proxy_id: &str,
    limit: i64,
) -> Result<Vec<ProxyLogEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, request_id, error_kind, http_status, message, occurred_at, platform
           FROM proxy_failure_events
          WHERE proxy_id = ?1
          ORDER BY occurred_at DESC
          LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![proxy_id, limit], |row| {
        Ok(ProxyLogEntry {
            id: row.get(0)?,
            task_id: row.get(1)?,
            request_id: row.get(2)?,
            error_kind: row.get(3)?,
            http_status: row.get(4)?,
            message: row.get(5)?,
            occurred_at: row.get(6)?,
            platform: row.get(7)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// 账号风控可归责的窗口计数：仅统计与账号有关的失败 kind 子集，过滤掉
/// 网络 / 414 / 5xx 这类纯代理侧故障的"伴生日志"——日志里仍可见，但不算到风控阈值。
///
/// kind 集合（与 [`crate::queue::risk::attribute`] 中 `Account` / `Both` 分支一致）：
/// - error_kind = `login_required` / `business_reject`
/// - error_kind = `http_status` 且 http_status ∈ {429, 403, 412, 418, 451}
pub fn count_account_attributable_failures_since(
    conn: &Connection,
    account_id: &str,
    since: DateTime<Utc>,
) -> Result<i64, AppError> {
    let s = since.to_rfc3339();
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM account_failure_events
          WHERE account_id = ?1 AND occurred_at >= ?2
            AND (
              error_kind IN ('login_required', 'business_reject')
              OR (error_kind = 'http_status'
                  AND http_status IN (429, 403, 412, 418, 451))
            )",
        params![account_id, s],
        |r| r.get(0),
    )?;
    Ok(n)
}

pub fn count_account_failures_by_kind_since(
    conn: &Connection,
    account_id: &str,
    error_kind: &str,
    since: DateTime<Utc>,
) -> Result<i64, AppError> {
    let s = since.to_rfc3339();
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM account_failure_events
          WHERE account_id = ?1 AND error_kind = ?2 AND occurred_at >= ?3",
        params![account_id, error_kind, s],
        |r| r.get(0),
    )?;
    Ok(n)
}

// NOTE: v3 引入的全局 `count_proxy_attributable_failures_since` 已被
// v4 / 方案 C 的 per-platform 版 `count_proxy_attributable_failures_by_platform_since`
// 完全取代——5xx / 414 / 429 都按 (IP, platform) scope 算，没有"全局任意失败"
// 这个语义了。global 那档现在只剩 net_fails，由 `count_proxy_failures_by_kind_since`
// + `derive_proxy_global_status` 直接处理。

pub fn count_proxy_failures_by_kind_since(
    conn: &Connection,
    proxy_id: &str,
    error_kind: &str,
    since: DateTime<Utc>,
) -> Result<i64, AppError> {
    let s = since.to_rfc3339();
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM proxy_failure_events
          WHERE proxy_id = ?1 AND error_kind = ?2 AND occurred_at >= ?3",
        params![proxy_id, error_kind, s],
        |r| r.get(0),
    )?;
    Ok(n)
}

// 全局 5xx 计数器（`count_proxy_failures_by_status_range_since`）同样被
// per-platform 的 `count_proxy_failures_by_status_range_and_platform_since` 取代，已删除。

// ────────────────────────────────────────────────────────────────────────────
// v4 / 方案 C：per-(proxy, platform) scope 的窗口计数。
// 设计取舍：
// - **HTTP 类失败**（414 / 429 / 5xx）按 platform scope 算：weibo 的 5xx
//   不应该让 douyin 任务里的同一 IP 也被打成 Restricted；
// - **Network 失败**：在阈值低档（≥3）按 platform 算 Restricted（DNS 抖动
//   也可能是某个域名级别的问题），但在阈值高档（≥10）走全局，因为出口连
//   *任何* 平台都连不上时该 IP 就该全局 Invalid，与具体 task 关联无意义。
// - 老库 platform 列回填后绝大多数有值，残留 NULL 的事件会同时落入
//   "全局" 与 "无平台 scope"，per-platform 查询不会算到它。
// ────────────────────────────────────────────────────────────────────────────

/// 同 `count_proxy_attributable_failures_since`，但额外限定 `platform`。
pub fn count_proxy_attributable_failures_by_platform_since(
    conn: &Connection,
    proxy_id: &str,
    platform: &str,
    since: DateTime<Utc>,
) -> Result<i64, AppError> {
    let s = since.to_rfc3339();
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM proxy_failure_events
          WHERE proxy_id = ?1 AND platform = ?2 AND occurred_at >= ?3
            AND (
              error_kind = 'network'
              OR (error_kind = 'http_status'
                  AND (http_status IN (414, 429)
                       OR http_status BETWEEN 500 AND 599))
            )",
        params![proxy_id, platform, s],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// per-platform 版的 by-kind 计数。Network kind 用得最多（≥3 → Restricted on platform）。
pub fn count_proxy_failures_by_kind_and_platform_since(
    conn: &Connection,
    proxy_id: &str,
    error_kind: &str,
    platform: &str,
    since: DateTime<Utc>,
) -> Result<i64, AppError> {
    let s = since.to_rfc3339();
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM proxy_failure_events
          WHERE proxy_id = ?1 AND error_kind = ?2 AND platform = ?3 AND occurred_at >= ?4",
        params![proxy_id, error_kind, platform, s],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// per-platform 版的 5xx 计数。
pub fn count_proxy_failures_by_status_range_and_platform_since(
    conn: &Connection,
    proxy_id: &str,
    lo: i64,
    hi: i64,
    platform: &str,
    since: DateTime<Utc>,
) -> Result<i64, AppError> {
    let s = since.to_rfc3339();
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM proxy_failure_events
          WHERE proxy_id = ?1 AND platform = ?2
            AND http_status BETWEEN ?3 AND ?4
            AND occurred_at >= ?5",
        params![proxy_id, platform, lo, hi, s],
        |r| r.get(0),
    )?;
    Ok(n)
}

// ────────────────────────────────────────────────────────────────────────────
// v6 / 批量版：把 per-(proxy, platform) 的窗口计数一次扫表 group by 出来，
// 给 `proxy_service::list_proxies_runtime` / `list_proxies_health` 用，
// 避免每条 proxy 各发若干次 COUNT 的 N+1。
// 各函数返回 `proxy_id → count`，platform / kind / status range 同 single 版语义。
// ────────────────────────────────────────────────────────────────────────────

/// 全平台 / 单 kind 的 group by。给批量 `derive_proxy_global_status_batch` 用。
pub fn group_proxy_failures_by_kind_since(
    conn: &Connection,
    error_kind: &str,
    since: DateTime<Utc>,
) -> Result<HashMap<String, i64>, AppError> {
    let s = since.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT proxy_id, COUNT(*) FROM proxy_failure_events
          WHERE error_kind = ?1 AND occurred_at >= ?2
          GROUP BY proxy_id",
    )?;
    let rows = stmt.query_map(params![error_kind, s], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (pid, c) = row?;
        out.insert(pid, c);
    }
    Ok(out)
}

/// per-platform 的 by-kind group by。
pub fn group_proxy_failures_by_kind_and_platform_since(
    conn: &Connection,
    error_kind: &str,
    platform: &str,
    since: DateTime<Utc>,
) -> Result<HashMap<String, i64>, AppError> {
    let s = since.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT proxy_id, COUNT(*) FROM proxy_failure_events
          WHERE error_kind = ?1 AND platform = ?2 AND occurred_at >= ?3
          GROUP BY proxy_id",
    )?;
    let rows = stmt.query_map(params![error_kind, platform, s], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (pid, c) = row?;
        out.insert(pid, c);
    }
    Ok(out)
}

/// per-platform 的 5xx 区间 group by。
pub fn group_proxy_failures_by_status_range_and_platform_since(
    conn: &Connection,
    lo: i64,
    hi: i64,
    platform: &str,
    since: DateTime<Utc>,
) -> Result<HashMap<String, i64>, AppError> {
    let s = since.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT proxy_id, COUNT(*) FROM proxy_failure_events
          WHERE platform = ?1 AND http_status BETWEEN ?2 AND ?3
            AND occurred_at >= ?4
          GROUP BY proxy_id",
    )?;
    let rows = stmt.query_map(params![platform, lo, hi, s], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (pid, c) = row?;
        out.insert(pid, c);
    }
    Ok(out)
}

/// per-platform 的「归责到代理的失败」group by。
/// 与 `count_proxy_attributable_failures_by_platform_since` 同 WHERE 条件。
pub fn group_proxy_attributable_failures_by_platform_since(
    conn: &Connection,
    platform: &str,
    since: DateTime<Utc>,
) -> Result<HashMap<String, i64>, AppError> {
    let s = since.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT proxy_id, COUNT(*) FROM proxy_failure_events
          WHERE platform = ?1 AND occurred_at >= ?2
            AND (
              error_kind = 'network'
              OR (error_kind = 'http_status'
                  AND (http_status IN (414, 429)
                       OR http_status BETWEEN 500 AND 599))
            )
          GROUP BY proxy_id",
    )?;
    let rows = stmt.query_map(params![platform, s], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (pid, c) = row?;
        out.insert(pid, c);
    }
    Ok(out)
}

/// 全代理在窗口内出现失败的平台 distinct 列表，按 proxy 分组。
/// 给 `list_proxies_health` 一次性派发。
pub fn group_proxy_failure_platforms_since(
    conn: &Connection,
    since: DateTime<Utc>,
) -> Result<HashMap<String, Vec<String>>, AppError> {
    let s = since.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT DISTINCT proxy_id, platform FROM proxy_failure_events
          WHERE occurred_at >= ?1 AND platform IS NOT NULL",
    )?;
    let rows = stmt.query_map(params![s], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (pid, plat) = row?;
        out.entry(pid).or_default().push(plat);
    }
    Ok(out)
}

/// 拉出该代理在窗口内出现过失败的平台去重列表。
/// `proxy_service::list_proxies_health` 用它来决定要为哪些 platform 调
/// per-platform 派生函数，避免硬编码遍历所有平台 enum。
pub fn list_proxy_failure_platforms_since(
    conn: &Connection,
    proxy_id: &str,
    since: DateTime<Utc>,
) -> Result<Vec<String>, AppError> {
    let s = since.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT DISTINCT platform FROM proxy_failure_events
          WHERE proxy_id = ?1 AND occurred_at >= ?2 AND platform IS NOT NULL",
    )?;
    let rows = stmt.query_map(params![proxy_id, s], |r| r.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// 清理 `threshold` 之前的旧事件。`run_scheduler` 退出前调用。
pub fn purge_older_than(conn: &Connection, threshold: DateTime<Utc>) -> Result<(), AppError> {
    let s = threshold.to_rfc3339();
    conn.execute(
        "DELETE FROM account_failure_events WHERE occurred_at < ?1",
        params![s],
    )?;
    conn.execute(
        "DELETE FROM proxy_failure_events WHERE occurred_at < ?1",
        params![s],
    )?;
    Ok(())
}
