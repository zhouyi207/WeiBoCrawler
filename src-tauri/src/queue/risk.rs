//! 账号 / 代理风控核心：错误归因 + 滑窗阈值判定。
//!
//! 流程：worker 在每条请求结束后调用：
//! - 成功：[`record_success`] 仅更新内存 `RiskCounters`（连续成功计数），用于回落判定；
//! - 失败：[`record`] 把 [`AppError`] 归因后写入 `account_failure_events` /
//!   `proxy_failure_events`；
//! - 周期性（每 10 条 / 每次失败）调 [`evaluate`]，按 5 min 滑窗给出 [`Verdict`]，
//!   告诉调用方账号 / 代理是否需要状态迁移。
//!
//! 阈值参见模块常量；目前不暴露给前端配置。

use std::collections::HashMap;
use std::time::Instant;

use chrono::{Duration, Utc};
use rusqlite::Connection;

use crate::db::risk_event_repo::{
    self, count_account_attributable_failures_since, count_account_failures_by_kind_since,
    count_proxy_attributable_failures_by_platform_since,
    count_proxy_failures_by_kind_and_platform_since, count_proxy_failures_by_kind_since,
    count_proxy_failures_by_status_range_and_platform_since, FailureEvent,
};
use crate::error::AppError;
use crate::model::account::AccountStatus;
use crate::model::proxy::{IpStatus, ProxyType};

/// 滑动窗口长度。
pub const WINDOW: Duration = Duration::minutes(5);

/// 账号窗口内 LoginRequired 次数 ≥ 该值 → `Error`。
const ACC_LOGIN_REQUIRED_TO_ERROR: i64 = 3;
/// 账号窗口内任意失败次数 ≥ 该值 → `Restricted`。
const ACC_ANY_FAIL_TO_RESTRICTED: i64 = 5;
/// 代理窗口内 Network 失败次数 ≥ 该值 → `Invalid`。
pub const PROXY_NETWORK_TO_INVALID: i64 = 10;
/// 代理窗口内 Network 失败次数 ≥ 该值 → `Restricted`。
pub const PROXY_NETWORK_TO_RESTRICTED: i64 = 3;
/// 代理窗口内 5xx 次数 ≥ 该值 → `Restricted`。
pub const PROXY_5XX_TO_RESTRICTED: i64 = 5;
/// 代理窗口内**任意**归责到自己的失败次数 ≥ 该值 → `Restricted`。
/// 用于覆盖那些没有被网络 / 5xx 单独阈值捕获、但仍属于 IP 维度的信号
/// （典型场景：HTTP 414，被 Weibo 用作 IP 限流码）。
pub const PROXY_ANY_FAIL_TO_RESTRICTED: i64 = 8;
/// `Restricted` 自动回落需要在内存中累计的连续成功条数。
const RECOVER_CONSECUTIVE_SUCCESS: i64 = 10;

/// **Worker 级**熔断：连续失败到该值 → 退避一段时间 + 打印 warning。
/// 与风控事件表无关，纯内存计数，用于扛住「请求构造类错误（414/400/...）」
/// 这种与账号 / 代理无关、又会被所有 worker 同步爆发的故障。
pub const WORKER_CB_BACKOFF_AFTER: i64 = 8;
/// 熔断退避时长（毫秒）。退避结束后重置计数继续尝试。
pub const WORKER_CB_BACKOFF_MS: u64 = 30_000;
/// 连续失败到该值 → 直接退出 worker，避免无限刷屏。
pub const WORKER_CB_HARD_LIMIT: i64 = 20;

/// 错误粒度，由 [`classify`] 从 [`AppError`] 推导。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Network,
    HttpStatus(u16),
    LoginRequired,
    BusinessReject,
    Other,
}

impl ErrorKind {
    /// 写库 / SQL 比较用的 tag（小写蛇形）。
    pub fn as_tag(&self) -> &'static str {
        match self {
            ErrorKind::Network => "network",
            ErrorKind::HttpStatus(_) => "http_status",
            ErrorKind::LoginRequired => "login_required",
            ErrorKind::BusinessReject => "business_reject",
            ErrorKind::Other => "other",
        }
    }
}

/// 该错误应记到账号、代理还是两者；`Neither` 表示与风控无关，跳过落库。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribution {
    Account,
    Proxy,
    Both,
    Neither,
}

/// `evaluate` 的输出。`account` / `proxy` 为 `Some` 时调用方需要把状态写回 DB
/// 并 emit 一次「risk」事件，否则维持现状。
#[derive(Debug, Default, Clone, Copy)]
pub struct Verdict {
    pub account: Option<AccountStatus>,
    pub proxy: Option<IpStatus>,
}

