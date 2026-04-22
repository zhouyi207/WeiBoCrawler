use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::db::risk_event_repo::{self, ProxyLogEntry};
use crate::db::{app_event_repo, enum_to_str, proxy_repo, proxy_runtime_repo, Database};
use crate::error::AppError;
use crate::model::proxy::{
    IpStatus, LatencyOutcome, ProxyGeoInfo, ProxyGlobalRow, ProxyIp, ProxyPlatformRow, ProxyType,
};
use crate::queue::registry::WorkerRegistry;
use crate::queue::risk;
use crate::service::{geoip, settings_service};

/// 单次 HTTP 请求的超时上限。两次（预热 + 计时）总耗时受 `PROBE_TIMEOUT_EACH * 2` 约束。
/// **5s** 与 `geoip::LOOKUP_TIMEOUT` 对齐：add_proxy / check_all 时把
/// geo + cn + intl 三件事并行 spawn，最坏 ~10s = max(geo: 5+5, cn: 5+5, intl: 5+5)。
const PROBE_TIMEOUT_EACH: Duration = Duration::from_secs(5);

/// `latency` 列约定的「失败」哨兵。
const PROBE_FAIL_LATENCY: i64 = -1;

/// 批量延迟探针并发上限。本机出口带宽 + sqlite 写盘综合考量。
/// 每条代理内部的 cn / intl 又各开一个 thread → 实际同时 in-flight = N×2 次 HTTP，
/// 因此这里取一半的"代理"并发；ip-api 限速 45 req/min 对每代理 1 次 geo 也够（30s 内最多 16 次）。
const DUAL_PROBE_CONCURRENCY: usize = 16;

pub fn list_proxies(db: &Database) -> Result<Vec<ProxyIp>, AppError> {
    let conn = db.conn();
    proxy_repo::list(&conn)
}

pub fn add_proxy(
    db: &Database,
    address: &str,
    proxy_type: &str,
    remark: Option<String>,
) -> Result<ProxyIp, AppError> {
    let parsed_type = crate::db::str_to_enum::<ProxyType>(proxy_type)?;
    if matches!(parsed_type, ProxyType::Direct) {
        // 用户手工添加 Direct 行没意义：local-direct 系统行已经覆盖。
        return Err(AppError::Internal(
            "不允许手工添加 Direct 类型，本机直连请使用系统内置的 local-direct 行".to_string(),
        ));
    }
    let proxy = ProxyIp {
        id: uuid::Uuid::new_v4().to_string(),
        address: address.to_string(),
        proxy_type: parsed_type,
        remark,
        is_system: false,
        geo_country: None,
        geo_region: None,
        geo_city: None,
        geo_isp: None,
        geo_ip: None,
        cn_latency_ms: None,
        intl_latency_ms: None,
        last_probed_at: None,
        global_probe_ok: true,
    };
    {
        let conn = db.conn();
        proxy_repo::insert(&conn, &proxy)?;
    }

    // 同步并行：geo + cn 双探针 + intl 双探针，三件事一起跑（≤ ~10s）。
    // 失败不抛错，只是对应字段写空 / 写哨兵；用户随后可在 IP 列表里点
    // 「刷新并测延迟」覆盖。settings 单独读，不阻塞探针 spawn。
    let settings = settings_service::get_proxy_probe_settings(db)?;
    let (geo_info, cn_ms, intl_ms) =
        probe_one_full(&proxy, &settings.cn_target, &settings.intl_target);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    {
        let conn = db.conn();
        if let Err(e) = proxy_repo::update_geo_and_latency(
            &conn,
            &proxy.id,
            geo_info.as_ref(),
            Some(cn_ms),
            Some(intl_ms),
            &now,
        ) {
            log::warn!(
                "[proxy_service] update_geo_and_latency({}) 失败: {e}",
                proxy.id
            );
        }
    }

    // 重新读出来一次，把刚写好的 geo / 双探针字段一起返回给前端，省掉一次 list_proxies。
    let conn = db.conn();
    let out = proxy_repo::get_by_id(&conn, &proxy.id)?;
    app_event_repo::try_insert(
        &conn,
        "proxy",
        "create",
        "info",
        &format!(
            "添加代理 {}（{}）",
            out.address,
            enum_to_str(&out.proxy_type)
        ),
        Some("proxy"),
        Some(&out.id),
        None,
    );
    Ok(out)
}

