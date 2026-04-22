use tokio::sync::mpsc;

use crate::db::{app_event_repo, crawl_request_repo, enum_to_str, task_repo};
use crate::db::Database;
use crate::error::AppError;
use crate::model::crawl_request::{CrawlRequest, CrawlRequestStatus, CrawlRequestType, TaskProgress};
use crate::model::platform::Platform;
use crate::model::task::{CrawlStrategy, CrawlTask, RateLimitScope, TaskStatus, TaskType};
use crate::model::weibo_task::WeiboTaskPayload;
use crate::queue::dispatcher;
use crate::queue::message::CrawlCommand;

pub fn list_tasks(
    db: &Database,
    platform: Option<&str>,
) -> Result<Vec<CrawlTask>, AppError> {
    let conn = db.conn();
    task_repo::list(&conn, platform)
}

fn validate_weibo_task(task_type: TaskType, cfg: &WeiboTaskPayload) -> Result<(), AppError> {
    match (task_type, cfg) {
        (TaskType::Keyword, WeiboTaskPayload::List { .. }) => Ok(()),
        (TaskType::UserProfile, WeiboTaskPayload::Body { .. }) => Ok(()),
        (TaskType::CommentLevel1, WeiboTaskPayload::CommentL1 { .. }) => Ok(()),
        (TaskType::CommentLevel2, WeiboTaskPayload::CommentL2 { .. }) => Ok(()),
        (TaskType::Trending, _) => Ok(()),
        _ => Err(AppError::Internal(
            "微博任务类型与 weiboConfig.api 不一致：keyword→list，user_profile→body，comment_level1→comment_l1，comment_level2→comment_l2"
                .into(),
        )),
    }
}

fn normalize_weibo_config(
    platform: Platform,
    task_type: TaskType,
    weibo_config: Option<WeiboTaskPayload>,
) -> Result<Option<WeiboTaskPayload>, AppError> {
    if platform != Platform::Weibo {
        if matches!(
            task_type,
            TaskType::CommentLevel1 | TaskType::CommentLevel2
        ) {
            return Err(AppError::Internal(
                "一级/二级评论任务仅限微博平台".into(),
            ));
        }
        return Ok(None);
    }
    match task_type {
        TaskType::Trending => Ok(None),
        _ => {
            let cfg = weibo_config.ok_or_else(|| {
                AppError::Internal("微博任务需提交 weiboConfig（对齐 WeiBoCrawler 请求参数）".into())
            })?;
            validate_weibo_task(task_type, &cfg)?;
            match &cfg {
                WeiboTaskPayload::List { search_for, .. } if search_for.trim().is_empty() => {
                    Err(AppError::Internal("列表搜索关键词不能为空".into()))
                }
                WeiboTaskPayload::Body { status_ids } if status_ids.is_empty() => {
                    Err(AppError::Internal("详细页至少填写一条微博 id".into()))
                }
                WeiboTaskPayload::CommentL1 { pairs } | WeiboTaskPayload::CommentL2 { pairs }
                    if pairs.is_empty() =>
                {
                    Err(AppError::Internal("评论任务至少一对 uid/mid".into()))
                }
                _ => Ok(Some(cfg)),
            }
        }
    }
}

pub fn create_task(
    db: &Database,
    platform: &str,
    task_type: &str,
    name: &str,
    strategy: &str,
    rate_limit: i64,
    account_ids: Option<Vec<String>>,
    proxy_ids: Option<Vec<String>>,
    rate_limit_scope: Option<&str>,
    weibo_config: Option<WeiboTaskPayload>,
) -> Result<CrawlTask, AppError> {
    let platform_enum = crate::db::str_to_enum(platform)?;
    let task_type_enum = crate::db::str_to_enum::<TaskType>(task_type)?;
    let weibo_config = normalize_weibo_config(platform_enum, task_type_enum, weibo_config)?;

    let account_pool = account_ids.as_ref().map(|v| v.len() as i64).unwrap_or(0);
    let proxy_pool = proxy_ids.as_ref().map(|v| v.len() as i64).unwrap_or(0);
    let scope = parse_rate_limit_scope(rate_limit_scope)?;
    let task = CrawlTask {
        id: uuid::Uuid::new_v4().to_string(),
        platform: platform_enum,
        task_type: task_type_enum,
        name: name.to_string(),
        status: TaskStatus::Paused,
        strategy: crate::db::str_to_enum::<CrawlStrategy>(strategy)?,
        rate_limit,
        account_pool_size: account_pool,
        ip_pool_size: proxy_pool,
        created_at: now_string(),
        bound_account_ids: account_ids,
        bound_proxy_ids: proxy_ids,
        rate_limit_scope: scope,
        weibo_config,
    };
    let conn = db.conn();
    task_repo::insert(&conn, &task)?;
    app_event_repo::try_insert(
        &conn,
        "task",
        "create",
        "info",
        &format!(
            "创建任务「{}」（{} · {}）",
            task.name,
            enum_to_str(&task.platform),
            enum_to_str(&task.task_type),
        ),
        Some("task"),
        Some(&task.id),
        Some(&task.id),
    );
    Ok(task)
}