/// 单个 (account, proxy) worker 的风控内存计数。回落判定不查库，避免 hot path 写放大。
#[derive(Debug, Default)]
pub struct RiskCounters {
    pub consecutive_success: i64,
    pub consecutive_failure: i64,
    pub last_failure_at: Option<Instant>,
}

impl RiskCounters {
    pub fn on_success(&mut self) {
        self.consecutive_success = self.consecutive_success.saturating_add(1);
        self.consecutive_failure = 0;
    }

    pub fn on_failure(&mut self) {
        self.consecutive_success = 0;
        self.consecutive_failure = self.consecutive_failure.saturating_add(1);
        self.last_failure_at = Some(Instant::now());
    }
}

pub fn classify(err: &AppError) -> ErrorKind {
    match err {
        AppError::Network(_) => ErrorKind::Network,
        AppError::HttpStatus { code, .. } => ErrorKind::HttpStatus(*code),
        AppError::LoginRequired(_) => ErrorKind::LoginRequired,
        AppError::BusinessReject { .. } => ErrorKind::BusinessReject,
        // 其他类型（Db / NotFound / Internal / 旧 Http）不参与风控判定，
        // 统一 Other：调用方决定是否记账（[`attribute`] 返回 Neither 即跳过）。
        _ => ErrorKind::Other,
    }
}

/// 把错误归责到账号 / 代理 / 两者 / 无。规则与 plan 节 4 一致：
/// - 网络错误归代理（连接 / DNS / 超时）；
/// - 429 双归（限流既可能是代理也可能是账号）；
/// - 403 / 412 / 418 / 451 视为账号被风控；
/// - **414 归代理**：物理上不可能是 URI Too Long（实测请求只有 ~1.5KB），Weibo
///   边缘把它当 IP 维度限流码用。同 IP 上一旦出现 414，所有账号会同时报错，与
///   账号无关、与请求构造也无关。
/// - 5xx 归代理（上游 / 隧道侧故障）；
/// - LoginRequired / BusinessReject 归账号。
pub fn attribute(kind: ErrorKind) -> Attribution {
    match kind {
        ErrorKind::Network => Attribution::Proxy,
        ErrorKind::HttpStatus(429) => Attribution::Both,
        ErrorKind::HttpStatus(403)
        | ErrorKind::HttpStatus(412)
        | ErrorKind::HttpStatus(418)
        | ErrorKind::HttpStatus(451) => Attribution::Account,
        ErrorKind::HttpStatus(414) => Attribution::Proxy,
        ErrorKind::HttpStatus(c) if (500..=599).contains(&c) => Attribution::Proxy,
        ErrorKind::LoginRequired => Attribution::Account,
        ErrorKind::BusinessReject => Attribution::Account,
        _ => Attribution::Neither,
    }
}

/// 失败事件落库。
///
/// **可见性 vs 风控归责的解耦**（v3 起）：
/// - 早期实现按 [`Attribution`] 二选一写入，导致 Network / 414 / 5xx 等错误
///   只出现在代理日志里，账号视角看不到任何失败痕迹，给排障带来困难。
/// - 现在的策略：
///   1. 只要 `kind` 不是 `Other`（即业务上认为是"请求失败"），就尽量同时
///      写入 `account_failure_events` 和 `proxy_failure_events`，让两边日志
///      modal 都能看到这条事件；
///   2. **风控状态机**（[`evaluate`]）改用
///      `count_*_attributable_failures_since` 在查询侧按归责 kind 过滤，
///      因此账号档位不会被纯网络故障误升 Restricted、代理档位也不会被
///      LoginRequired 这类纯账号事件误判。
///
/// `proxy_id == None` 时（直连模式）跳过代理表写入；账号 id 始终非空。
///
/// `platform`：v4 / 方案 C，由 worker 层透传任务平台。落到代理日志，让
/// 「IP × 平台」维度的滑窗派生（`derive_proxy_status_for_platform`）能区分出
/// 代理是只对某一平台受限，还是出口本身有问题。
pub fn record(
    conn: &Connection,
    task_id: Option<&str>,
    request_id: Option<&str>,
    account_id: &str,
    proxy_id: Option<&str>,
    platform: Option<&str>,
    err: &AppError,
) -> Result<(), AppError> {
    let kind = classify(err);
    // Other = Db / NotFound / Internal 等系统错误，不是 crawl 请求失败，跳过。
    if matches!(attribute(kind), Attribution::Neither) {
        return Ok(());
    }
    let http_status = match kind {
        ErrorKind::HttpStatus(c) => Some(c as i64),
        _ => None,
    };
    let message = err.to_string();
    let evt = FailureEvent {
        task_id,
        request_id,
        error_kind: kind.as_tag(),
        http_status,
        message: Some(message.as_str()),
        platform,
    };

    // 账号日志：始终写。account_id 是必填，调用方在 worker 层保证。
    // 账号是单平台资源，不在 account_failure_events 上 scope platform。
    risk_event_repo::insert_account_failure(conn, account_id, &evt)?;
    // 代理日志：直连模式下没有代理 id，跳过；其余一律写入（含 platform）。
    if let Some(pid) = proxy_id {
        risk_event_repo::insert_proxy_failure(conn, pid, &evt)?;
    }
    Ok(())
}

