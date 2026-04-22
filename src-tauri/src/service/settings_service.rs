//! 应用全局配置服务。当前承载：
//! - 界面主题（`ui.theme`：`light` / `dark`）；
//! - IP 代理双探针目标 URL；
//! - 采集 Worker 熔断退避时长（按任务平台）。
//!
//! 设计：
//! - 后端固化默认值（见下方常量），前端没改过就用默认；
//! - 所有「读」对 None 用默认值兜底；
//! - 探针 URL 写前校验 `http(s)://` 前缀。
//!
//! 之所以在 service 层做默认值兜底而不是在迁移里 INSERT 一条 seed：
//! 用户改完后想"恢复默认"时，直接清空 `app_settings` 行即可，不会被 seed 干扰。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::db::{settings_repo, Database};
use crate::error::AppError;
use crate::queue::risk::WORKER_CB_BACKOFF_MS;

/// 国内探针目标默认值。选 `baidu.com/favicon.ico`：
/// - 长期稳定，HTTPS 全国 CDN，国内出口几乎都能命中；
/// - 体积仅几百字节，单次请求很轻；
/// - 不依赖 Cookie / UA，不会被反爬。
pub const DEFAULT_CN_TARGET: &str = "https://www.baidu.com/favicon.ico";

/// 国外探针目标默认值。选 Cloudflare anycast trace 端点：
/// - 全球 anycast，"是否真的能出墙"会非常清晰地反映在延迟差上；
/// - 响应是几行明文，零依赖；
/// - 持续可用性极高（CF 自身基础设施）。
pub const DEFAULT_INTL_TARGET: &str = "https://www.cloudflare.com/cdn-cgi/trace";

const KEY_UI_THEME: &str = "ui.theme";

const KEY_CN_TARGET: &str = "proxy.latency_probe.cn_target";
const KEY_INTL_TARGET: &str = "proxy.latency_probe.intl_target";

/// `app_settings` 中存 JSON：`{"weibo":30,"douyin":60,...}`，缺省键用 [`DEFAULT_WORKER_BACKOFF_SECS`]。
const KEY_WORKER_BACKOFF_SEC: &str = "worker.circuit_breaker.backoff_seconds_by_platform";

/// 与 [`crate::queue::risk::WORKER_CB_BACKOFF_MS`] 对齐的默认退避秒数（30s）。
pub const DEFAULT_WORKER_BACKOFF_SECS: u64 = WORKER_CB_BACKOFF_MS / 1000;

/// 双探针目标 URL 配置。前端 settings dialog 直接消费此结构。
///
/// `default_*_target` 字段把后端常量同时下发给前端，让「恢复默认」按钮不再
/// 在前端硬编码同一份字符串，避免后端改了默认前端没跟上时两边不一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyProbeSettings {
    pub cn_target: String,
    pub intl_target: String,
    pub default_cn_target: String,
    pub default_intl_target: String,
}

impl ProxyProbeSettings {
    pub fn defaults() -> Self {
        Self {
            cn_target: DEFAULT_CN_TARGET.to_string(),
            intl_target: DEFAULT_INTL_TARGET.to_string(),
            default_cn_target: DEFAULT_CN_TARGET.to_string(),
            default_intl_target: DEFAULT_INTL_TARGET.to_string(),
        }
    }
}

/// 采集 worker 连续失败达到阈值后的退避时长（按任务 `platform` 区分）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerBackoffSettings {
    /// 键为 `Platform::as_tag()`（`weibo`、`douyin` 等）。未出现的键用 `defaultBackoffSeconds`。
    pub seconds_by_platform: HashMap<String, u64>,
    /// 与 [`DEFAULT_WORKER_BACKOFF_SECS`] 相同，下发给前端「恢复默认」用。
    pub default_backoff_seconds: u64,
}

/// Tauri `update_worker_backoff_settings` 入参。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkerBackoffPayload {
    pub seconds_by_platform: HashMap<String, u64>,
}

/// 读取退避配置；`app_settings` 无记录或解析失败时返回空 map + 默认秒数。
pub fn get_worker_backoff_settings(db: &Database) -> Result<WorkerBackoffSettings, AppError> {
    let conn = db.conn();
    let raw = settings_repo::get(&conn, KEY_WORKER_BACKOFF_SEC)?;
    let seconds_by_platform: HashMap<String, u64> = raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    Ok(WorkerBackoffSettings {
        seconds_by_platform,
        default_backoff_seconds: DEFAULT_WORKER_BACKOFF_SECS,
    })
}