fn parse_rate_limit_scope(raw: Option<&str>) -> Result<RateLimitScope, AppError> {
    match raw {
        None | Some("") => Ok(RateLimitScope::default()),
        Some(s) => crate::db::str_to_enum::<RateLimitScope>(s),
    }
}

pub fn delete_task(db: &Database, id: &str) -> Result<(), AppError> {
    let conn = db.conn();
    let meta = task_repo::get_by_id(&conn, id).ok().map(|t| {
        (
            t.name.clone(),
            enum_to_str(&t.platform),
        )
    });
    task_repo::delete(&conn, id)?;
    let msg = match meta {
        Some((name, plat)) => format!("删除任务「{name}」（{plat}）"),
        None => format!("删除任务（id={id}）"),
    };
    app_event_repo::try_insert(
        &conn,
        "task",
        "delete",
        "info",
        &msg,
        Some("task"),
        Some(id),
        None,
    );
    Ok(())
}

/// 更新任务可编辑字段（不改 `id` / `platform` / `task_type` / `status` / `created_at`）。
pub fn update_task(
    db: &Database,
    id: &str,
    name: &str,
    strategy: &str,
    rate_limit: i64,
    account_ids: Option<Vec<String>>,
    proxy_ids: Option<Vec<String>>,
    rate_limit_scope: Option<&str>,
    weibo_config: Option<WeiboTaskPayload>,
) -> Result<CrawlTask, AppError> {
    let conn = db.conn();
    let mut task = task_repo::get_by_id(&conn, id)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Internal("任务名称不能为空".into()));
    }
    let weibo_config = normalize_weibo_config(task.platform, task.task_type, weibo_config)?;
    let account_pool = account_ids.as_ref().map(|v| v.len() as i64).unwrap_or(0);
    let proxy_pool = proxy_ids.as_ref().map(|v| v.len() as i64).unwrap_or(0);
    task.name = name.to_string();
    task.strategy = crate::db::str_to_enum::<CrawlStrategy>(strategy)?;
    task.rate_limit = rate_limit;
    task.bound_account_ids = account_ids;
    task.account_pool_size = account_pool;
    task.bound_proxy_ids = proxy_ids;
    task.ip_pool_size = proxy_pool;
    task.rate_limit_scope = parse_rate_limit_scope(rate_limit_scope)?;
    task.weibo_config = weibo_config;
    task_repo::update(&conn, &task)?;
    app_event_repo::try_insert(
        &conn,
        "task",
        "update",
        "info",
        &format!(
            "更新任务「{}」（{}）",
            task.name,
            enum_to_str(&task.platform),
        ),
        Some("task"),
        Some(id),
        Some(id),
    );
    Ok(task)
}

pub fn start_task(
    db: &Database,
    queue_tx: &mpsc::Sender<CrawlCommand>,
    id: &str,
) -> Result<(), AppError> {
    let conn = db.conn();
    let task = task_repo::get_by_id(&conn, id)?;

    if !crawl_request_repo::has_any(&conn, id)? {
        expand_task(&conn, &task)?;
    } else {
        crawl_request_repo::reset_running_to_pending(&conn, id)?;
    }

    task_repo::update_status(&conn, id, TaskStatus::Running)?;
    drop(conn);

    let cmd = CrawlCommand {
        task_id: task.id,
        platform: crate::db::enum_to_str(&task.platform),
        task_type: crate::db::enum_to_str(&task.task_type),
        weibo_config: task.weibo_config.clone(),
    };
    dispatcher::dispatch(queue_tx, cmd)?;
    Ok(())
}

pub fn pause_task(db: &Database, id: &str) -> Result<(), AppError> {
    let conn = db.conn();
    task_repo::update_status(&conn, id, TaskStatus::Paused)
}

/// 异常退出后重新打开应用：将所有仍为 `running` 的任务改为暂停，并把卡在 `running` 的请求行恢复为 `pending`。
pub fn reconcile_stale_running_tasks_to_paused(db: &Database) -> Result<(), AppError> {
    let conn = db.conn();
    let all = task_repo::list(&conn, None)?;
    for task in all
        .into_iter()
        .filter(|t| t.status == TaskStatus::Running)
    {
        crawl_request_repo::reset_running_to_pending(&conn, &task.id)?;
        task_repo::update_status(&conn, &task.id, TaskStatus::Paused)?;
    }
    Ok(())
}

pub fn get_task_progress(db: &Database, task_id: &str) -> Result<TaskProgress, AppError> {
    let conn = db.conn();
    crawl_request_repo::count_by_status(&conn, task_id)
}

