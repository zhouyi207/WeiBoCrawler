use std::sync::atomic::{AtomicBool, Ordering};

static ALLOW_CLOSE_ONCE: AtomicBool = AtomicBool::new(false);

/// 下一帧 `CloseRequested` 时不再 `prevent_close`，用于前端在确认后调用 `close()`。
#[tauri::command]
pub fn allow_close_once() {
    ALLOW_CLOSE_ONCE.store(true, Ordering::SeqCst);
}

/// 若已允许关闭则返回 `true` 并清除标志；否则返回 `false`。
pub(crate) fn take_allow_close() -> bool {
    ALLOW_CLOSE_ONCE.swap(false, Ordering::SeqCst)
}
