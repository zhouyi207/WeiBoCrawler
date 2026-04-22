//! 运行中 worker 的内存注册表。
//!
//! 用途：让 IP 代理页 per-platform tab 的「运行账号数量」列能拿到真实数字。
//! - 不入库——重启即清零，避免遗留脏数据；
//! - 维护粒度 `(proxy_id, platform) → set<account_id>`：同一 (account, proxy)
//!   组合即使被多个 worker 复用（理论上目前一对一）也不会重复计数。
//! - 读取走 [`count`] / [`snapshot`]，写入走 [`Guard`] 的 RAII：
//!   `register` 拿到 guard，drop 时自动 unregister，避免 worker 早退漏减。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct WorkerRegistry {
    inner: Arc<Mutex<HashMap<(String, String), HashSet<String>>>>,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个 (proxy_id, platform, account_id) 三元组，返回 RAII guard：
    /// guard drop 时会自动撤销注册，确保 worker panic / 早退时不会留下"幽灵在跑"。
    pub fn register(&self, proxy_id: &str, platform: &str, account_id: &str) -> Guard {
        let key = (proxy_id.to_string(), platform.to_string());
        if let Ok(mut g) = self.inner.lock() {
            g.entry(key.clone()).or_default().insert(account_id.to_string());
        }
        Guard {
            map: self.inner.clone(),
            key,
            account_id: account_id.to_string(),
        }
    }

    /// 当前 (proxy_id, platform) 上正在运行的账号数。
    #[allow(dead_code)]
    pub fn count(&self, proxy_id: &str, platform: &str) -> i64 {
        let key = (proxy_id.to_string(), platform.to_string());
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.get(&key).map(|s| s.len() as i64))
            .unwrap_or(0)
    }

    /// 一次性把所有 (proxy_id, platform) → 账号数 拉成 map，
    /// 给 `proxy_service::list_proxies_runtime` 一次组装多行用。
    pub fn snapshot(&self) -> HashMap<(String, String), i64> {
        self.inner
            .lock()
            .ok()
            .map(|g| {
                g.iter()
                    .map(|(k, v)| (k.clone(), v.len() as i64))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// RAII 守卫：drop 时把 (proxy_id, platform, account_id) 从注册表里移除，
/// 空集合自动清理，避免 map 长期堆积空 set。
pub struct Guard {
    map: Arc<Mutex<HashMap<(String, String), HashSet<String>>>>,
    key: (String, String),
    account_id: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut g) = self.map.lock() {
            if let Some(set) = g.get_mut(&self.key) {
                set.remove(&self.account_id);
                if set.is_empty() {
                    g.remove(&self.key);
                }
            }
        }
    }
}
