use std::collections::BTreeMap;

use rusqlite::Connection;

use crate::db::{account_repo, task_repo, Database};
use crate::error::AppError;
use crate::model::proxy::IpStatus;
use crate::model::stats::{
    AccountStats, DashboardStats, IpStats, LogEntry, PlatformOverview, TaskStats,
};
use crate::service::proxy_service::{self, ProxyHealthBrief};

pub fn get_dashboard_stats(db: &Database) -> Result<DashboardStats, AppError> {
    // ⚠ 关键约束：`Database::conn()` 返回 `MutexGuard<'_, Connection>`，背后是
    // `std::sync::Mutex`——**非可重入**。本函数会调多个 service（`proxy_service`
    // 内部会再 `db.conn()`），必须把每段 conn 的作用域**严格控制在不再调用其它
    // 会再加锁的函数之内**，否则同一线程二次 lock 会直接死锁，整个后端所有命令
    // 都会被 block 住，表现为「全部数据无法加载」。
    //
    // 流程：
    //   step1 (持锁): 任务统计 + 账号全局/分平台统计 → 释放
    //   step2 (无锁): 调 proxy_service::list_proxies_health（内部自取自释锁）
    //   step3 (无锁): 用 healths 同时算「全局 IP 三态」「按平台 IP 三态」
    //   step4 (无锁): join 账号桶 + IP 桶 = per_platform
    //   step5 (持锁): 拉最近日志

    // ─ step1 ────────────────────────────────────────────────
    let (task_stats, account_stats, account_per_platform_raw) = {
        let conn = db.conn();
        let (running, paused, error) = task_repo::count_by_status(&conn)?;
        let task_stats = TaskStats {
            running,
            paused,
            error,
            total: running + paused + error,
        };

        let (normal, restricted, acc_error) = account_repo::count_by_risk_status(&conn)?;
        let account_stats = AccountStats {
            normal,
            restricted,
            error: acc_error,
            total: normal + restricted + acc_error,
        };

        // 失败时不打断 dashboard：per_platform 退化成空数组，前端兜底「暂无账号」。
        let raw = account_repo::count_by_platform_and_risk_status(&conn).unwrap_or_else(|e| {
            log::warn!(
                "[stats_service] count_by_platform_and_risk_status failed, per_platform fallback to empty: {e}"
            );
            Vec::new()
        });
        (task_stats, account_stats, raw)
    };

    // ─ step2 ────────────────────────────────────────────────
    let healths_result = proxy_service::list_proxies_health(db);

    // ─ step3 + step4 ───────────────────────────────────────
    let (ip_stats, per_platform) = match &healths_result {
        Ok(healths) => {
            let ip_stats = aggregate_global_ip_stats(healths);
            let account_buckets = collect_account_buckets(account_per_platform_raw);
            let per_platform = join_per_platform(account_buckets, healths);
            (ip_stats, per_platform)
        }
        Err(e) => {
            log::warn!(
                "[stats_service] list_proxies_health failed, ip stats fallback to zeros: {e}"
            );
            // healths 拿不到时：全局 IP 卡只显示 total；per_platform 表只展示账号侧三态，
            // IP 列全部置 0（前端会原样显示，不会因此崩掉）。
            let total = proxy_service::list_proxies(db)
                .map(|v| v.len() as i64)
                .unwrap_or(0);
            let ip_stats = IpStats {
                available: 0,
                restricted: 0,
                invalid: 0,
                total,
            };
            let account_buckets = collect_account_buckets(account_per_platform_raw);
            let per_platform = account_buckets
                .into_values()
                .map(|b| PlatformOverview {
                    platform: b.platform,
                    account_normal: b.normal,
                    account_restricted: b.restricted,
                    account_error: b.error,
                    account_total: b.total,
                    ip_available: 0,
                    ip_restricted: 0,
                    ip_invalid: 0,
                })
                .collect();
            (ip_stats, per_platform)
        }
    };

    // ─ step5 ────────────────────────────────────────────────
    let recent_logs = {
        let conn = db.conn();
        load_recent_logs(&conn).unwrap_or_else(|e| {
            log::warn!("[stats_service] load_recent_logs failed, returning empty: {e}");
            Vec::new()
        })
    };

    Ok(DashboardStats {
        task_stats,
        account_stats,
        ip_stats,
        per_platform,
        recent_logs,
    })
}

