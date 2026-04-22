//! 「最近一次响应」内存缓冲，用于把 worker 的高频 upsert 合并成 1s/批。
//!
//! ## 背景
//!
//! 之前 [`crate::queue::worker`] 在每条请求结束后都直接 `proxy_runtime_repo::upsert`，
//! 高并发场景下（多 worker × 同 (proxy, platform)）会产生大量
//! "覆盖前一条又被后一条覆盖" 的写盘动作。SQLite 在 `journal_mode=WAL` + 高频
//! 单行 upsert 下 fsync 抖动很明显，会反向拖慢真正的爬取请求。
//!
//! 由于「最后一次响应」这个画像本身就是 *按 (proxy, platform) 去重保留最新*，
//! 把同 key 的多条样本在内存里折叠后再批量落盘，并不会丢失语义——
//! 唯一的副作用是 IP 管理页 per-platform tab 的「最后一次响应时间」
//! 最多滞后 1 个 flush 周期 (≤ 1s)，这对人眼判定"现在还活着吗"完全够用。
//!
//! ## 实现
//!
//! - `Mutex<HashMap<(proxy_id, platform), OwnedSample>>`，按 key 去重；
//! - `start_flusher` 启动一个 1s tick 的 tokio task：
//!   1. 把 map drain 走（持锁时间 < 1ms）；
//!   2. 在数据库连接上开 BEGIN IMMEDIATE 事务，逐条 upsert；
//!   3. 失败仅记录日志，不影响下一周期。
//!
//! ## 关掉 / 直写降级
//!
//! 当前不暴露开关；如果将来想关掉合并（比如调试时想 1:1 落盘观察），
//! 把 `push` 改成调用旧 [`proxy_runtime_repo::upsert`] 即可。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::db::proxy_runtime_repo;

/// flush 周期。1s 是 IP 管理页人眼可接受的"最近一次"刷新延迟上限。
const FLUSH_INTERVAL: Duration = Duration::from_secs(1);

/// 单条 owned 样本，与 [`proxy_runtime_repo::RuntimeSample`] 的字段一一对应，
/// 区别只在所有字段都拥有所有权（map 需要 owned key/value）。
#[derive(Debug, Clone)]
pub struct OwnedSample {
    pub account_id: String,
    pub latency_ms: i64,
    pub status: String,
    pub error_kind: Option<String>,
    pub http_status: Option<i64>,
    pub responded_at: String,
}

/// 内存缓冲。`Arc<Self>` 既被 worker 共享（写），也被 flusher task 共享（读+清空）。
pub struct RuntimeBuffer {
    pending: Mutex<HashMap<(String, String), OwnedSample>>,
}

impl RuntimeBuffer {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// 推入一条新的 (proxy, platform) 样本。同 key 的旧样本会被直接覆盖
    /// ——这正是「最近一次」的语义。
    pub fn push(
        &self,
        proxy_id: &str,
        platform: &str,
        sample: OwnedSample,
    ) {
        let key = (proxy_id.to_string(), platform.to_string());
        if let Ok(mut g) = self.pending.lock() {
            g.insert(key, sample);
        }
    }

    /// 取走当前所有 pending 样本，返回 owned vec。`flusher` 用这个把锁尽快释放。
    fn drain(&self) -> Vec<((String, String), OwnedSample)> {
        match self.pending.lock() {
            Ok(mut g) => g.drain().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 启动一个 task，每 [`FLUSH_INTERVAL`] 把 buffer 批量落盘。
    /// task 的生命周期与 `Arc<Self>` 一致：buffer 还有别的持有者时 task 就一直跑。
    /// 应用退出时 Tauri runtime 会停掉所有 task，这里不需要显式 cancel。
    ///
    /// 调用方需自行用 `Database::open_crawl_connection` 拿一条独立的 `Connection`
    /// 传进来——避免抢主 `db.conn()` 的 Mutex，阻塞 Tauri 命令线程。
    ///
    /// 使用 `tauri::async_runtime::spawn` 而不是 `tokio::spawn`：
    /// 在 Tauri `setup` 回调里调用此函数时尚未进入裸 tokio runtime，
    /// 直接 `tokio::spawn` 会 panic "there is no reactor running"。
    /// `tauri::async_runtime` 在内部桥接到 tokio，并且无论从哪个上下文
    /// 调用都能正确派发 task。
    pub fn start_flusher(self: Arc<Self>, mut conn: rusqlite::Connection) {
        tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
            // 第一次 tick 立即触发，跳过——避免应用刚启动时 buffer 还没东西就开锁。
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let pending = self.drain();
                if pending.is_empty() {
                    continue;
                }
                if let Err(e) = flush_batch(&mut conn, &pending) {
                    log::warn!(
                        "[runtime_buffer] flush {n} samples failed: {e}",
                        n = pending.len()
                    );
                }
            }
        });
    }
}

impl Default for RuntimeBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// 在单个事务里把 `pending` 的所有样本 upsert 一遍。
/// 任意一条失败整个事务回滚，下一周期会重新累积——不丢样本，但可能丢"中间快照"。
fn flush_batch(
    conn: &mut rusqlite::Connection,
    pending: &[((String, String), OwnedSample)],
) -> Result<(), crate::error::AppError> {
    let tx = conn.transaction()?;
    for ((proxy_id, platform), sample) in pending {
        let s = proxy_runtime_repo::RuntimeSample {
            proxy_id,
            platform,
            account_id: &sample.account_id,
            latency_ms: sample.latency_ms,
            status: &sample.status,
            error_kind: sample.error_kind.as_deref(),
            http_status: sample.http_status,
            responded_at: &sample.responded_at,
        };
        proxy_runtime_repo::upsert(&tx, &s)?;
    }
    tx.commit()?;
    Ok(())
}
