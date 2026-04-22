use rusqlite::Connection;

use crate::error::AppError;

pub fn run(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tasks (
            id          TEXT PRIMARY KEY,
            platform    TEXT NOT NULL,
            task_type   TEXT NOT NULL,
            name        TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'paused',
            strategy    TEXT NOT NULL DEFAULT 'round_robin',
            rate_limit  INTEGER NOT NULL DEFAULT 60,
            account_pool_size INTEGER NOT NULL DEFAULT 0,
            ip_pool_size      INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS accounts (
            id              TEXT PRIMARY KEY,
            platform        TEXT NOT NULL,
            username        TEXT NOT NULL,
            bound_ip        TEXT,
            bound_proxy_id  TEXT REFERENCES proxies(id) ON DELETE SET NULL,
            risk_status     TEXT NOT NULL DEFAULT 'normal',
            created_at      TEXT NOT NULL,
            last_active_at  TEXT NOT NULL,
            cookies         TEXT
        );

        CREATE TABLE IF NOT EXISTS proxies (
            id              TEXT PRIMARY KEY,
            address         TEXT NOT NULL,
            proxy_type      TEXT NOT NULL,
            remark          TEXT,
            is_system       INTEGER NOT NULL DEFAULT 0,
            geo_country     TEXT,
            geo_region      TEXT,
            geo_city        TEXT,
            geo_isp         TEXT,
            geo_ip          TEXT,
            geo_updated_at  TEXT
        );

        CREATE TABLE IF NOT EXISTS records (
            id              TEXT PRIMARY KEY,
            platform        TEXT NOT NULL,
            task_name       TEXT NOT NULL,
            keyword         TEXT NOT NULL,
            blog_id         TEXT,
            content_preview TEXT NOT NULL,
            author          TEXT NOT NULL,
            crawled_at      TEXT NOT NULL
        );
        ",
    )?;
    migrate_accounts_add_cookies_column(conn)?;
    migrate_proxies_add_remark_column(conn)?;
    migrate_weibo_account_profiles_table(conn)?;
    migrate_weibo_uid_unique_index(conn)?;
    migrate_tasks_bound_account_ids(conn)?;
    migrate_tasks_task_config(conn)?;
    migrate_records_json_data(conn)?;
    migrate_crawl_requests_table(conn)?;
    migrate_crawl_requests_add_response_data(conn)?;
    migrate_records_parent_entity(conn)?;
    migrate_records_blog_id_drop_cleaned_duplicate(conn)?;
    migrate_records_drop_weibo_derived_columns(conn)?;
    migrate_tasks_drop_weighted_strategy(conn)?;
    migrate_tasks_bound_proxy_ids(conn)?;
    migrate_tasks_rate_limit_scope(conn)?;
    migrate_create_failure_events(conn)?;
    migrate_proxies_add_is_system_column(conn)?;
    migrate_proxies_add_geo_columns(conn)?;
    migrate_proxy_failure_events_add_platform(conn)?;
    migrate_proxies_rebuild_drop_legacy(conn)?;
    migrate_create_proxy_platform_runtime(conn)?;
    migrate_create_proxy_latency_probes(conn)?;
    migrate_create_app_settings(conn)?;
    migrate_accounts_bound_proxy_id(conn)?;
    // v7：proxy_latency_probes 行转列 stack 进 proxies；geo_updated_at 与
    // probed_at 合并为 last_probed_at。必须排在 latency_probes / geo_columns
    // 两个迁移之后，确保前置数据齐全后再 backfill。
    migrate_proxies_stack_latency_and_merge_probed_at(conn)?;
    migrate_accounts_slim_add_created_at(conn)?;
    migrate_proxies_add_global_probe_ok(conn)?;
    migrate_app_event_log(conn)?;
    seed_local_direct_proxy(conn)?;
    Ok(())
}

/// 应用事件日志表；首页「最近日志」数据源。已废弃的 `logs` 表若仍存在则删除。
fn migrate_app_event_log(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_event_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            occurred_at TEXT NOT NULL,
            scope TEXT NOT NULL,
            action TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL,
            context_json TEXT,
            subject_type TEXT,
            subject_id TEXT,
            task_id TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_app_event_log_occurred ON app_event_log(occurred_at DESC);
        ",
    )?;
    conn.execute("DROP TABLE IF EXISTS logs", [])?;
    Ok(())
}