/// Delete all crawl_requests for a task, re-expand from config, and start scheduling.
pub fn restart_task(
    db: &Database,
    queue_tx: &mpsc::Sender<CrawlCommand>,
    id: &str,
) -> Result<(), AppError> {
    let conn = db.conn();
    let task = task_repo::get_by_id(&conn, id)?;
    crawl_request_repo::delete_by_task(&conn, id)?;
    expand_task(&conn, &task)?;
    task_repo::update_status(&conn, id, TaskStatus::Running)?;
    drop(conn);

    let cmd = CrawlCommand {
        task_id: task.id,
        platform: crate::db::enum_to_str(&task.platform),
        task_type: crate::db::enum_to_str(&task.task_type),
        weibo_config: task.weibo_config.clone(),
    };
    dispatcher::dispatch(queue_tx, cmd)?;
    Ok(())
}

pub fn retry_failed_requests(
    db: &Database,
    queue_tx: &mpsc::Sender<CrawlCommand>,
    task_id: &str,
) -> Result<u64, AppError> {
    let conn = db.conn();
    let task = task_repo::get_by_id(&conn, task_id)?;
    let count = crawl_request_repo::reset_failed_to_pending(&conn, task_id)?;
    if count > 0 {
        task_repo::update_status(&conn, task_id, TaskStatus::Running)?;
        drop(conn);
        let cmd = CrawlCommand {
            task_id: task.id,
            platform: crate::db::enum_to_str(&task.platform),
            task_type: crate::db::enum_to_str(&task.task_type),
            weibo_config: task.weibo_config.clone(),
        };
        dispatcher::dispatch(queue_tx, cmd)?;
    }
    Ok(count)
}

const LIST_MAX_PAGES: i32 = 50;

/// Generate initial `crawl_requests` rows from the task configuration.
fn expand_task(
    conn: &rusqlite::Connection,
    task: &CrawlTask,
) -> Result<(), AppError> {
    let now = now_iso();
    let weibo = task.weibo_config.as_ref().ok_or_else(|| {
        AppError::Internal("任务缺少 weiboConfig，无法展开请求".into())
    })?;

    let mut requests: Vec<CrawlRequest> = Vec::new();

    match weibo {
        WeiboTaskPayload::List {
            search_for,
            list_kind,
            advanced_kind,
            time_start,
            time_end,
        } => {
            for page in 1..=LIST_MAX_PAGES {
                let params = serde_json::json!({
                    "search_for": search_for,
                    "page": page,
                    "list_kind": list_kind,
                    "advanced_kind": advanced_kind,
                    "time_start": time_start,
                    "time_end": time_end,
                });
                requests.push(CrawlRequest {
                    id: uuid::Uuid::new_v4().to_string(),
                    task_id: task.id.clone(),
                    request_type: CrawlRequestType::ListPage,
                    request_params: params.to_string(),
                    status: CrawlRequestStatus::Pending,
                    account_id: None,
                    proxy_id: None,
                    error_message: None,
                    response_summary: None,
                    response_data: None,
                    parent_request_id: None,
                    retry_count: 0,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                });
            }
        }
        WeiboTaskPayload::Body { status_ids } => {
            for sid in status_ids {
                let params = serde_json::json!({ "status_id": sid });
                requests.push(CrawlRequest {
                    id: uuid::Uuid::new_v4().to_string(),
                    task_id: task.id.clone(),
                    request_type: CrawlRequestType::Body,
                    request_params: params.to_string(),
                    status: CrawlRequestStatus::Pending,
                    account_id: None,
                    proxy_id: None,
                    error_message: None,
                    response_summary: None,
                    response_data: None,
                    parent_request_id: None,
                    retry_count: 0,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                });
            }
        }
        WeiboTaskPayload::CommentL1 { pairs } => {
            for p in pairs {
                let params = serde_json::json!({ "uid": p.uid, "mid": p.mid });
                requests.push(CrawlRequest {
                    id: uuid::Uuid::new_v4().to_string(),
                    task_id: task.id.clone(),
                    request_type: CrawlRequestType::CommentL1,
                    request_params: params.to_string(),
                    status: CrawlRequestStatus::Pending,
                    account_id: None,
                    proxy_id: None,
                    error_message: None,
                    response_summary: None,
                    response_data: None,
                    parent_request_id: None,
                    retry_count: 0,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                });
            }
        }
        WeiboTaskPayload::CommentL2 { pairs } => {
            for p in pairs {
                let params = serde_json::json!({ "uid": p.uid, "mid": p.mid });
                requests.push(CrawlRequest {
                    id: uuid::Uuid::new_v4().to_string(),
                    task_id: task.id.clone(),
                    request_type: CrawlRequestType::CommentL2,
                    request_params: params.to_string(),
                    status: CrawlRequestStatus::Pending,
                    account_id: None,
                    proxy_id: None,
                    error_message: None,
                    response_summary: None,
                    response_data: None,
                    parent_request_id: None,
                    retry_count: 0,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                });
            }
        }
    }

    crawl_request_repo::insert_batch(conn, &requests)?;
    Ok(())
}

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn now_string() -> String {
    now_iso()
}