/// 写入整表；每项必须在 1–3600 秒。
///
/// ⚠ `Database::conn()` 是 `std::sync::Mutex`、**不可重入**：写完必须先 drop
/// 当前 MutexGuard，再调用 `get_worker_backoff_settings`（其内部会再次 lock），
/// 否则同线程二次 lock 直接死锁，前端表现为「保存按钮一直转圈、整窗卡死」。
pub fn update_worker_backoff_settings(
    db: &Database,
    seconds_by_platform: HashMap<String, u64>,
) -> Result<WorkerBackoffSettings, AppError> {
    for (k, v) in &seconds_by_platform {
        if !(1..=3600).contains(v) {
            return Err(AppError::Internal(format!(
                "退避时长须在 1–3600 秒之间：{k}={v}"
            )));
        }
    }
    let json = serde_json::to_string(&seconds_by_platform)?;
    {
        let conn = db.conn();
        settings_repo::set(&conn, KEY_WORKER_BACKOFF_SEC, &json)?;
    }
    get_worker_backoff_settings(db)
}

/// 当前任务平台对应的退避毫秒数；读库失败时用 [`WORKER_CB_BACKOFF_MS`]。
pub fn worker_backoff_ms_for_platform(db: &Database, platform_tag: &str) -> u64 {
    match get_worker_backoff_settings(db) {
        Ok(s) => {
            let secs = s
                .seconds_by_platform
                .get(platform_tag)
                .copied()
                .unwrap_or(s.default_backoff_seconds);
            secs.max(1).min(3600) * 1000
        }
        Err(_) => WORKER_CB_BACKOFF_MS,
    }
}

/// 读取当前生效的双探针目标。任意一个 key 缺失就用默认值兜底。
pub fn get_proxy_probe_settings(db: &Database) -> Result<ProxyProbeSettings, AppError> {
    let conn = db.conn();
    let cn = settings_repo::get(&conn, KEY_CN_TARGET)?
        .unwrap_or_else(|| DEFAULT_CN_TARGET.to_string());
    let intl = settings_repo::get(&conn, KEY_INTL_TARGET)?
        .unwrap_or_else(|| DEFAULT_INTL_TARGET.to_string());
    Ok(ProxyProbeSettings {
        cn_target: cn,
        intl_target: intl,
        default_cn_target: DEFAULT_CN_TARGET.to_string(),
        default_intl_target: DEFAULT_INTL_TARGET.to_string(),
    })
}

/// 写入双探针目标。允许任一字段传 `None` 表示"不修改这一项"——前端只想改一项时
/// 不必把另一项当前值再回传一次。
pub fn update_proxy_probe_settings(
    db: &Database,
    cn_target: Option<&str>,
    intl_target: Option<&str>,
) -> Result<ProxyProbeSettings, AppError> {
    if let Some(v) = cn_target {
        validate_url(v, "cn_target")?;
        let conn = db.conn();
        settings_repo::set(&conn, KEY_CN_TARGET, v)?;
    }
    if let Some(v) = intl_target {
        validate_url(v, "intl_target")?;
        let conn = db.conn();
        settings_repo::set(&conn, KEY_INTL_TARGET, v)?;
    }
    get_proxy_probe_settings(db)
}

pub fn get_ui_theme(db: &Database) -> Result<Option<String>, AppError> {
    let conn = db.conn();
    settings_repo::get(&conn, KEY_UI_THEME)
}

pub fn set_ui_theme(db: &Database, mode: &str) -> Result<(), AppError> {
    if mode != "dark" && mode != "light" {
        return Err(AppError::Internal("ui.theme 仅支持 light 或 dark".into()));
    }
    let conn = db.conn();
    settings_repo::set(&conn, KEY_UI_THEME, mode)
}

fn validate_url(v: &str, field: &str) -> Result<(), AppError> {
    let trimmed = v.trim();
    if trimmed.is_empty() {
        return Err(AppError::Internal(format!("{field} 不能为空")));
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(AppError::Internal(format!(
            "{field} 必须以 http:// 或 https:// 开头：{trimmed}"
        )));
    }
    Ok(())
}