/// v9：`global_probe_ok` — 最近一次「国内 + 国外」双探针若**均失败**则为 0（全局不可用），
/// 否则为 1。供 `list_proxies_health` 与风控派生的全局档合并。
fn migrate_proxies_add_global_probe_ok(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('proxies') WHERE name = 'global_probe_ok'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute(
            "ALTER TABLE proxies ADD COLUMN global_probe_ok INTEGER NOT NULL DEFAULT 1",
            [],
        )?;
    }
    Ok(())
}

/// v8：去掉 `login_status` / `token_status` / `assign_status`；新增 `created_at`（添加时间），
/// `last_active_at` 专用于最后活跃时间。老库用 `last_active_at` 回填 `created_at`。
fn migrate_accounts_slim_add_created_at(conn: &Connection) -> Result<(), AppError> {
    let created_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('accounts') WHERE name = 'created_at'",
        [],
        |r| r.get(0),
    )?;
    if created_exists > 0 {
        return Ok(());
    }

    let login_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('accounts') WHERE name = 'login_status'",
        [],
        |r| r.get(0),
    )?;
    if login_exists == 0 {
        conn.execute(
            "ALTER TABLE accounts ADD COLUMN created_at TEXT NOT NULL DEFAULT ''",
            [],
        )?;
        conn.execute(
            "UPDATE accounts SET created_at = last_active_at WHERE created_at = ''",
            [],
        )?;
        return Ok(());
    }

    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = OFF;
        BEGIN IMMEDIATE;
        CREATE TABLE accounts_new (
            id              TEXT PRIMARY KEY,
            platform        TEXT NOT NULL,
            username        TEXT NOT NULL,
            bound_ip        TEXT,
            bound_proxy_id  TEXT REFERENCES proxies(id) ON DELETE SET NULL,
            risk_status     TEXT NOT NULL DEFAULT 'normal',
            created_at      TEXT NOT NULL,
            last_active_at  TEXT NOT NULL,
            cookies         TEXT
        );
        INSERT INTO accounts_new (
            id, platform, username, bound_ip, bound_proxy_id, risk_status, created_at, last_active_at, cookies
        )
        SELECT
            id, platform, username, bound_ip, bound_proxy_id, risk_status,
            last_active_at, last_active_at, cookies
        FROM accounts;
        DROP TABLE accounts;
        ALTER TABLE accounts_new RENAME TO accounts;
        CREATE INDEX IF NOT EXISTS idx_accounts_bound_proxy_id
            ON accounts(bound_proxy_id, platform);
        COMMIT;
        PRAGMA foreign_keys = ON;
        "#,
    )?;
    Ok(())
}

/// v6：给 `accounts` 增加 `bound_proxy_id` 列，作为账号 → 代理的稳定外键。
///
/// 历史背景：早期实现把代理地址（`address`）反写到 `accounts.bound_ip` 做展示，
/// IP 管理页统计「绑定账号数」时不得不按 address 字符串 join，存在歧义
/// （address 重复 / 改写后回填失败）。
///
/// 本迁移：
/// 1. 列不存在则 `ALTER TABLE ADD COLUMN bound_proxy_id TEXT`（外键约束已在
///    新建表语句里写明，老库走 ADD COLUMN 不带 REFERENCES，行为等价）；
/// 2. backfill：按 `bound_ip == proxies.address` 一次性回填 proxy_id；
/// 3. 建索引 `(bound_proxy_id, platform)`，给账号绑定关系按 platform group by 用。
fn migrate_accounts_bound_proxy_id(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('accounts') WHERE name = 'bound_proxy_id'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute("ALTER TABLE accounts ADD COLUMN bound_proxy_id TEXT", [])?;
        // backfill：bound_ip == proxies.address 时回填。LIMIT 1 防御 address 重复。
        conn.execute(
            "UPDATE accounts SET bound_proxy_id = (
                 SELECT p.id FROM proxies p WHERE p.address = accounts.bound_ip LIMIT 1
             ) WHERE bound_proxy_id IS NULL AND bound_ip IS NOT NULL",
            [],
        )?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_accounts_bound_proxy_id
            ON accounts(bound_proxy_id, platform)",
        [],
    )?;
    Ok(())
}