/// 滑窗判定。
///
/// - 账号档：仍依赖 `current_account`（DB 持久状态）+ `counters`，保留 Error 不自动回落、
///   Restricted 需累计连续成功的语义。
/// - 代理档：v3 起 **不再持久化** `proxies.status`，纯函数从 `proxy_failure_events`
///   滑窗派生。`last_known_proxy_status` 仅作为「上一轮派生结果」的内存记忆，用于
///   `Verdict.proxy` 是否需要 emit「状态迁移」事件 / 触发 worker 退出的判断；
///   `None` 表示 worker 启动后还没派生过（首轮一定 emit）。
///
/// 输出 `Verdict.account` / `Verdict.proxy` 为：
/// - `Some(new_status)`：与上一轮不同，调用方 emit 「risk」事件 + 必要时退出 worker；
/// - `None`：维持现状。
/// `platform`：任务平台。v4 起代理状态按 (IP, platform) scope 派生，没有平台就
/// 退化成仅看全局 Invalid 档（不会写 Restricted 给 worker，避免误退出）。
pub fn evaluate(
    conn: &Connection,
    account_id: &str,
    current_account: AccountStatus,
    proxy_id: Option<&str>,
    last_known_proxy_status: Option<IpStatus>,
    proxy_type: Option<ProxyType>,
    platform: Option<&str>,
    counters: &RiskCounters,
) -> Result<Verdict, AppError> {
    let now = Utc::now();
    let since = now - WINDOW;
    let mut verdict = Verdict::default();

    let acc_login_fails =
        count_account_failures_by_kind_since(conn, account_id, ErrorKind::LoginRequired.as_tag(), since)?;
    // 仅统计与账号有关的归责 kind，过滤掉伴生写入的 network / 414 / 5xx，
    // 否则账号档位会被网络故障带累成 Restricted。
    let acc_any_fails = count_account_attributable_failures_since(conn, account_id, since)?;
    let acc_target = decide_account(current_account, acc_login_fails, acc_any_fails, counters);
    if acc_target != current_account {
        verdict.account = Some(acc_target);
    }

    if let Some(pid) = proxy_id {
        let is_direct = matches!(proxy_type, Some(ProxyType::Direct));
        // worker 视角看到的代理档位 = max(全局, 当前任务平台)。
        // - 全局 Invalid → worker 必须退出（出口本身就连不上）；
        // - 当前平台 Restricted → 仅本平台 worker 自缓，其他平台不受影响。
        let global = derive_proxy_global_status(conn, pid, since, is_direct)?;
        let derived = if matches!(global, IpStatus::Invalid) {
            IpStatus::Invalid
        } else if let Some(p) = platform {
            derive_proxy_platform_status(conn, pid, p, since)?
        } else {
            global
        };
        if Some(derived) != last_known_proxy_status {
            verdict.proxy = Some(derived);
        }
    }

    Ok(verdict)
}

/// 全局派生（与 platform 无关）：
/// - net_fails ≥ 10 → Invalid（出口连任何平台都连不上，cap 在直连 Restricted）；
/// - 其它情况 → Available。"是否对某个平台受限" 由 [`derive_proxy_platform_status`] 决定。
///
/// 给 `stats_service` 当 dashboard 聚合用：仅看全局，不展开 N 个平台。
pub fn derive_proxy_global_status(
    conn: &Connection,
    proxy_id: &str,
    since: chrono::DateTime<Utc>,
    is_direct: bool,
) -> Result<IpStatus, AppError> {
    let net_fails =
        count_proxy_failures_by_kind_since(conn, proxy_id, ErrorKind::Network.as_tag(), since)?;
    Ok(decide_proxy_global(net_fails, is_direct))
}

