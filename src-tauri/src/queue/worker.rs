//! 持久化队列调度：每个任务按「账号 × 代理」笛卡尔积起一组并发 worker。
//!
//! - 每个 worker 是固定 (account, proxy) 对，独立持有 SQLite 连接、reqwest::Client。
//! - 通过 [`crate::db::crawl_request_repo::claim_one`] 原子领取 pending 请求。
//! - 限流支持两档：
//!   - `PerWorker`：每 worker 自己 sleep `60_000/rate_limit` ms。
//!   - `PerAccount`：同账号下的多个 worker 共享一个 [`Mutex<Instant>`] 令牌桶。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::db::{
    account_repo, app_event_repo, crawl_request_repo, proxy_repo, risk_event_repo, task_repo,
};
use crate::error::AppError;
use crate::model::account::{Account, AccountStatus};
use crate::model::crawl_request::CrawlRequestType;
use crate::model::proxy::{IpStatus, ProxyIp};
use crate::model::task::{CrawlStrategy, CrawlTask, RateLimitScope, TaskStatus};
use crate::queue::message::{CrawlCommand, CrawlProgressEvent};
use crate::queue::registry::Guard as RegistryGuard;
use crate::queue::risk::{
    self, classify, ErrorKind, RiskCounters, WORKER_CB_BACKOFF_AFTER, WORKER_CB_HARD_LIMIT,
};
use crate::service::settings_service;
use crate::queue::runtime_buffer::{OwnedSample, RuntimeBuffer};
use crate::weibo;
use crate::AppState;

/// 每多少条 done 触发一次 [`risk::evaluate`]，避免每条都查窗口。
/// 失败路径会**额外**触发一次评估，所以这里只是稳态吞吐时的兜底节奏。
const RISK_EVAL_EVERY_N_SUCCESS: i64 = 10;

fn strategy_label(s: CrawlStrategy) -> &'static str {
    match s {
        CrawlStrategy::RoundRobin => "轮询",
        CrawlStrategy::Random => "随机",
    }
}

fn scope_label(s: RateLimitScope) -> &'static str {
    match s {
        RateLimitScope::PerWorker => "per_worker",
        RateLimitScope::PerAccount => "per_account",
    }
}

/// 根据触发熔断的最后一次错误类型 + 当前是否挂代理，给出可执行的人工建议。
/// 出现在 `risk` / `progress` 事件文本里，前端横幅 / 日志面板都会显示。
fn cb_hint_for(kind: ErrorKind, has_proxy: bool) -> &'static str {
    match kind {
        ErrorKind::HttpStatus(414) => {
            if has_proxy {
                "疑似当前代理 IP 被微博限流，建议更换代理 / 等待 5-30 min 后再试。"
            } else {
                "疑似当前出口 IP 被微博限流（同 IP 上所有账号都会同时报 414）；建议挂代理 IP 跑此任务，或暂停 5-30 min 让 WAF 冷却。"
            }
        }
        ErrorKind::HttpStatus(429) => "请求太频繁被限流；请降低任务的限流阈值或换代理。",
        ErrorKind::HttpStatus(c) if (500..=599).contains(&c) => {
            "上游服务端错误，建议稍后重试或更换代理。"
        }
        ErrorKind::Network => "网络层故障（连接 / DNS / 超时），建议检查代理可用性。",
        ErrorKind::LoginRequired => "账号登录失效，请在「账号管理」中重新扫码登录。",
        ErrorKind::BusinessReject => "业务侧拒绝（接口返回 errno != 0），请检查参数 / 账号权限。",
        _ => "请检查请求模板 / Cookie / 代理是否正常。",
    }
}

/// Background worker: receives `CrawlCommand` from the channel, then runs the
/// persistent-queue scheduler for each task in a blocking thread.
pub async fn run(mut rx: mpsc::Receiver<CrawlCommand>, app_handle: AppHandle) {
    while let Some(cmd) = rx.recv().await {
        let app = app_handle.clone();
        let _ = tokio::task::spawn_blocking(move || {
            run_scheduler(&app, &cmd);
        });
    }
}