/// v5：重建 `proxies` 表，把以下五列彻底丢掉（详见 `model/proxy.rs` 注释）：
/// `status / bound_account_count / running_task_count / risk_score / latency`。
///
/// 实现路径走 SQLite 官方推荐的「CREATE TEMP → INSERT SELECT → DROP → RENAME」
/// 模式，避免依赖 SQLite 3.35+ 的 `ALTER TABLE ... DROP COLUMN`：
/// - 先用 PRAGMA 判断 `risk_score` 列是否还在，没在就直接跳过（已迁移）；
/// - 在事务里搬数据；
/// - 顺手把 seed 行的 `is_system` 置为 1（旧迁移已经做过，这里只是兜底）。
///
/// 列被删后，新版 `proxy_repo::insert/list/get_by_id` 都不再 SELECT 这些列；
/// 风控分（per-platform）改由 `proxy_service::compute_platform_risk_score`
/// 在线计算；延迟改由 `proxy_latency_probes` 维护。
fn migrate_proxies_rebuild_drop_legacy(conn: &Connection) -> Result<(), AppError> {
    let still_legacy: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('proxies') WHERE name = 'risk_score'",
        [],
        |r| r.get(0),
    )?;
    if still_legacy == 0 {
        return Ok(());
    }

    // 一次性事务，避免中途宕机后表名残留。
    conn.execute_batch(
        "BEGIN;
         CREATE TABLE proxies_new (
             id              TEXT PRIMARY KEY,
             address         TEXT NOT NULL,
             proxy_type      TEXT NOT NULL,
             remark          TEXT,
             is_system       INTEGER NOT NULL DEFAULT 0,
             geo_country     TEXT,
             geo_region      TEXT,
             geo_city        TEXT,
             geo_isp         TEXT,
             geo_ip          TEXT,
             geo_updated_at  TEXT
         );
         INSERT INTO proxies_new (id, address, proxy_type, remark, is_system,
                                  geo_country, geo_region, geo_city, geo_isp, geo_ip, geo_updated_at)
         SELECT id, address, proxy_type, remark, COALESCE(is_system, 0),
                geo_country, geo_region, geo_city, geo_isp, geo_ip, geo_updated_at
           FROM proxies;
         DROP TABLE proxies;
         ALTER TABLE proxies_new RENAME TO proxies;
         COMMIT;",
    )?;
    Ok(())
}

/// v5：per-(proxy, platform) 维度的"最近一次响应"画像表。
/// 详见 `db::proxy_runtime_repo` 模块注释。
fn migrate_create_proxy_platform_runtime(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS proxy_platform_runtime (
            proxy_id          TEXT NOT NULL,
            platform          TEXT NOT NULL,
            last_responded_at TEXT,
            last_account_id   TEXT,
            last_latency_ms   INTEGER,
            last_status       TEXT,
            last_error_kind   TEXT,
            last_http_status  INTEGER,
            PRIMARY KEY (proxy_id, platform),
            FOREIGN KEY (proxy_id) REFERENCES proxies(id) ON DELETE CASCADE
        );",
    )?;
    Ok(())
}

/// v5：双探针（cn / intl）延迟样本表。
///
/// **v7 起该表被 `migrate_proxies_stack_latency_and_merge_probed_at` 行转列搬入
/// `proxies`，物理表已 DROP**。这里 `CREATE TABLE IF NOT EXISTS` 保留是为了让
/// stack 迁移本身能跑：老 DB 升级路径 = 先 v5 建表 → v7 backfill → v7 DROP。
/// 重新跑迁移时表已经被 DROP，下一次再升级也不会重新 backfill（v7 用列存在性
/// 当幂等开关）。
fn migrate_create_proxy_latency_probes(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS proxy_latency_probes (
            proxy_id   TEXT NOT NULL,
            target     TEXT NOT NULL,
            latency_ms INTEGER NOT NULL,
            probed_at  TEXT NOT NULL,
            PRIMARY KEY (proxy_id, target),
            FOREIGN KEY (proxy_id) REFERENCES proxies(id) ON DELETE CASCADE
        );",
    )?;
    Ok(())
}