/// per-platform 派生：判断该 IP 在指定平台 scope 下是否 Restricted。
/// - net_fails_p ≥ 3 / s5xx_p ≥ 5 / any_fails_p ≥ 8 → Restricted；
/// - 否则 Available。
///
/// 注意：此函数**不会**返回 Invalid——Invalid 是全局判定，由
/// [`derive_proxy_global_status`] 单独算。
pub fn derive_proxy_platform_status(
    conn: &Connection,
    proxy_id: &str,
    platform: &str,
    since: chrono::DateTime<Utc>,
) -> Result<IpStatus, AppError> {
    let net_p = count_proxy_failures_by_kind_and_platform_since(
        conn,
        proxy_id,
        ErrorKind::Network.as_tag(),
        platform,
        since,
    )?;
    let s5xx_p = count_proxy_failures_by_status_range_and_platform_since(
        conn, proxy_id, 500, 599, platform, since,
    )?;
    let any_p = count_proxy_attributable_failures_by_platform_since(conn, proxy_id, platform, since)?;
    Ok(decide_proxy_for_platform(net_p, s5xx_p, any_p))
}

/// 给 `proxy_service::list_proxies_health` 用的便捷入口：默认 [`WINDOW`] 滑窗。
pub fn derive_proxy_global_status_now(
    conn: &Connection,
    proxy_id: &str,
    is_direct: bool,
) -> Result<IpStatus, AppError> {
    derive_proxy_global_status(conn, proxy_id, Utc::now() - WINDOW, is_direct)
}

pub fn derive_proxy_platform_status_now(
    conn: &Connection,
    proxy_id: &str,
    platform: &str,
) -> Result<IpStatus, AppError> {
    derive_proxy_platform_status(conn, proxy_id, platform, Utc::now() - WINDOW)
}

/// 批量派生代理状态，输入 `[(proxy_id, ProxyType)]`，输出 `proxy_id → IpStatus`。
///
/// 与 `evaluate` 同套阈值；区别在于：
/// - 仅做 (proxy, platform) 的「max(全局, platform)」派生，不读 in-memory counters；
/// - 单次扫表 group by 后内存判定，给 `proxy_service::list_proxies_runtime` /
///   `list_proxies_health` 用，消除 N×4 的 query_row。
///
/// `platform = None` 时只返回全局 status（仅 Available / Invalid）。
/// `platform = Some(p)` 时 = max(全局, 当前 p)，可能返回三档全部。
pub fn derive_proxy_status_batch(
    conn: &Connection,
    proxies: &[(String, ProxyType)],
    platform: Option<&str>,
) -> Result<HashMap<String, IpStatus>, AppError> {
    let since = Utc::now() - WINDOW;

    // 全局：仅看 net 失败数。
    let global_net = risk_event_repo::group_proxy_failures_by_kind_since(
        conn,
        ErrorKind::Network.as_tag(),
        since,
    )?;

    // platform scope 的三个 group。无 platform 则跳过 SQL。
    let (plat_net, plat_5xx, plat_any) = if let Some(p) = platform {
        (
            risk_event_repo::group_proxy_failures_by_kind_and_platform_since(
                conn,
                ErrorKind::Network.as_tag(),
                p,
                since,
            )?,
            risk_event_repo::group_proxy_failures_by_status_range_and_platform_since(
                conn, 500, 599, p, since,
            )?,
            risk_event_repo::group_proxy_attributable_failures_by_platform_since(conn, p, since)?,
        )
    } else {
        (HashMap::new(), HashMap::new(), HashMap::new())
    };

    let mut out = HashMap::with_capacity(proxies.len());
    for (pid, ptype) in proxies {
        let is_direct = matches!(ptype, ProxyType::Direct);
        let g_net = global_net.get(pid).copied().unwrap_or(0);
        let global = decide_proxy_global(g_net, is_direct);
        let merged = if matches!(global, IpStatus::Invalid) || platform.is_none() {
            global
        } else {
            let np = plat_net.get(pid).copied().unwrap_or(0);
            let s5 = plat_5xx.get(pid).copied().unwrap_or(0);
            let any = plat_any.get(pid).copied().unwrap_or(0);
            let plat = decide_proxy_for_platform(np, s5, any);
            // max(global, platform)：global 这里只可能是 Available（Invalid 已上面 short-circuit）。
            if matches!(plat, IpStatus::Restricted) { IpStatus::Restricted } else { global }
        };
        out.insert(pid.clone(), merged);
    }
    Ok(out)
}