pub fn delete_proxy(db: &Database, id: &str) -> Result<(), AppError> {
    let conn = db.conn();
    let msg = match proxy_repo::get_by_id(&conn, id) {
        Ok(p) => format!("删除代理 {}（{}）", p.address, enum_to_str(&p.proxy_type)),
        Err(_) => format!("删除代理（id={id}）"),
    };
    proxy_repo::delete(&conn, id)?;
    app_event_repo::try_insert(
        &conn,
        "proxy",
        "delete",
        "info",
        &msg,
        Some("proxy"),
        Some(id),
        None,
    );
    Ok(())
}

/// 编辑用户已存在的代理。校验：
/// - `proxy_type` 必须能解析；不允许改成 `Direct`（与 `add_proxy` 同语义）；
/// - 系统行（`is_system`）会被 [`proxy_repo::update`] 直接拒绝。
///
/// 行为：
/// - 仅更新 address / proxy_type / remark；
/// - **若 address 发生变化**：同步并行重跑 geo + cn + intl 三件事
///   （与 `add_proxy` 一致）。语义升级：address 变 = 出口换了，旧的双探针
///   样本必然过期，必须一并刷新；
/// - 返回最新行（含可能刷新过的 geo / 双探针），前端不需要再 refetch。
pub fn update_proxy(
    db: &Database,
    id: &str,
    address: &str,
    proxy_type: &str,
    remark: Option<String>,
) -> Result<ProxyIp, AppError> {
    let parsed_type = crate::db::str_to_enum::<ProxyType>(proxy_type)?;
    if matches!(parsed_type, ProxyType::Direct) {
        return Err(AppError::Internal(
            "不允许把代理改成 Direct，本机直连请使用系统内置的 local-direct 行".to_string(),
        ));
    }

    // 先取出原行，比对 address 决定要不要刷 geo + 双探针。
    let old = {
        let conn = db.conn();
        proxy_repo::get_by_id(&conn, id)?
    };

    {
        let conn = db.conn();
        proxy_repo::update(&conn, id, address, &parsed_type, remark.as_deref())?;
    }

    if old.address != address {
        // 重新构造一份用于反查 / 探测的视图——只有 address / proxy_type 影响出口路径。
        let preview = ProxyIp {
            address: address.to_string(),
            proxy_type: parsed_type,
            ..old
        };
        let settings = settings_service::get_proxy_probe_settings(db)?;
        let (info, cn_ms, intl_ms) =
            probe_one_full(&preview, &settings.cn_target, &settings.intl_target);
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let conn = db.conn();
        if let Err(e) = proxy_repo::update_geo_and_latency(
            &conn,
            id,
            info.as_ref(),
            Some(cn_ms),
            Some(intl_ms),
            &now,
        ) {
            log::warn!("[proxy_service] update_geo_and_latency({id}) 失败: {e}");
        }
    }

    let conn = db.conn();
    let out = proxy_repo::get_by_id(&conn, id)?;
    app_event_repo::try_insert(
        &conn,
        "proxy",
        "update",
        "info",
        &format!(
            "更新代理 {}（{}）",
            out.address,
            enum_to_str(&out.proxy_type)
        ),
        Some("proxy"),
        Some(id),
        None,
    );
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// 双探针：cn + intl。每次 batch 跑一次，把 `proxy_latency_probes` 的两行覆盖。
// ─────────────────────────────────────────────────────────────────────────────

/// 单代理 ad-hoc 健康探测：仅返回 ms（>=0 / -1），**不**写库。
/// 留作前端「单条快速测一下」的轻量入口（如有需要）。
#[allow(dead_code)]
pub fn check_health_once(db: &Database, id: &str, target_url: &str) -> Result<i64, AppError> {
    let proxy = {
        let conn = db.conn();
        proxy_repo::get_by_id(&conn, id)?
    };
    Ok(probe_one(&proxy, target_url))
}

/// 批量并发探测所有代理（geo + cn + intl 三件事并行），结果一次性写回
/// `proxies` 行（cn_latency_ms / intl_latency_ms / geo_* / last_probed_at），
/// 然后返回组装好的「全局」行列表给前端。
///
/// 用户视角 = 「刷新并测延迟」按钮：geo 反查与双延迟探针都包进来，单次
/// 操作刷新一条代理的全部时序信息。
///
/// **并发布局**：外层按 `DUAL_PROBE_CONCURRENCY` 分块；每条代理内部
/// `probe_one_full` 再 spawn 3 个 thread（geo / cn / intl），所以最大同时 in-flight
/// HTTP ≤ `DUAL_PROBE_CONCURRENCY × 3 = 48`。
pub fn check_all_proxies_dual_health(db: &Database) -> Result<Vec<ProxyGlobalRow>, AppError> {
    let settings = settings_service::get_proxy_probe_settings(db)?;
    let cn_target = settings.cn_target;
    let intl_target = settings.intl_target;
    let proxies = list_proxies(db)?;

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let conn = db.conn();

    for chunk in proxies.chunks(DUAL_PROBE_CONCURRENCY) {
        let mut handles = Vec::with_capacity(chunk.len());
        for p in chunk {
            let p = p.clone();
            let id = p.id.clone();
            let cn = cn_target.clone();
            let intl = intl_target.clone();
            handles.push(thread::spawn(move || {
                let (info, cn_ms, intl_ms) = probe_one_full(&p, &cn, &intl);
                (id, info, cn_ms, intl_ms)
            }));
        }
        for h in handles {
            if let Ok((id, info, cn_ms, intl_ms)) = h.join() {
                if let Err(e) = proxy_repo::update_geo_and_latency(
                    &conn,
                    &id,
                    info.as_ref(),
                    Some(cn_ms),
                    Some(intl_ms),
                    &now,
                ) {
                    log::warn!("[proxy_service] update_geo_and_latency({id}) 失败: {e}");
                }
            }
        }
    }
    drop(conn);

    list_proxies_global(db)
}

/// 「全局」tab 的数据装配：基础元数据 + 双延迟样本。
///
/// v7 stack 后所有数据都在 `proxies` 行内，不再外联 `proxy_latency_probes`：
/// 直接 `LatencyOutcome::from_proxy_columns(ms, last_probed_at)` 现场派生。
pub fn list_proxies_global(db: &Database) -> Result<Vec<ProxyGlobalRow>, AppError> {
    let proxies = list_proxies(db)?;
    let mut out = Vec::with_capacity(proxies.len());
    for p in proxies {
        let cn = LatencyOutcome::from_proxy_columns(p.cn_latency_ms, p.last_probed_at.as_deref());
        let intl =
            LatencyOutcome::from_proxy_columns(p.intl_latency_ms, p.last_probed_at.as_deref());
        out.push(ProxyGlobalRow {
            proxy: p,
            cn_latency: cn,
            intl_latency: intl,
        });
    }
    Ok(out)
}

/// 一条代理的"完整画像"探测：并行跑 geo / cn / intl 三件事，
/// 所有 reqwest::blocking 工作都丢到独立 OS 线程（避免在 tokio runtime 内 drop runtime）。
///
/// 返回 `(geo_info, cn_ms, intl_ms)`：
/// - `geo_info = None` 表示反查失败；写库时调 `update_geo_and_latency(.., None, ..)` 会清空 geo*；
/// - `cn_ms / intl_ms` 用 `>=0 / -1` 哨兵语义，由 `probe_one` 决定。
fn probe_one_full(
    proxy: &ProxyIp,
    cn_target: &str,
    intl_target: &str,
) -> (Option<ProxyGeoInfo>, i64, i64) {
    let p_geo = proxy.clone();
    let geo_handle = thread::spawn(move || geoip::lookup_blocking(&p_geo));

    let p_cn = proxy.clone();
    let cn_target_owned = cn_target.to_string();
    let cn_handle = thread::spawn(move || probe_one(&p_cn, &cn_target_owned));

    let p_intl = proxy.clone();
    let intl_target_owned = intl_target.to_string();
    let intl_handle = thread::spawn(move || probe_one(&p_intl, &intl_target_owned));

    // join 任何一条 panic 都退化成默认值，不让一个线程的崩溃整票连坐。
    let geo = geo_handle.join().unwrap_or_else(|payload| {
        log::warn!("[proxy_service] probe_one_full geo worker panicked: {payload:?}");
        None
    });
    let cn_ms = cn_handle.join().unwrap_or_else(|payload| {
        log::warn!("[proxy_service] probe_one_full cn worker panicked: {payload:?}");
        PROBE_FAIL_LATENCY
    });
    let intl_ms = intl_handle.join().unwrap_or_else(|payload| {
        log::warn!("[proxy_service] probe_one_full intl worker panicked: {payload:?}");
        PROBE_FAIL_LATENCY
    });

    (geo, cn_ms, intl_ms)
}

/// 拉取某代理最近 `limit` 条日志；上限 200，避免一次返回过多。
pub fn list_proxy_logs(
    db: &Database,
    id: &str,
    limit: Option<i64>,
) -> Result<Vec<ProxyLogEntry>, AppError> {
    let lim = limit.unwrap_or(100).clamp(1, 200);
    let conn = db.conn();
    risk_event_repo::list_proxy_logs(&conn, id, lim)
}

/// 单条 (proxy, platform) 受限项。出现在 [`ProxyHealthBrief::restrictions`] 列表里。
/// 仅在派生结果是 `Restricted` 时才进入；`Available` 平台不会被列出。
///
/// 注：v4 起 `Invalid` 一律是全局判定（出口连不上任何平台），由
/// [`ProxyHealthBrief::global_status`] 表达，不会出现在该列表中。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyRestriction {
    pub platform: String,
    pub status: IpStatus,
}