/// v7：把 `proxy_latency_probes` 的两行 (cn / intl) 行转列搬到 `proxies`，
/// 同时把 `geo_updated_at` 与 `probed_at` 合并为 `last_probed_at`。
///
/// 幂等开关：检测 `proxies.cn_latency_ms` 列是否已存在——存在即视为本迁移已跑。
///
/// 步骤（一次性事务，避免半成品）：
/// 1. CREATE TABLE proxies_new（带 cn_latency_ms / intl_latency_ms / last_probed_at，
///    没有 geo_updated_at）；
/// 2. INSERT SELECT：从老 proxies 拷元数据，从 proxy_latency_probes 子查询拉
///    cn / intl 的 ms；`last_probed_at = MAX(geo_updated_at, cn.probed_at, intl.probed_at)`，
///    保留信息最丰富的那一份；
/// 3. DROP 老 proxies + RENAME proxies_new → proxies；
/// 4. DROP TABLE proxy_latency_probes（数据已迁出，老表彻底退役）。
///
/// 注：`COALESCE(MAX(...))` 用 SQLite 的 `MAX()` 单值函数（无聚合 group），
/// 它会返回非 NULL 的最大字符串。`YYYY-MM-DD HH:MM:SS` 字符串可直接按字典序
/// 比较与按时间序一致，无需先 DATETIME 转换。
fn migrate_proxies_stack_latency_and_merge_probed_at(conn: &Connection) -> Result<(), AppError> {
    let already_done: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('proxies') WHERE name = 'cn_latency_ms'",
        [],
        |r| r.get(0),
    )?;
    if already_done > 0 {
        return Ok(());
    }

    conn.execute_batch(
        "BEGIN;
         CREATE TABLE proxies_new (
             id              TEXT PRIMARY KEY,
             address         TEXT NOT NULL,
             proxy_type      TEXT NOT NULL,
             remark          TEXT,
             is_system       INTEGER NOT NULL DEFAULT 0,
             geo_country     TEXT,
             geo_region      TEXT,
             geo_city        TEXT,
             geo_isp         TEXT,
             geo_ip          TEXT,
             cn_latency_ms   INTEGER,
             intl_latency_ms INTEGER,
             last_probed_at  TEXT
         );
         INSERT INTO proxies_new (
             id, address, proxy_type, remark, is_system,
             geo_country, geo_region, geo_city, geo_isp, geo_ip,
             cn_latency_ms, intl_latency_ms, last_probed_at
         )
         SELECT p.id, p.address, p.proxy_type, p.remark, COALESCE(p.is_system, 0),
                p.geo_country, p.geo_region, p.geo_city, p.geo_isp, p.geo_ip,
                (SELECT pr.latency_ms FROM proxy_latency_probes pr
                   WHERE pr.proxy_id = p.id AND pr.target = 'cn'),
                (SELECT pr.latency_ms FROM proxy_latency_probes pr
                   WHERE pr.proxy_id = p.id AND pr.target = 'intl'),
                COALESCE(
                    (SELECT MAX(pr.probed_at) FROM proxy_latency_probes pr
                       WHERE pr.proxy_id = p.id),
                    p.geo_updated_at
                )
           FROM proxies p;
         DROP TABLE proxies;
         ALTER TABLE proxies_new RENAME TO proxies;
         DROP TABLE IF EXISTS proxy_latency_probes;
         COMMIT;",
    )?;
    Ok(())
}

/// v5：通用 KV 配置表。当前承载「双探针目标 URL」配置。
fn migrate_create_app_settings(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;
    Ok(())
}

/// v4 / 方案 C：给 `proxy_failure_events` 增加 `platform` 列，把代理风控
/// 从「跨平台一锅汇总」拆成「按 (proxy, platform) scope」。
///
/// 历史数据：老库里的事件没有 platform 字段，能从 `task_id` 反查
/// `tasks.platform` 的就一次性回填；查不到（比如手动重置或 task 已删）
/// 留 NULL，新版本写入路径已经强制带 platform，新增事件不会再产生 NULL。
///
/// 索引：在 `(proxy_id, platform, occurred_at DESC)` 上建一个，per-platform
/// 滑窗 count 不会比原来慢。
fn migrate_proxy_failure_events_add_platform(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('proxy_failure_events') WHERE name = 'platform'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute("ALTER TABLE proxy_failure_events ADD COLUMN platform TEXT", [])?;
        // 历史事件按 task_id 反查 tasks.platform 一次性回填。
        // 用 IS NULL 守门，重复跑也只会刷一次。
        conn.execute(
            "UPDATE proxy_failure_events
                SET platform = (
                    SELECT t.platform FROM tasks t WHERE t.id = proxy_failure_events.task_id
                )
              WHERE platform IS NULL AND task_id IS NOT NULL",
            [],
        )?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_proxy_fail_proxy_platform_time
            ON proxy_failure_events(proxy_id, platform, occurred_at DESC)",
        [],
    )?;
    Ok(())
}