fn decide_account(
    current: AccountStatus,
    login_fails: i64,
    any_fails: i64,
    counters: &RiskCounters,
) -> AccountStatus {
    if login_fails >= ACC_LOGIN_REQUIRED_TO_ERROR {
        return AccountStatus::Error;
    }
    if any_fails >= ACC_ANY_FAIL_TO_RESTRICTED {
        if matches!(current, AccountStatus::Error) {
            // Error 不自动回落，需重新登录
            return AccountStatus::Error;
        }
        return AccountStatus::Restricted;
    }
    match current {
        AccountStatus::Error => AccountStatus::Error,
        AccountStatus::Restricted => {
            if any_fails == 0 && counters.consecutive_success >= RECOVER_CONSECUTIVE_SUCCESS {
                AccountStatus::Normal
            } else {
                AccountStatus::Restricted
            }
        }
        AccountStatus::Normal => AccountStatus::Normal,
    }
}

/// 纯函数：全局档判定。仅看 net_fails，不再考虑 s5xx / any_fails——后者按 platform scope。
/// Direct 永不被打成 Invalid（标失效只会让本机直连任务长期卡住）。
fn decide_proxy_global(net_fails: i64, is_direct: bool) -> IpStatus {
    if !is_direct && net_fails >= PROXY_NETWORK_TO_INVALID {
        return IpStatus::Invalid;
    }
    IpStatus::Available
}

/// 纯函数：(IP, platform) scope 派生。返回值仅 Restricted / Available；
/// Invalid 由 [`decide_proxy_global`] 单独决定。
fn decide_proxy_for_platform(net_p: i64, s5xx_p: i64, any_p: i64) -> IpStatus {
    if net_p >= PROXY_NETWORK_TO_RESTRICTED
        || s5xx_p >= PROXY_5XX_TO_RESTRICTED
        || any_p >= PROXY_ANY_FAIL_TO_RESTRICTED
    {
        return IpStatus::Restricted;
    }
    IpStatus::Available
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_required_three_times_marks_account_error() {
        let c = RiskCounters::default();
        assert_eq!(decide_account(AccountStatus::Normal, 3, 3, &c), AccountStatus::Error);
        assert_eq!(decide_account(AccountStatus::Restricted, 3, 4, &c), AccountStatus::Error);
    }

    #[test]
    fn five_any_failures_marks_account_restricted() {
        let c = RiskCounters::default();
        assert_eq!(decide_account(AccountStatus::Normal, 0, 5, &c), AccountStatus::Restricted);
    }

    #[test]
    fn restricted_recovers_after_streak() {
        let c = RiskCounters { consecutive_success: 10, ..Default::default() };
        assert_eq!(decide_account(AccountStatus::Restricted, 0, 0, &c), AccountStatus::Normal);
    }

    #[test]
    fn proxy_global_invalid_threshold() {
        // 全局档：≥10 net_fails → Invalid（仅 net_fails 决定）。
        assert_eq!(decide_proxy_global(10, false), IpStatus::Invalid);
        assert_eq!(decide_proxy_global(9, false), IpStatus::Available);
    }

    #[test]
    fn direct_proxy_never_invalid() {
        // Direct 即便 100 次 network 失败也不会被打成 Invalid，
        // 否则 worker 会自杀，用户却无法替换本机出口。
        assert_eq!(decide_proxy_global(100, true), IpStatus::Available);
    }

    #[test]
    fn proxy_per_platform_thresholds() {
        // platform scope 仅产出 Restricted / Available，不会越级到 Invalid。
        assert_eq!(decide_proxy_for_platform(3, 0, 3), IpStatus::Restricted);
        assert_eq!(decide_proxy_for_platform(0, 5, 5), IpStatus::Restricted);
        assert_eq!(decide_proxy_for_platform(0, 0, 8), IpStatus::Restricted);
        assert_eq!(decide_proxy_for_platform(0, 0, 7), IpStatus::Available);
    }

    #[test]
    fn proxy_recovers_when_window_clears() {
        assert_eq!(decide_proxy_global(0, false), IpStatus::Available);
        assert_eq!(decide_proxy_for_platform(0, 0, 0), IpStatus::Available);
    }

    #[test]
    fn http_414_attributed_to_proxy() {
        assert_eq!(attribute(ErrorKind::HttpStatus(414)), Attribution::Proxy);
    }

    #[test]
    fn account_error_never_recovers_automatically() {
        let c = RiskCounters { consecutive_success: 100, ..Default::default() };
        assert_eq!(decide_account(AccountStatus::Error, 0, 0, &c), AccountStatus::Error);
    }
}
