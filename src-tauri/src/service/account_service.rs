use crate::db::account_repo;
use crate::db::app_event_repo;
use crate::db::enum_to_str;
use crate::db::proxy_repo;
use crate::db::risk_event_repo::{self, AccountLogEntry};
use crate::db::Database;
use crate::error::AppError;
use crate::model::account::{Account, AccountStatus, GenerateQrResponse, WeiboQrPollResponse};
use chrono::Local;
use crate::model::platform::Platform;
use crate::model::proxy::ProxyIp;
use crate::weibo::request_weibo_login_qr;

pub fn list_accounts(
    db: &Database,
    platform: Option<&str>,
) -> Result<Vec<Account>, AppError> {
    let conn = db.conn();
    account_repo::list(&conn, platform)
}

/// `ip_id`：`proxies.id`。二维码 HTTP 请求经该代理出口发出（含用户自建的「本机」代理行）。
pub fn generate_login_qr(
    state: &crate::AppState,
    platform: &str,
    ip_id: Option<&str>,
) -> Result<GenerateQrResponse, AppError> {
    let parsed_platform: Platform = crate::db::str_to_enum(platform)?;

    let id = ip_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Internal("请选择代理".into()))?;

    let proxy: ProxyIp = {
        let conn = state.db.conn();
        proxy_repo::get_by_id(&conn, id)?
    };

    let account_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    let bound_display = proxy.address.clone();
    let bound_proxy_id = Some(proxy.id.clone());

    let account = Account {
        id: account_id.clone(),
        platform: parsed_platform,
        username: format!("pending_{}", &account_id[..8]),
        bound_ip: Some(bound_display),
        bound_proxy_id,
        risk_status: AccountStatus::Normal,
        created_at: now.clone(),
        last_active_at: now,
        cookies: None,
        weibo_profile: None,
    };

    if matches!(parsed_platform, Platform::Weibo) {
        let pt = crate::db::enum_to_str(&proxy.proxy_type);
        let (session, qr_data_url) =
            request_weibo_login_qr(Some(proxy.address.as_str()), &pt)?;

        // 微博走「先草稿、扫码成功才入库」流程：
        // - pending_accounts 持有未完成 Account，二维码失效 / 用户关弹窗都不会脏化 accounts 表；
        // - weibo_sessions 持有 reqwest 客户端 & 二维码 qrid，poll 路径靠它跑。
        // 两个 map 用同一个 account_id 串起来。
        state
            .pending_accounts
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?
            .insert(account_id.clone(), account);

        state
            .weibo_sessions
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?
            .insert(account_id.clone(), session);

        return Ok(GenerateQrResponse {
            account_id,
            qr_data: qr_data_url,
        });
    }

    // 非微博平台暂未接入扫码 / OAuth 流程，仍保持「立即落库」语义；
    // 后续接其它平台时统一改成 pending → 成功才入库。
    let conn = state.db.conn();
    account_repo::insert(&conn, &account)?;
    app_event_repo::try_insert(
        &conn,
        "account",
        "create",
        "info",
        &format!(
            "添加账号 {}（{}）",
            account.username,
            enum_to_str(&account.platform)
        ),
        Some("account"),
        Some(&account_id),
        None,
    );
    Ok(GenerateQrResponse {
        account_id,
        qr_data: String::new(),
    })
}