// NOTE: 历史上 v3 短暂引入过 `migrate_proxy_failure_events_add_account_id`
// 给 `proxy_failure_events` 增加 `account_id` 列，用于在 IP 日志 modal 中
// 反向展示「该次代理故障是哪个账号触发的」。后续业务上明确：IP 与账号
// 仅在「任务配置」做笛卡尔积，运行期不再做反向耦合，遂删除该迁移。
//
// 已经升级过的 dev 数据库会留下一列空闲的 `account_id` 列，新版本既不读也不写，
// 不影响功能；如需彻底清理可手工 DROP，但 SQLite DROP COLUMN 需要 3.35+，
// 不值得为此再写一个迁移。

/// 给 `proxies` 增加 IP 反查得到的地理位置 / ISP 字段。新列允许 NULL，
/// 老库迁移完不会立刻有值——由 `service::geoip` 在用户「添加代理」或者
/// 主动「刷新地理信息」时填充。
fn migrate_proxies_add_geo_columns(conn: &Connection) -> Result<(), AppError> {
    for (col, sql) in [
        ("geo_country", "ALTER TABLE proxies ADD COLUMN geo_country TEXT"),
        ("geo_region", "ALTER TABLE proxies ADD COLUMN geo_region TEXT"),
        ("geo_city", "ALTER TABLE proxies ADD COLUMN geo_city TEXT"),
        ("geo_isp", "ALTER TABLE proxies ADD COLUMN geo_isp TEXT"),
        ("geo_ip", "ALTER TABLE proxies ADD COLUMN geo_ip TEXT"),
        ("geo_updated_at", "ALTER TABLE proxies ADD COLUMN geo_updated_at TEXT"),
    ] {
        let n: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('proxies') WHERE name = '{col}'"),
            [],
            |r| r.get(0),
        )?;
        if n == 0 {
            conn.execute(sql, [])?;
        }
    }
    Ok(())
}