/// IP 健康摘要。v4 起拆成：
/// - `global_status`：全局档（仅 Available / Invalid），出口本身的连通性；
/// - `restrictions`：(IP, platform) 维度被判 Restricted 的平台列表，N 一般 0~2。
///
/// CreateTaskModal 用 `restrictions` 过滤当前任务平台是否受限；IP 页面用
/// `restrictions.len()` 渲染聚合「⚠ N 平台」徽章。Dashboard 仅看 `global_status`。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyHealthBrief {
    pub id: String,
    pub global_status: IpStatus,
    pub restrictions: Vec<ProxyRestriction>,
}

/// 批量派生每条代理的健康摘要。
///
/// **批量化**（v6）：
/// - 全局档：调 `derive_proxy_status_batch(.., None)` 单次扫表派生；
/// - per-platform：先 `group_proxy_failure_platforms_since` 一次拿到「每个 proxy
///   在哪些 platform 上有过失败」，再按 platform 分组派生（一个 platform 对应
///   一组 4 条 SQL）。复杂度 O(unique_platforms) 而非 O(N)。
pub fn list_proxies_health(db: &Database) -> Result<Vec<ProxyHealthBrief>, AppError> {
    let proxies = list_proxies(db)?;
    let conn = db.conn();
    let since = Utc::now() - risk::WINDOW;

    let proxy_keys: Vec<(String, ProxyType)> =
        proxies.iter().map(|p| (p.id.clone(), p.proxy_type)).collect();
    let global_status_map = risk::derive_proxy_status_batch(&conn, &proxy_keys, None)?;

    // (proxy_id → [platform, ...])
    let plat_map = risk_event_repo::group_proxy_failure_platforms_since(&conn, since)
        .unwrap_or_default();

    // 反向：platform → 该 platform 上有失败记录的 proxy_keys 子集。
    let mut by_plat: HashMap<String, Vec<(String, ProxyType)>> = HashMap::new();
    let proxy_type_by_id: HashMap<String, ProxyType> = proxies
        .iter()
        .map(|p| (p.id.clone(), p.proxy_type))
        .collect();
    for (pid, plats) in &plat_map {
        let Some(pt) = proxy_type_by_id.get(pid).copied() else { continue };
        for p in plats {
            by_plat.entry(p.clone()).or_default().push((pid.clone(), pt));
        }
    }

    // 每个 platform 跑一次 batch，结果落 (proxy_id, platform) → IpStatus。
    let mut platform_status: HashMap<(String, String), IpStatus> = HashMap::new();
    for (plat, keys) in &by_plat {
        if let Ok(m) = risk::derive_proxy_status_batch(&conn, keys, Some(plat)) {
            for (pid, st) in m {
                platform_status.insert((pid, plat.clone()), st);
            }
        }
    }
    drop(conn);

    let mut out = Vec::with_capacity(proxies.len());
    for p in proxies {
        // 双探针均失败时，库内 `global_probe_ok = false`，全局档强制不可用（优先于风控滑窗）。
        let risk_global = global_status_map
            .get(&p.id)
            .copied()
            .unwrap_or(IpStatus::Available);
        let global_status = if !p.global_probe_ok {
            IpStatus::Invalid
        } else {
            risk_global
        };

        let restrictions = if matches!(global_status, IpStatus::Invalid) {
            Vec::new()
        } else if let Some(plats) = plat_map.get(&p.id) {
            let mut acc = Vec::with_capacity(plats.len());
            for plat in plats {
                let st = platform_status
                    .get(&(p.id.clone(), plat.clone()))
                    .copied()
                    .unwrap_or(IpStatus::Available);
                if matches!(st, IpStatus::Restricted) {
                    acc.push(ProxyRestriction { platform: plat.clone(), status: st });
                }
            }
            acc
        } else {
            Vec::new()
        };

        out.push(ProxyHealthBrief { id: p.id, global_status, restrictions });
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// per-platform tab 装配
// ─────────────────────────────────────────────────────────────────────────────

/// 装配某平台 tab 下所有代理的运行时画像。N 一般几十，复杂度 O(N)。
///
/// **批量化**（v6）：所有 per-row 信息在一开始用 5 条 grouped SQL 一次性
/// 读出（runtime snapshot map / 失败计数 group / accounts 绑定 group /
/// account username map / WorkerRegistry snapshot），后续循环纯 in-memory
/// join。原本 N×~5 的 query_row 缩到固定 ~5 条。
///
/// 数据来源：
/// - 元数据：`proxies` 表；
/// - 最后一次响应：`proxy_platform_runtime` 的 platform 切片（一条 SQL）；
/// - **绑定账号数**：`tasks.bound_proxy_ids × bound_account_ids` 笛卡尔展开后
///   按 (proxy, platform) 去重 count（v7：从「任务规划」维度反查，含义是
///   "有多少账号被任务规划要在该 IP 上跑"，而不是"账号扫码登录时绑了哪条代理"）；
/// - 运行账号数：传入的 in-memory `WorkerRegistry`；
/// - 状态：`derive_proxy_status_batch`（内部 4 条 grouped SQL）；
/// - 风险系数：5 min 内 (proxy, platform) 归责失败次数 × 10，封顶 100，
///   走同一份 grouped count 的 inline 计算。
pub fn list_proxies_runtime(
    db: &Database,
    registry: &Arc<WorkerRegistry>,
    platform: &str,
) -> Result<Vec<ProxyPlatformRow>, AppError> {
    let proxies = list_proxies(db)?;
    let conn = db.conn();

    let username_map = collect_account_usernames(&conn)?;
    let planned_map =
        crate::db::task_repo::group_planned_account_ids_by_proxy(&conn, platform)?;
    let runtime_map = proxy_runtime_repo::list_by_platform(&conn, platform)?;

    // 状态批量派生（max(global, platform)）。
    let proxy_keys: Vec<(String, ProxyType)> =
        proxies.iter().map(|p| (p.id.clone(), p.proxy_type)).collect();
    let status_map =
        risk::derive_proxy_status_batch(&conn, &proxy_keys, Some(platform))?;

    // 风险系数：与 derive_proxy_status_batch 的 plat_any 同一查询，但是后者
    // 已经在 risk 模块里 collapse 成 IpStatus，没法把原始 count 再拿回来。
    // 干脆这里直接再 group 一次（同一 SQL 已经在 risk 模块里调过了，rusqlite
    // 的 prepare 缓存命中，开销可忽略），换来代码简洁。
    let since = Utc::now() - risk::WINDOW;
    let fail_count_map =
        risk_event_repo::group_proxy_attributable_failures_by_platform_since(
            &conn, platform, since,
        )?;

    let registry_snapshot = registry.snapshot();
    drop(conn);

    let mut out = Vec::with_capacity(proxies.len());
    for p in proxies {
        let snap = runtime_map.get(&p.id).cloned().unwrap_or_default();

        let last_account_name = snap
            .last_account_id
            .as_deref()
            .and_then(|id| username_map.get(id).cloned());

        let derived = status_map.get(&p.id).copied().unwrap_or(IpStatus::Available);
        let status = if !p.global_probe_ok {
            IpStatus::Invalid
        } else {
            derived
        };
        let risk_score = (fail_count_map.get(&p.id).copied().unwrap_or(0) * 10).min(100);
        let bound = planned_map
            .get(&p.id)
            .map(|s| s.len() as i64)
            .unwrap_or(0);
        let running = registry_snapshot
            .get(&(p.id.clone(), platform.to_string()))
            .copied()
            .unwrap_or(0);

        out.push(ProxyPlatformRow {
            proxy: p,
            last_responded_at: snap.last_responded_at,
            last_account_id: snap.last_account_id,
            last_account_name,
            last_latency_ms: snap.last_latency_ms,
            last_status: snap.last_status,
            last_error_kind: snap.last_error_kind,
            last_http_status: snap.last_http_status,
            bound_account_count: bound,
            running_account_count: running,
            status,
            risk_score,
        });
    }
    Ok(out)
}

fn collect_account_usernames(
    conn: &rusqlite::Connection,
) -> Result<HashMap<String, String>, AppError> {
    let mut stmt = conn.prepare("SELECT id, username FROM accounts")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (id, name) = row?;
        out.insert(id, name);
    }
    Ok(out)
}

/// 真实的同步探测：**预热 + 计时** 两次请求，靠 reqwest 的 connection pool 让
/// 计时这一次走热连接，剥离 SOCKS5 / TLS / TCP 一次性握手成本，得到接近真实
/// 「请求 → 响应」的 RTT。
///
/// ### 关键不变量：必须把 body 读完，连接才能归还到 pool
/// reqwest 0.12 blocking 内部跑 async runtime，drop 一个还在 streaming 的
/// `Response` 会**取消 stream 并关闭底层 TCP**。早期实现 warmup 直接 drop
/// `resp`，导致 measure 这一次又重建 SOCKS5 + TLS，显示出来的延迟其实是冷连接。
/// 这里统一用 [`drain_body`] 把 body 喂给 sink，确保 keep-alive 真生效。
///
/// ### HEAD 优先 + 仅 405/501 fallback 到 GET
/// HEAD 不返回 body、最便宜；个别 CDN（baidu LB / 部分 CloudFront）对 HEAD 返
/// 回 405/501，那时 fallback 一次 GET。其他状态码（404/5xx）算真失败，不再
/// 「降级到预热耗时」掩盖故障。
///
/// 返回值：
/// - `>= 0`：本次（热连接）探测耗时（毫秒）；
/// - `-1`：构造 client 失败 / 预热失败 / 计时失败 / 非 2xx 响应。
fn probe_one(p: &ProxyIp, target_url: &str) -> i64 {
    // 同一个 Client 在多次请求间共享 connection pool；
    // 仅当上一次请求把 body 读完归还到 pool，下一次才能走 keep-alive。
    let client =
        match crate::weibo::http_client_with_proxy_and_timeout(Some(p), PROBE_TIMEOUT_EACH) {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "[proxy_service] probe build client failed for {}: {e}",
                    p.id
                );
                return PROBE_FAIL_LATENCY;
            }
        };

    // 预热：把 SOCKS5 / TLS / TCP 全部建好，**不计时**，结束后必须 drain body。
    if !warmup_with_keepalive(&client, target_url, &p.id) {
        return PROBE_FAIL_LATENCY;
    }

    // 计时：连接池里现在有一条热连接，HEAD 走 keep-alive 几乎零握手成本。
    // 拿到的 elapsed 才是「请求 → 响应」的真实 RTT。
    let measure_start = Instant::now();
    match send_head_then_get(&client, target_url) {
        Ok(mut resp) => {
            // 即使是 measure 也要把 body 读完——否则下一轮 refresh 时这条连接还是脏的。
            let _ = drain_body(&mut resp);
            measure_start.elapsed().as_millis() as i64
        }
        Err(MeasureFail::Status(code)) => {
            log::warn!(
                "[proxy_service] probe {} measure non-success status {} for {target_url}",
                p.id,
                code
            );
            PROBE_FAIL_LATENCY
        }
        Err(MeasureFail::Transport(e)) => {
            log::warn!(
                "[proxy_service] probe {} measure failed for {target_url}: {}",
                p.id,
                crate::weibo::fmt_reqwest_error(&e)
            );
            PROBE_FAIL_LATENCY
        }
    }
}