/// 任务调度入口：构建 worker 池并并发执行，主线程 join 后落最终状态。
fn run_scheduler(app: &AppHandle, cmd: &CrawlCommand) {
    let state = app.state::<AppState>();
    let task_id = &cmd.task_id;

    let conn = match state.db.open_crawl_connection() {
        Ok(c) => c,
        Err(e) => {
            emit(app, task_id, "error", format!("无法打开数据库连接: {e}"));
            return;
        }
    };

    let task = match task_repo::get_by_id(&conn, task_id) {
        Ok(t) => t,
        Err(e) => {
            emit(app, task_id, "error", format!("读取任务失败: {e}"));
            return;
        }
    };

    let account_ids = task.bound_account_ids.clone().unwrap_or_default();
    if account_ids.is_empty() {
        emit(app, task_id, "error", "任务未绑定账号，无法调度");
        let _ = task_repo::update_status(&conn, task_id, TaskStatus::Error);
        return;
    }
    let proxy_ids = task.bound_proxy_ids.clone().unwrap_or_default();

    // 构建 worker 描述：(account, optional proxy)。proxy 为空时退化为 N×1。
    let workers = match build_worker_specs(&conn, &account_ids, &proxy_ids, task.strategy) {
        Ok(v) => v,
        Err(e) => {
            emit(app, task_id, "error", format!("构建 worker 失败: {e}"));
            let _ = task_repo::update_status(&conn, task_id, TaskStatus::Error);
            return;
        }
    };

    if workers.is_empty() {
        emit(app, task_id, "error", "无可用 (账号, 代理) 组合");
        let _ = task_repo::update_status(&conn, task_id, TaskStatus::Error);
        return;
    }

    let rate_limit = task.rate_limit.max(1);
    let delay = Duration::from_millis((60_000 / rate_limit) as u64);

    // PerAccount 限流：每账号一份 Mutex<Instant>，记录该账号最近一次请求时间。
    let account_gates: Arc<HashMap<String, Arc<Mutex<Instant>>>> = if matches!(
        task.rate_limit_scope,
        RateLimitScope::PerAccount
    ) {
        let mut map = HashMap::new();
        for id in account_ids.iter() {
            map.entry(id.clone())
                .or_insert_with(|| Arc::new(Mutex::new(Instant::now() - delay)));
        }
        Arc::new(map)
    } else {
        Arc::new(HashMap::new())
    };

    emit(
        app,
        task_id,
        "progress",
        format!(
            "开始调度「{}」（IP 派发：{}，限流：{}，并发 worker：{}）",
            task.name,
            strategy_label(task.strategy),
            scope_label(task.rate_limit_scope),
            workers.len()
        ),
    );

    drop(conn);

    let task_arc = Arc::new(task);
    // 任务级 stop：外部暂停 / 全局错误。所有 worker 都遵从。
    let task_stop = Arc::new(AtomicBool::new(false));
    // 账号级 stop：某账号被风控判 Error 时，**只**让同账号的 worker 退出，
    // 不影响其它账号继续跑。
    let account_stops: Arc<HashMap<String, Arc<AtomicBool>>> = {
        let mut map = HashMap::new();
        for id in account_ids.iter() {
            map.entry(id.clone())
                .or_insert_with(|| Arc::new(AtomicBool::new(false)));
        }
        Arc::new(map)
    };
    let mut handles = Vec::with_capacity(workers.len());

    for spec in workers {
        let app_handle = app.clone();
        let task_clone = Arc::clone(&task_arc);
        let task_stop_clone = Arc::clone(&task_stop);
        let acc_stop = account_stops
            .get(&spec.account.id)
            .cloned()
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
        let gates = Arc::clone(&account_gates);
        let scope = task_arc.rate_limit_scope;
        let task_rate_limit = rate_limit;
        let worker_delay = delay;

        let handle = thread::spawn(move || {
            run_worker(
                app_handle,
                task_clone,
                spec,
                task_stop_clone,
                acc_stop,
                scope,
                gates,
                worker_delay,
                task_rate_limit,
            );
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }

    // 收尾：根据最终统计设置 task 状态。
    let conn = match state.db.open_crawl_connection() {
        Ok(c) => c,
        Err(_) => return,
    };
    let progress = crawl_request_repo::count_by_status(&conn, task_id).ok();
    let failed_n = progress.as_ref().map(|p| p.failed).unwrap_or(0);
    let done_n = progress.as_ref().map(|p| p.done).unwrap_or(0);
    let total_n = progress.as_ref().map(|p| p.total).unwrap_or(0);
    let pending_n = progress.as_ref().map(|p| p.pending).unwrap_or(0);

    // 暂停优先：只要还有 pending（说明是被外部暂停 / 主动停止）就保持 paused。
    let final_status = if pending_n > 0 {
        TaskStatus::Paused
    } else if failed_n > 0 && done_n == 0 {
        TaskStatus::Error
    } else if failed_n == 0 && done_n > 0 {
        TaskStatus::Completed
    } else {
        TaskStatus::Paused
    };
    let _ = task_repo::update_status(&conn, task_id, final_status);

    // 清理 24h 前的失败事件，避免表无限增长。
    let threshold = chrono::Utc::now() - chrono::Duration::hours(24);
    if let Err(e) = risk_event_repo::purge_older_than(&conn, threshold) {
        emit(app, task_id, "progress", format!("清理风控事件失败: {e}"));
    }

    let msg = format!("调度完成：成功 {done_n}/{total_n}，失败 {failed_n}");
    emit(app, task_id, "done", msg);
}

/// 单个 worker 的执行循环。
///
/// `task_stop`: 任务级停止（外部暂停 / 致命错误），所有 worker 均退出。
/// `account_stop`: 账号级停止（本账号被风控判 Error），只让同账号的 worker 退出。
fn run_worker(
    app: AppHandle,
    task: Arc<CrawlTask>,
    spec: WorkerSpec,
    task_stop: Arc<AtomicBool>,
    account_stop: Arc<AtomicBool>,
    scope: RateLimitScope,
    gates: Arc<HashMap<String, Arc<Mutex<Instant>>>>,
    delay: Duration,
    rate_limit: i64,
) {
    let task_id = task.id.clone();
    let label = worker_label(&spec);

    let conn = {
        let state = app.state::<AppState>();
        match state.db.open_crawl_connection() {
            Ok(c) => c,
            Err(e) => {
                emit(&app, &task_id, "error", format!("[{label}] 无法打开 DB 连接: {e}"));
                return;
            }
        }
    };

    let client = match weibo::http_client_with_proxy(spec.proxy.as_ref()) {
        Ok(c) => c,
        Err(e) => {
            emit(&app, &task_id, "error", format!("[{label}] HTTP 客户端构造失败: {e}"));
            return;
        }
    };

    let cookies_json = match spec
        .account
        .cookies
        .as_ref()
        .filter(|c| c.len() > 2)
    {
        Some(c) => c.clone(),
        None => {
            emit(
                &app,
                &task_id,
                "error",
                format!("[{label}] 账号无有效 Cookie，跳过该 worker"),
            );
            return;
        }
    };
    let stored = match weibo::weibo_cookies_from_json(&cookies_json) {
        Ok(s) => s,
        Err(e) => {
            emit(&app, &task_id, "error", format!("[{label}] 解析 Cookie 失败: {e}"));
            return;
        }
    };

    let cookie_header_len = stored.header.len();
    emit(
        &app,
        &task_id,
        "progress",
        format!(
            "[{label}] Cookie 白名单：保留 {} 项，丢弃 {} 项 (header={}b)",
            stored.kept, stored.dropped, cookie_header_len
        ),
    );

    let account_id = spec.account.id.clone();
    let proxy_id = spec.proxy.as_ref().map(|p| p.id.clone());
    // 注册到 in-memory worker 注册表：用于 IP 管理页「运行账号数量」实时统计。
    // RAII Guard 在 run_worker 退出（return / 正常结束）时自动反注册。
    let _registry_guard: Option<RegistryGuard> = proxy_id.as_deref().map(|pid| {
        let state = app.state::<AppState>();
        state
            .worker_registry
            .register(pid, task.platform.as_tag(), &account_id)
    });

    // 白名单裁剪后仍 > COOKIE_HARD_LIMIT 字节：通常意味着 SUB / SUBP / WBPSESS
    // 中某个值异常（多次登录残留 / 解析错位），继续跑大概率会持续 414。
    // 直接把账号置 Restricted、提示用户重新登录，并退出本 worker；其它账号
    // 不受影响。
    const COOKIE_HARD_LIMIT: usize = 6144;
    if cookie_header_len > COOKIE_HARD_LIMIT {
        let conn_for_status = {
            let state = app.state::<AppState>();
            state.db.open_crawl_connection().ok()
        };
        if let Some(c) = conn_for_status.as_ref() {
            if account_repo::update_risk_status(c, &account_id, AccountStatus::Restricted).is_ok() {
                app_event_repo::try_insert(
                    c,
                    "risk",
                    "risk_change",
                    "warn",
                    &format!(
                        "任务「{}」：账号 {} Cookie 体积过大（{}b），风控已置为受限",
                        task.name, spec.account.username, cookie_header_len,
                    ),
                    Some("account"),
                    Some(&account_id),
                    Some(&task_id),
                );
            }
        }
        emit(
            &app,
            &task_id,
            "risk",
            format!(
                "[{label}] Cookie 异常超大 ({}b > {}b)，疑似账号 Cookie 污染，已置 Restricted；请在「账号管理」中重新扫码登录。本 worker 退出，其它账号继续。",
                cookie_header_len, COOKIE_HARD_LIMIT
            ),
        );
        return;
    }
    // 取一次初始状态后，本 worker 内自行追踪迁移，避免每轮查 accounts/proxies。
    // v3 起代理档不再持久化（详见 risk::derive_proxy_status），这里只是 worker 内
    // 的"上一态记忆"，用于判断是否需要 emit 状态事件 + Invalid 退出。首轮 None
    // → 第一次 evaluate 一定会给出 Some(派生档)；若派生为 Available（出口本来就健康），
    // 下游 emit 处会把这条 baseline 降级到 progress 通道，避免误报"风控提示"。
    let mut current_account_status = spec.account.risk_status;
    let mut current_proxy_status: Option<IpStatus> = None;

    let mut counters = RiskCounters::default();
    let mut success_since_eval: i64 = 0;

    loop {
        // 任务级 / 账号级停止：任一触发即退出。
        if task_stop.load(Ordering::Relaxed) || account_stop.load(Ordering::Relaxed) {
            return;
        }
        if let Ok(current) = task_repo::get_by_id(&conn, &task_id) {
            if current.status == TaskStatus::Paused {
                task_stop.store(true, Ordering::Relaxed);
                emit(&app, &task_id, "progress", format!("[{label}] 任务已暂停，退出"));
                return;
            }
        }

        // 限流：进入下一条请求前先持有令牌。
        acquire_rate_limit(scope, &gates, &account_id, delay);

        let req = match crawl_request_repo::claim_one(
            &conn,
            &task_id,
            &account_id,
            proxy_id.as_deref(),
        ) {
            Ok(Some(r)) => r,
            Ok(None) => {
                // 队列暂时为空：若仍有 running 行（其他 worker 在跑），等等再试。
                let progress = crawl_request_repo::count_by_status(&conn, &task_id).ok();
                let has_running = progress.as_ref().map(|p| p.running > 0).unwrap_or(false);
                if has_running {
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                return;
            }
            Err(e) => {
                emit(&app, &task_id, "error", format!("[{label}] 领取请求失败: {e}"));
                thread::sleep(Duration::from_millis(500));
                continue;
            }
        };

        let type_label = match req.request_type {
            CrawlRequestType::ListPage => "列表页",
            CrawlRequestType::Body => "正文",
            CrawlRequestType::CommentL1 => "一级评论",
            CrawlRequestType::CommentL2 => "二级评论",
        };

        emit(
            &app,
            &task_id,
            "progress",
            format!("[{label}] [{type_label}] 执行 {}…", short_id(&req.id)),
        );

        let req_start = Instant::now();
        let outcome = weibo::execute_single_request(&conn, &task, &client, &stored, &req, rate_limit);
        let elapsed_ms = req_start.elapsed().as_millis() as i64;
        let mut should_eval = false;
        // 把这次请求结果 push 到 runtime_buffer（仅当挂了代理）。
        // worker 不再直写 sqlite——同 (proxy, platform) 高频请求会在内存里折叠，
        // 1s 后由 flusher 批量 upsert。详见 [`queue::runtime_buffer`]。
        if let Some(pid) = proxy_id.as_deref() {
            let state = app.state::<AppState>();
            push_runtime_sample(
                &state.runtime_buffer,
                pid,
                task.platform.as_tag(),
                &account_id,
                elapsed_ms,
                outcome.as_ref().err(),
            );
        }
        match &outcome {
            Ok(result) => {
                let _ = crawl_request_repo::mark_done(
                    &conn,
                    &req.id,
                    result.response_summary.as_deref(),
                    result.response_data.as_deref(),
                );
                let _ = account_repo::touch_last_active(&conn, &account_id);

                if !result.derived_requests.is_empty() {
                    let _ = crawl_request_repo::insert_batch(&conn, &result.derived_requests);
                }

                counters.on_success();
                success_since_eval += 1;
                if success_since_eval >= RISK_EVAL_EVERY_N_SUCCESS {
                    should_eval = true;
                }

                let progress = crawl_request_repo::count_by_status(&conn, &task_id).ok();
                let progress_str = progress
                    .map(|p| format!("({}/{})", p.done, p.total))
                    .unwrap_or_default();
                emit(
                    &app,
                    &task_id,
                    "progress",
                    format!(
                        "[{label}] [{type_label}] 完成，写入 {} 条 {progress_str}",
                        result.records_inserted
                    ),
                );
            }
            Err(e) => {
                let _ = crawl_request_repo::mark_failed(
                    &conn,
                    &req.id,
                    &e.to_string(),
                    req.retry_count + 1,
                );
                counters.on_failure();
                if let Err(rec_err) = risk::record(
                    &conn,
                    Some(&task_id),
                    Some(&req.id),
                    &account_id,
                    proxy_id.as_deref(),
                    Some(task.platform.as_tag()),
                    e,
                ) {
                    emit(
                        &app,
                        &task_id,
                        "progress",
                        format!("[{label}] 记录风控事件失败: {rec_err}"),
                    );
                }
                should_eval = true;
                let last_kind = classify(e);
                emit(
                    &app,
                    &task_id,
                    "progress",
                    format!("[{label}] [{type_label}] 失败: {}", e.summary()),
                );

                // Worker 级熔断：与风控事件表无关，纯本地连续失败计数。
                // 用于扛住 414/400 这种「请求构造类」错误——它们不归属账号或代理，
                // 但所有 worker 会同步爆发，没有熔断会刷屏到天荒地老。
                if counters.consecutive_failure >= WORKER_CB_HARD_LIMIT {
                    emit(
                        &app,
                        &task_id,
                        "risk",
                        format!(
                            "[{label}] 连续失败 {} 条，停止本 worker。{}",
                            counters.consecutive_failure,
                            cb_hint_for(last_kind, proxy_id.is_some())
                        ),
                    );
                    return;
                }
                if counters.consecutive_failure >= WORKER_CB_BACKOFF_AFTER {
                    let backoff_ms = {
                        let state = app.state::<AppState>();
                        settings_service::worker_backoff_ms_for_platform(
                            &state.db,
                            task.platform.as_tag(),
                        )
                    };
                    emit(
                        &app,
                        &task_id,
                        "progress",
                        format!(
                            "[{label}] 连续失败 {} 条，退避 {} 秒后重试。{}",
                            counters.consecutive_failure,
                            backoff_ms / 1000,
                            cb_hint_for(last_kind, proxy_id.is_some())
                        ),
                    );
                    // 退避期内仍然检查 stop 信号，避免暂停任务时被长时间卡住。
                    let step = Duration::from_millis(500);
                    let mut waited = Duration::ZERO;
                    let total = Duration::from_millis(backoff_ms);
                    while waited < total {
                        if task_stop.load(Ordering::Relaxed)
                            || account_stop.load(Ordering::Relaxed)
                        {
                            return;
                        }
                        thread::sleep(step);
                        waited += step;
                    }
                    // 退避结束后清零计数，给后续请求一次重试机会；
                    // 若仍然连续失败，会再次走到熔断分支。
                    counters.consecutive_failure = 0;
                }
            }
        }

        if should_eval {
            success_since_eval = 0;
            match risk::evaluate(
                &conn,
                &account_id,
                current_account_status,
                proxy_id.as_deref(),
                current_proxy_status,
                spec.proxy.as_ref().map(|p| p.proxy_type),
                Some(task.platform.as_tag()),
                &counters,
            ) {
                Ok(verdict) => {
                    if let Some(new_acc) = verdict.account {
                        if new_acc != current_account_status {
                            if let Err(e) = account_repo::update_risk_status(&conn, &account_id, new_acc) {
                                emit(&app, &task_id, "progress", format!("[{label}] 写账号风控状态失败: {e}"));
                            } else {
                                let prev_acc = current_account_status;
                                current_account_status = new_acc;
                                app_event_repo::try_insert(
                                    &conn,
                                    "risk",
                                    "risk_change",
                                    "warn",
                                    &format!(
                                        "任务「{}」：账号 {} 风控 {} → {}",
                                        task.name,
                                        spec.account.username,
                                        account_status_label(prev_acc),
                                        account_status_label(new_acc),
                                    ),
                                    Some("account"),
                                    Some(&account_id),
                                    Some(&task_id),
                                );
                                emit(
                                    &app,
                                    &task_id,
                                    "risk",
                                    format!("[{label}] 账号风控 → {}", account_status_label(new_acc)),
                                );
                                if matches!(new_acc, AccountStatus::Error) {
                                    // 账号 Error：同账号下其它 (account, proxy) worker 也需立即退出，
                                    // 但其它账号继续跑（task_stop 不动）。
                                    account_stop.store(true, Ordering::Relaxed);
                                    return;
                                }
                            }
                        }
                    }
                    if let (Some(new_proxy), Some(_pid)) = (verdict.proxy, proxy_id.as_deref()) {
                        if Some(new_proxy) != current_proxy_status {
                            // v3：不再写库，只更新 worker 内存上一态 + emit + Invalid 退出。
                            let prev_proxy_status = current_proxy_status;
                            current_proxy_status = Some(new_proxy);
                            // 首轮 baseline（prev = None）派生为 Available 时不算"风控告警"，
                            // 走 progress 通道只记日志，避免每个 worker 启动都弹一次
                            // "风控提示"toast 干扰用户。真正的恶化（Available→Restricted/
                            // Invalid）和回落（Restricted→Available 等）依旧走 risk 通道。
                            let is_initial_available_baseline =
                                prev_proxy_status.is_none()
                                    && matches!(new_proxy, IpStatus::Available);
                            let channel = if is_initial_available_baseline {
                                "progress"
                            } else {
                                "risk"
                            };
                            emit(
                                &app,
                                &task_id,
                                channel,
                                format!("[{label}] 代理状态 → {}", proxy_status_label(new_proxy)),
                            );
                            if channel == "risk" {
                                let prev_l = prev_proxy_status
                                    .map(proxy_status_label)
                                    .unwrap_or("—");
                                let px = proxy_id.as_deref().unwrap_or("?");
                                app_event_repo::try_insert(
                                    &conn,
                                    "risk",
                                    "risk_change",
                                    "warn",
                                    &format!(
                                        "任务「{}」：代理 {} 状态 {} → {}",
                                        task.name,
                                        px,
                                        prev_l,
                                        proxy_status_label(new_proxy),
                                    ),
                                    Some("proxy"),
                                    proxy_id.as_deref(),
                                    Some(&task_id),
                                );
                            }
                            if matches!(new_proxy, IpStatus::Invalid) {
                                // 代理 Invalid：仅退当前 worker，其它代理不受影响。
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    emit(&app, &task_id, "progress", format!("[{label}] 风控评估失败: {e}"));
                }
            }
        }

        // PerWorker 模式下，请求结束后再用 `delay` 节流到下一轮。
        if matches!(scope, RateLimitScope::PerWorker) {
            weibo::rate_limit_sleep(delay, req_start.elapsed());
        }
    }
}

/// 把单次请求结果 push 到 [`RuntimeBuffer`]。
/// 失败时会从 [`AppError`] 中抽取 `error_kind` / `http_status`，便于前端 IP 管理页
/// 显示「最后一次响应状态：成功 / 失败(414) / 失败(network)」之类的诊断标签。
///
/// 注意：这里只 push 到内存；批量 upsert 由 `RuntimeBuffer::start_flusher` 负责。
fn push_runtime_sample(
    buffer: &RuntimeBuffer,
    proxy_id: &str,
    platform: &str,
    account_id: &str,
    latency_ms: i64,
    err: Option<&AppError>,
) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let (status, error_kind, http_status) = match err {
        None => ("success".to_string(), None, None),
        Some(err) => {
            let kind = classify(err);
            let http = match err {
                AppError::HttpStatus { code, .. } => Some(*code as i64),
                _ => None,
            };
            ("failure".to_string(), Some(kind.as_tag().to_string()), http)
        }
    };
    buffer.push(
        proxy_id,
        platform,
        OwnedSample {
            account_id: account_id.to_string(),
            latency_ms,
            status,
            error_kind,
            http_status,
            responded_at: now,
        },
    );
}

fn account_status_label(s: AccountStatus) -> &'static str {
    match s {
        AccountStatus::Normal => "正常",
        AccountStatus::Restricted => "受限",
        AccountStatus::Error => "异常",
    }
}

fn proxy_status_label(s: IpStatus) -> &'static str {
    match s {
        IpStatus::Available => "可用",
        IpStatus::Restricted => "受限",
        IpStatus::Invalid => "失效",
    }
}

/// 限流入口：根据 scope 决定 sleep 多久。
///
/// PerAccount 模式下，同账号的多个 worker 共享同一个 `Instant` 令牌：
/// 谁先抢到锁谁就更新「上次发请求」的时间，并按 delay 排队等待。
fn acquire_rate_limit(
    scope: RateLimitScope,
    gates: &HashMap<String, Arc<Mutex<Instant>>>,
    account_id: &str,
    delay: Duration,
) {
    if !matches!(scope, RateLimitScope::PerAccount) {
        return;
    }
    let Some(gate) = gates.get(account_id).cloned() else {
        return;
    };
    let mut last = gate.lock().expect("rate limit mutex poisoned");
    let now = Instant::now();
    let elapsed = now.duration_since(*last);
    if elapsed < delay {
        let wait = delay - elapsed;
        // 持锁 sleep：等待期间其它 worker 阻塞在 gate.lock()，正好排队。
        thread::sleep(wait);
    }
    *last = Instant::now();
}

#[derive(Debug, Clone)]
struct WorkerSpec {
    account: Account,
    proxy: Option<ProxyIp>,
}

fn worker_label(spec: &WorkerSpec) -> String {
    let acc = &spec.account.username;
    match &spec.proxy {
        // Direct 行用更直观的「直连」字样，避免 "@ 本机直连" 之类绕口的渲染。
        Some(p) if matches!(p.proxy_type, crate::model::proxy::ProxyType::Direct) => {
            format!("{acc} @ direct")
        }
        Some(p) => format!("{acc} @ {}", redact_proxy_address(&p.address)),
        None => format!("{acc} @ no-proxy"),
    }
}

/// 把 `scheme://user:password@host:port` 形式中的 userinfo 改成 `***`，
/// 避免代理账号密码明文出现在 progress / risk 日志里被截屏 / 上传问题反馈。
fn redact_proxy_address(addr: &str) -> String {
    let Some((scheme, rest)) = addr.split_once("://") else {
        return addr.to_string();
    };
    match rest.split_once('@') {
        Some((_userinfo, host)) => format!("{scheme}://***@{host}"),
        None => addr.to_string(),
    }
}

#[cfg(test)]
mod redact_tests {
    use super::redact_proxy_address;

    #[test]
    fn redacts_userinfo_in_url() {
        assert_eq!(
            redact_proxy_address("socks5://crawler:Yi656869!@8.129.97.181:1080"),
            "socks5://***@8.129.97.181:1080"
        );
    }

    #[test]
    fn keeps_address_without_userinfo() {
        assert_eq!(
            redact_proxy_address("http://1.2.3.4:8080"),
            "http://1.2.3.4:8080"
        );
        assert_eq!(redact_proxy_address("1.2.3.4:8080"), "1.2.3.4:8080");
    }
}

/// 加载账号 + 代理实体，按 `CrawlStrategy` 决定 worker 顺序：
/// - `RoundRobin`：账号外层 × 代理内层 顺序展开。
/// - `Random`：展开后用系统时间纳秒做种子洗牌。
///
/// `proxy_ids` 为空时自动回落到「本机直连」系统行（[`crate::model::proxy::LOCAL_DIRECT_PROXY_ID`]），
/// 让直连路径也有一个真实的 `proxy_id` 可记录失败事件 / 进风控统计。
fn build_worker_specs(
    conn: &Connection,
    account_ids: &[String],
    proxy_ids: &[String],
    strategy: CrawlStrategy,
) -> Result<Vec<WorkerSpec>, AppError> {
    let mut accounts: Vec<Account> = Vec::with_capacity(account_ids.len());
    for id in account_ids {
        accounts.push(crate::db::account_repo::get_by_id(conn, id)?);
    }

    let effective_proxy_ids: Vec<String> = if proxy_ids.is_empty() {
        vec![crate::model::proxy::LOCAL_DIRECT_PROXY_ID.to_string()]
    } else {
        proxy_ids.to_vec()
    };
    let mut proxies: Vec<ProxyIp> = Vec::with_capacity(effective_proxy_ids.len());
    for id in &effective_proxy_ids {
        proxies.push(proxy_repo::get_by_id(conn, id)?);
    }

    let mut workers: Vec<WorkerSpec> =
        Vec::with_capacity(accounts.len().max(1) * proxies.len().max(1));
    for acc in &accounts {
        for px in &proxies {
            workers.push(WorkerSpec {
                account: acc.clone(),
                proxy: Some(px.clone()),
            });
        }
    }

    if matches!(strategy, CrawlStrategy::Random) {
        shuffle_in_place(&mut workers);
    }
    Ok(workers)
}

/// 简易 Fisher–Yates，不引入额外 RNG 依赖。
fn shuffle_in_place<T>(v: &mut [T]) {
    let len = v.len();
    if len < 2 {
        return;
    }
    let mut state = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xDEAD_BEEF);
    for i in (1..len).rev() {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state % ((i + 1) as u64)) as usize;
        v.swap(i, j);
    }
}

fn emit(app: &AppHandle, task_id: &str, status: &str, message: impl Into<String>) {
    let _ = app.emit(
        "crawl-progress",
        &CrawlProgressEvent {
            task_id: task_id.to_string(),
            status: status.to_string(),
            message: message.into(),
        },
    );
}

fn short_id(id: &str) -> &str {
    if id.len() > 8 { &id[..8] } else { id }
}