/// 把 `ProxyHealthBrief` 列表压成全局视图的 IP 三态：
/// - `Invalid` → invalid；
/// - 否则 `restrictions` 非空 → restricted；
/// - 否则 → available。
///
/// 这里的「restricted」是**只要在任何平台被限就计 1 次**的全局口径，
/// 与 per_platform 行里"该平台是否被限"的口径**不同**——前端两个位置
/// 显示的同一 IP 可能落在不同的桶里，这是设计预期，不是 bug。
fn aggregate_global_ip_stats(healths: &[ProxyHealthBrief]) -> IpStats {
    let mut available = 0i64;
    let mut restricted = 0i64;
    let mut invalid = 0i64;
    for h in healths {
        match h.global_status {
            IpStatus::Invalid => invalid += 1,
            _ if !h.restrictions.is_empty() => restricted += 1,
            _ => available += 1,
        }
    }
    IpStats {
        available,
        restricted,
        invalid,
        total: available + restricted + invalid,
    }
}

/// 账号 per-platform 桶。仅 service 内部使用，不对外。
struct AccountBucket {
    platform: String,
    normal: i64,
    restricted: i64,
    error: i64,
    total: i64,
}

/// 折叠 `(platform, risk_status, count)` 扁平行 → `BTreeMap<platform, AccountBucket>`。
/// 用 BTreeMap 是为了让结果按 platform 字符串排序，前端再按 `PLATFORMS` 顺序重排即可。
/// 未知 `risk_status` 直接忽略，避免后续新增枚举把整列数据顶掉。
fn collect_account_buckets(rows: Vec<(String, String, i64)>) -> BTreeMap<String, AccountBucket> {
    let mut buckets: BTreeMap<String, AccountBucket> = BTreeMap::new();
    for (platform, status, count) in rows {
        let entry = buckets
            .entry(platform.clone())
            .or_insert_with(|| AccountBucket {
                platform,
                normal: 0,
                restricted: 0,
                error: 0,
                total: 0,
            });
        match status.as_str() {
            "normal" => entry.normal += count,
            "restricted" => entry.restricted += count,
            "error" => entry.error += count,
            _ => continue,
        }
        entry.total += count;
    }
    buckets
}

/// 计算「该平台视角下的 IP 三态」——见 `model::stats::PlatformOverview` 的 doc：
/// - invalid: `global_status == Invalid`（**所有平台行都计**，因为出口对谁都不可用）；
/// - restricted: 全局非 invalid 且 `restrictions[].platform == p`；
/// - available: 全局非 invalid 且 `restrictions[]` 不含 p。
fn ip_stats_for_platform(healths: &[ProxyHealthBrief], platform: &str) -> (i64, i64, i64) {
    let mut available = 0i64;
    let mut restricted = 0i64;
    let mut invalid = 0i64;
    for h in healths {
        if matches!(h.global_status, IpStatus::Invalid) {
            invalid += 1;
            continue;
        }
        if h.restrictions.iter().any(|r| r.platform == platform) {
            restricted += 1;
        } else {
            available += 1;
        }
    }
    (available, restricted, invalid)
}

/// 把账号桶与按平台派生的 IP 桶 join 成最终 `Vec<PlatformOverview>`。
/// 行集合 = 账号桶里出现过的平台（即「实际有账号的平台」）。这与首页 UI 的设计一致：
/// 没账号的平台即使能算出 IP 数字，也不会被用来跑任务，没必要占行。
fn join_per_platform(
    account_buckets: BTreeMap<String, AccountBucket>,
    healths: &[ProxyHealthBrief],
) -> Vec<PlatformOverview> {
    account_buckets
        .into_values()
        .map(|b| {
            let (ip_available, ip_restricted, ip_invalid) =
                ip_stats_for_platform(healths, &b.platform);
            PlatformOverview {
                platform: b.platform,
                account_normal: b.normal,
                account_restricted: b.restricted,
                account_error: b.error,
                account_total: b.total,
                ip_available,
                ip_restricted,
                ip_invalid,
            }
        })
        .collect()
}

fn load_recent_logs(conn: &Connection) -> Result<Vec<LogEntry>, AppError> {
    use crate::db::app_event_repo;
    let rows = app_event_repo::list_recent(conn, 40)?;
    Ok(rows
        .into_iter()
        .map(|r| LogEntry {
            time: r.time,
            level: r.level,
            message: r.message,
            scope: r.scope,
            action: r.action,
        })
        .collect())
}