/// 预热：发一次 HEAD（必要时 fallback GET），成功就**完整读 body 后**归还到 pool。
fn warmup_with_keepalive(
    client: &reqwest::blocking::Client,
    target_url: &str,
    proxy_id: &str,
) -> bool {
    match send_head_then_get(client, target_url) {
        Ok(mut resp) => {
            // 关键：drain body 后连接才会归还 pool；不读就 drop = TCP 关闭 = 下一次 measure 又冷。
            if let Err(e) = drain_body(&mut resp) {
                log::warn!(
                    "[proxy_service] probe {proxy_id} warmup drain body failed: {}",
                    crate::weibo::fmt_reqwest_error(&e)
                );
                // body 读不全也认为预热失败：连接已脏，没法保证 measure 走热连。
                return false;
            }
            true
        }
        Err(MeasureFail::Status(code)) => {
            log::warn!(
                "[proxy_service] probe {proxy_id} warmup non-success status {code} for {target_url}"
            );
            false
        }
        Err(MeasureFail::Transport(e)) => {
            log::warn!(
                "[proxy_service] probe {proxy_id} warmup failed for {target_url}: {}",
                crate::weibo::fmt_reqwest_error(&e)
            );
            false
        }
    }
}

/// HEAD 优先；HEAD 任意非 2xx（包括 transport Err）都退一次 GET。
///
/// 之前只对 405 / 501 fallback，但 `cloudflare.com/cdn-cgi/trace` 等端点经 SOCKS5
/// 出来时常返回 403 / 421 / 5xx，HEAD 就被「真实失败」误杀，国外探针整片显示失败。
/// 现在按「HEAD 失败 → GET 兜底」语义，跟 measure 一致：GET 还失败才算真死。
/// HEAD 自带 body 一般为空，drop 不影响连接池复用；GET 那一支会在调用方 drain。
fn send_head_then_get(
    client: &reqwest::blocking::Client,
    target_url: &str,
) -> Result<reqwest::blocking::Response, MeasureFail> {
    match client.head(target_url).send() {
        Ok(resp) if resp.status().is_success() => Ok(resp),
        Ok(resp) => {
            let head_code = resp.status();
            // 关闭这条 HEAD 响应后，GET 复用同一 client 的连接池。
            drop(resp);
            log::debug!(
                "[proxy_service] HEAD non-2xx ({head_code}) for {target_url}, fallback to GET"
            );
            match client.get(target_url).send() {
                Ok(r2) if r2.status().is_success() => Ok(r2),
                Ok(r2) => Err(MeasureFail::Status(r2.status())),
                Err(e) => Err(MeasureFail::Transport(e)),
            }
        }
        Err(head_err) => {
            // HEAD 网络错误也兜一下 GET；个别中间盒会丢 HEAD 包但放 GET。
            log::debug!(
                "[proxy_service] HEAD transport err for {target_url} ({}), fallback to GET",
                crate::weibo::fmt_reqwest_error(&head_err)
            );
            match client.get(target_url).send() {
                Ok(r2) if r2.status().is_success() => Ok(r2),
                Ok(r2) => Err(MeasureFail::Status(r2.status())),
                // GET 也失败时回报 GET 这一边的错（更具操作性，HEAD 错可由 debug 日志查）。
                Err(e) => Err(MeasureFail::Transport(e)),
            }
        }
    }
}

/// 把响应 body 完整读完（写到 /dev/null）。reqwest blocking `Response` 实现了
/// `std::io::Read`，配 `io::sink()` 是最便宜的 drain 方式。
fn drain_body(resp: &mut reqwest::blocking::Response) -> Result<(), reqwest::Error> {
    // copy 失败用 reqwest::Error 包一层不直观；这里把 io::Error 转成 transport 失败语义即可。
    // Response::copy_to 内部就是 io::copy(self, &mut sink)，但它返回 reqwest::Error，统一用它。
    resp.copy_to(&mut std::io::sink()).map(|_| ())
}

enum MeasureFail {
    Status(reqwest::StatusCode),
    Transport(reqwest::Error),
}