pub fn poll_weibo_qr_login(
    state: &crate::AppState,
    account_id: &str,
) -> Result<WeiboQrPollResponse, AppError> {
    let session = {
        let guard = state
            .weibo_sessions
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        guard.get(account_id).cloned().ok_or_else(|| {
            AppError::NotFound(format!("no active weibo QR session for {account_id}"))
        })?
    };

    let mut res = crate::weibo::poll_weibo_qr_once(&session)?;

    match res.status.as_str() {
        "success" => {
            let cookies = res
                .cookies
                .clone()
                .ok_or_else(|| AppError::Internal("weibo poll: missing cookies".into()))?;

            // 取出 generate_login_qr 时落下的草稿。理论上一定存在；
            // 不存在多半是热重载 / 进程重启把内存清空了——回 NotFound 让前端重走生成。
            let mut draft = state
                .pending_accounts
                .lock()
                .map_err(|e| AppError::Internal(e.to_string()))?
                .remove(account_id)
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "no pending account draft for {account_id}, please regenerate QR"
                    ))
                })?;
            // 草稿扫码成功：写入 Cookie 并刷新最后活跃时间（添加时间保持草稿生成时的 created_at）。
            draft.cookies = Some(cookies);
            draft.last_active_at = Local::now().format("%Y-%m-%d %H:%M").to_string();

            let conn = state.db.conn();
            account_repo::insert(&conn, &draft)?;
            app_event_repo::try_insert(
                &conn,
                "account",
                "create",
                "info",
                &format!("添加微博账号 {}（{}）", draft.username, enum_to_str(&draft.platform)),
                Some("account"),
                Some(account_id),
                None,
            );
            let merged_into = match crate::weibo::enrich_account_from_my_sina_session(
                &conn,
                account_id,
                &session.client,
                Some(state.weibo_my_sina_debug_dir.as_path()),
            ) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("weibo my.sina profile enrich: {e}");
                    None
                }
            };
            let cookie_target_id = merged_into.as_deref().unwrap_or(account_id);
            // 资料页请求可能再写入 Cookie，从同一 Jar 再导出一次写入库
            match crate::weibo::cookies_json_from_jar(&session.cookie_jar) {
                Ok(full) if full.len() > 2 => {
                    if let Err(e) = account_repo::update_cookies(&conn, cookie_target_id, &full) {
                        log::warn!("weibo update cookies after my.sina: {e}");
                    }
                }
                Ok(_) | Err(_) => {}
            }
            if let Some(id) = merged_into {
                res.merged_into_account_id = Some(id);
            }
            drop(conn);
            state
                .weibo_sessions
                .lock()
                .map_err(|e| AppError::Internal(e.to_string()))?
                .remove(account_id);
        }
        "failed" => {
            // 失败：草稿直接丢，accounts 表保持干净；前端会自动重新调 generate_login_qr。
            if let Ok(mut g) = state.pending_accounts.lock() {
                g.remove(account_id);
            }
            state
                .weibo_sessions
                .lock()
                .map_err(|e| AppError::Internal(e.to_string()))?
                .remove(account_id);
        }
        _ => {}
    }

    Ok(res)
}

pub fn delete_account(state: &crate::AppState, id: &str) -> Result<(), AppError> {
    if let Ok(mut g) = state.weibo_sessions.lock() {
        g.remove(id);
    }
    // 防御性：账号还没扫码成功就被前端「删除」时，把内存草稿一并清掉，避免泄漏。
    if let Ok(mut g) = state.pending_accounts.lock() {
        g.remove(id);
    }
    let conn = state.db.conn();
    let msg = account_repo::get_by_id(&conn, id)
        .map(|a| {
            format!(
                "删除账号 {}（{}）",
                a.username,
                enum_to_str(&a.platform)
            )
        })
        .unwrap_or_else(|_| format!("删除账号（id={id}）"));
    account_repo::delete(&conn, id)?;
    app_event_repo::try_insert(
        &conn,
        "account",
        "delete",
        "info",
        &msg,
        Some("account"),
        Some(id),
        None,
    );
    Ok(())
}

/// 拉取某账号最近 `limit` 条失败事件。`limit` None 时默认 100，硬上限 200，
/// 与 `proxy_service::list_proxy_logs` 对齐——同样的 modal 只看时间线，不做翻页。
pub fn list_account_logs(
    db: &Database,
    account_id: &str,
    limit: Option<i64>,
) -> Result<Vec<AccountLogEntry>, AppError> {
    let lim = limit.unwrap_or(100).clamp(1, 200);
    let conn = db.conn();
    risk_event_repo::list_account_logs(&conn, account_id, lim)
}