/// 给 `proxies` 加 `is_system` 标志，用于标记 `local-direct` 这类系统内置行。
fn migrate_proxies_add_is_system_column(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('proxies') WHERE name = 'is_system'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute(
            "ALTER TABLE proxies ADD COLUMN is_system INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}

/// 写入「本机直连」伪代理行；已存在则跳过。该行 worker 拿到后直接构造无代理 client，
/// 同时被 `bound_proxy_ids` 等价默认值使用，让直连路径也能进风控统计。
fn seed_local_direct_proxy(conn: &Connection) -> Result<(), AppError> {
    use crate::model::proxy::{ProxyIp, LOCAL_DIRECT_PROXY_ID};

    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM proxies WHERE id = ?1",
        rusqlite::params![LOCAL_DIRECT_PROXY_ID],
        |r| r.get(0),
    )?;
    if exists > 0 {
        // 已存在则只补 is_system 标志，避免老库 seed 行没标 system 导致前端能删。
        conn.execute(
            "UPDATE proxies SET is_system = 1 WHERE id = ?1",
            rusqlite::params![LOCAL_DIRECT_PROXY_ID],
        )?;
        return Ok(());
    }
    let p = ProxyIp::local_direct_template();
    // v5：proxies 已经重建，不再有 status / bound_*/running_*/risk_score/latency 列。
    conn.execute(
        "INSERT INTO proxies (id, address, proxy_type, remark, is_system) \
         VALUES (?1, ?2, ?3, ?4, 1)",
        rusqlite::params![
            p.id,
            p.address,
            crate::db::enum_to_str(&p.proxy_type),
            p.remark,
        ],
    )?;
    Ok(())
}

/// 风控失败事件表：账号 / 代理各一张，统一 schema。worker 在请求失败时
/// 落一行；`queue::risk::evaluate` 用滑窗计数判定是否升级状态。
/// 旧记录由 `run_scheduler` 退出前 `purge_older_than(now - 24h)` 清理。
fn migrate_create_failure_events(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS account_failure_events (
            id           TEXT PRIMARY KEY,
            account_id   TEXT NOT NULL,
            task_id      TEXT,
            request_id   TEXT,
            error_kind   TEXT NOT NULL,
            http_status  INTEGER,
            message      TEXT,
            occurred_at  TEXT NOT NULL,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_acc_fail_acc_time
            ON account_failure_events(account_id, occurred_at DESC);

        CREATE TABLE IF NOT EXISTS proxy_failure_events (
            id           TEXT PRIMARY KEY,
            proxy_id     TEXT NOT NULL,
            task_id      TEXT,
            request_id   TEXT,
            error_kind   TEXT NOT NULL,
            http_status  INTEGER,
            message      TEXT,
            occurred_at  TEXT NOT NULL,
            FOREIGN KEY (proxy_id) REFERENCES proxies(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_proxy_fail_proxy_time
            ON proxy_failure_events(proxy_id, occurred_at DESC);
        ",
    )?;
    Ok(())
}

/// 任务新增「绑定代理 id 列表」（JSON 字符串），用于并发模式按 (账号 × 代理) 起 worker。
fn migrate_tasks_bound_proxy_ids(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'bound_proxy_ids'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute("ALTER TABLE tasks ADD COLUMN bound_proxy_ids TEXT", [])?;
    }
    Ok(())
}

/// 任务新增「限流粒度」字段（per_worker / per_account），默认按 worker。
fn migrate_tasks_rate_limit_scope(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'rate_limit_scope'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN rate_limit_scope TEXT NOT NULL DEFAULT 'per_worker'",
            [],
        )?;
    }
    Ok(())
}

/// 移除已废弃的 `weighted` 采集策略：现存任务回退到 `round_robin`。
fn migrate_tasks_drop_weighted_strategy(conn: &Connection) -> Result<(), AppError> {
    let has_table: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='tasks'",
        [],
        |r| r.get(0),
    )?;
    if has_table == 0 {
        return Ok(());
    }
    conn.execute(
        "UPDATE tasks SET strategy = 'round_robin' WHERE strategy = 'weighted'",
        [],
    )?;
    Ok(())
}

/// 移除已废弃的微博派生列（完整数据在 `json_data`）。
fn migrate_records_drop_weibo_derived_columns(conn: &Connection) -> Result<(), AppError> {
    for col in [
        "weibo_mblogid",
        "weibo_post_uid",
        "weibo_profile_uid",
        "weibo_uuid",
    ] {
        let n: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('records') WHERE name = '{col}'"),
            [],
            |r| r.get(0),
        )?;
        if n > 0 {
            conn.execute(
                &format!("ALTER TABLE records DROP COLUMN {col}"),
                [],
            )?;
        }
    }
    Ok(())
}

fn migrate_records_json_data(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('records') WHERE name = 'json_data'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute("ALTER TABLE records ADD COLUMN json_data TEXT", [])?;
    }
    Ok(())
}

fn migrate_tasks_bound_account_ids(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'bound_account_ids'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute("ALTER TABLE tasks ADD COLUMN bound_account_ids TEXT", [])?;
    }
    Ok(())
}

fn migrate_tasks_task_config(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'task_config'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute("ALTER TABLE tasks ADD COLUMN task_config TEXT", [])?;
    }
    Ok(())
}

/// 同一微博 uid 只绑定一条账号（防止重复扫码入库）。
fn migrate_weibo_uid_unique_index(conn: &Connection) -> Result<(), AppError> {
    conn.execute("DROP INDEX IF EXISTS idx_weibo_profiles_uid", [])?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS ux_weibo_account_profiles_uid ON weibo_account_profiles(weibo_uid)",
        [],
    )?;
    Ok(())
}

fn migrate_weibo_account_profiles_table(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS weibo_account_profiles (
            account_id          TEXT PRIMARY KEY,
            weibo_uid           TEXT NOT NULL,
            center_weibo_name   TEXT,
            updated_at          TEXT NOT NULL,
            FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE
        );
        ",
    )?;
    Ok(())
}

/// 旧库无 `cookies` 列时追加（新库已在 CREATE 中带上时可跳过）。
fn migrate_accounts_add_cookies_column(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('accounts') WHERE name = 'cookies'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute("ALTER TABLE accounts ADD COLUMN cookies TEXT", [])?;
    }
    Ok(())
}

fn migrate_crawl_requests_table(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS crawl_requests (
            id                TEXT PRIMARY KEY,
            task_id           TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            request_type      TEXT NOT NULL,
            request_params    TEXT NOT NULL,
            status            TEXT NOT NULL DEFAULT 'pending',
            account_id        TEXT,
            proxy_id          TEXT,
            error_message     TEXT,
            response_summary  TEXT,
            response_data     TEXT,
            parent_request_id TEXT,
            retry_count       INTEGER NOT NULL DEFAULT 0,
            created_at        TEXT NOT NULL,
            updated_at        TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_crawl_requests_task_status
            ON crawl_requests(task_id, status);
        ",
    )?;
    Ok(())
}

fn migrate_records_parent_entity(conn: &Connection) -> Result<(), AppError> {
    for (col, sql) in [
        ("parent_record_id", "ALTER TABLE records ADD COLUMN parent_record_id TEXT"),
        ("entity_type", "ALTER TABLE records ADD COLUMN entity_type TEXT"),
    ] {
        let n: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('records') WHERE name = '{col}'"),
            [],
            |r| r.get(0),
        )?;
        if n == 0 {
            conn.execute(sql, [])?;
        }
    }
    Ok(())
}

/// `keyword` 仅保留搜索词；博文标识迁入 `blog_id`；移除 `cleaned` / `duplicate`。
fn migrate_records_blog_id_drop_cleaned_duplicate(conn: &Connection) -> Result<(), AppError> {
    let has_blog_id: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('records') WHERE name = 'blog_id'",
        [],
        |r| r.get(0),
    )?;
    if has_blog_id == 0 {
        conn.execute("ALTER TABLE records ADD COLUMN blog_id TEXT", [])?;
    }

    // 旧数据：正文/评论曾把 mblogid/mid/status_id 存在 `keyword`。
    conn.execute(
        "UPDATE records SET blog_id = keyword \
         WHERE entity_type IN ('body','comment_l1','comment_l2') \
           AND (blog_id IS NULL OR TRIM(COALESCE(blog_id, '')) = '')",
        [],
    )?;
    conn.execute(
        "UPDATE records SET keyword = '' \
         WHERE entity_type IN ('body','comment_l1','comment_l2')",
        [],
    )?;

    conn.execute(
        r#"UPDATE records
        SET blog_id = COALESCE(
            NULLIF(TRIM(json_extract(json_data, '$.mblogid')), ''),
            NULLIF(TRIM(json_extract(json_data, '$.mid')), '')
        )
        WHERE COALESCE(entity_type, '') = 'feed'
          AND (blog_id IS NULL OR TRIM(COALESCE(blog_id, '')) = '')
          AND json_data IS NOT NULL AND TRIM(json_data) != ''"#,
        [],
    )?;

    for col in ["cleaned", "duplicate"] {
        let n: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('records') WHERE name = '{col}'"),
            [],
            |r| r.get(0),
        )?;
        if n > 0 {
            conn.execute(
                &format!("ALTER TABLE records DROP COLUMN {col}"),
                [],
            )?;
        }
    }
    Ok(())
}

fn migrate_crawl_requests_add_response_data(conn: &Connection) -> Result<(), AppError> {
    let has_table: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='crawl_requests'",
        [],
        |r| r.get(0),
    )?;
    if has_table == 0 {
        return Ok(());
    }
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('crawl_requests') WHERE name = 'response_data'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute("ALTER TABLE crawl_requests ADD COLUMN response_data TEXT", [])?;
    }
    Ok(())
}

fn migrate_proxies_add_remark_column(conn: &Connection) -> Result<(), AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('proxies') WHERE name = 'remark'",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute("ALTER TABLE proxies ADD COLUMN remark TEXT", [])?;
    }
    Ok(())
}
