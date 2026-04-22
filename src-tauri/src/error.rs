use std::fmt;

/// 采集 / 系统错误。变体粒度配合 [`crate::queue::risk`] 做账号 / 代理风控归因：
/// - [`AppError::Network`]：连接 / DNS / 隧道 / 读超时 —— 倾向归责代理。
/// - [`AppError::HttpStatus`]：服务端 4xx/5xx —— 状态码内容决定归责。
/// - [`AppError::LoginRequired`]：Cookie 失效 / 跳登录页 / `visible:false` —— 归责账号。
/// - [`AppError::BusinessReject`]：业务 `errno != 0` 的受限语义 —— 归责账号。
/// - [`AppError::Http`]：兼容兜底，仅用于无法明确归因的 HTTP 错误，
///   新增代码请优先选用上述更精细的变体。
#[derive(Debug)]
pub enum AppError {
    Db(rusqlite::Error),
    NotFound(String),
    Internal(String),

    Network(String),
    HttpStatus { code: u16, body_excerpt: String },
    LoginRequired(String),
    BusinessReject { errno: Option<i64>, msg: String },

    Http(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Db(e) => write!(f, "Database error: {e}"),
            AppError::NotFound(msg) => write!(f, "Not found: {msg}"),
            AppError::Internal(msg) => write!(f, "Internal error: {msg}"),
            AppError::Network(msg) => write!(f, "Network error: {msg}"),
            AppError::HttpStatus { code, body_excerpt } => {
                if body_excerpt.is_empty() {
                    write!(f, "HTTP {code}")
                } else {
                    write!(f, "HTTP {code}: {body_excerpt}")
                }
            }
            AppError::LoginRequired(msg) => write!(f, "Login required: {msg}"),
            AppError::BusinessReject { errno, msg } => match errno {
                Some(n) => write!(f, "Business reject (errno={n}): {msg}"),
                None => write!(f, "Business reject: {msg}"),
            },
            AppError::Http(msg) => write!(f, "HTTP error: {msg}"),
        }
    }
}

impl AppError {
    /// 面向用户的简短摘要（一行），用于 `crawl-progress` 日志、Tauri 事件等
    /// 暴露给前端的文本：**不**包含 HTML 响应体、长堆栈，只保留错误类型 + 关键字段。
    /// DB 的 `crawl_requests.error_message` 仍写完整 [`Display`] 文本，方便排查。
    pub fn summary(&self) -> String {
        const MAX_DETAIL: usize = 80;
        // Network 错误一般展开了 source() 链（"... → tls handshake → eof"），
        // 给更宽的预算才能让 root cause 不被截掉，否则前端只能看到一半提示。
        const MAX_NETWORK_DETAIL: usize = 240;
        match self {
            AppError::Db(_) => "数据库错误".into(),
            AppError::NotFound(s) => format!("未找到: {}", brief(s, MAX_DETAIL)),
            AppError::Internal(s) => format!("内部错误: {}", brief(s, MAX_DETAIL)),
            AppError::Network(s) => format!("网络错误: {}", brief(s, MAX_NETWORK_DETAIL)),
            AppError::HttpStatus { code, .. } => match *code {
                // 414 在 Weibo 边缘是 IP 限流的伪装码，不是真的 URI Too Long。
                // 见 queue/risk.rs 的 attribute() 注释。
                414 => "HTTP 414（疑似当前出口 IP 被限流，建议挂代理 / 降并发 / 暂停后再试）".into(),
                429 => "HTTP 429（被限流，建议降并发或更换代理）".into(),
                _ => format!("HTTP {code}"),
            },
            AppError::LoginRequired(_) => "登录失效（疑似跳登录页 / Cookie 失效）".into(),
            AppError::BusinessReject { errno, msg } => {
                let m = brief(msg, MAX_DETAIL);
                match errno {
                    Some(n) => format!("业务受限 errno={n}: {m}"),
                    None => format!("业务受限: {m}"),
                }
            }
            AppError::Http(s) => format!("HTTP 错误: {}", brief(s, MAX_DETAIL)),
        }
    }
}

/// 折成一行 + 截断到 `max` 个字符，超出加省略号。用于把 HTML / 多行错误压成可读摘要。
fn brief(s: &str, max: usize) -> String {
    let mut one_line = String::with_capacity(s.len().min(max + 8));
    let mut prev_space = false;
    for c in s.chars() {
        let c = if c.is_whitespace() { ' ' } else { c };
        if c == ' ' && prev_space {
            continue;
        }
        prev_space = c == ' ';
        one_line.push(c);
    }
    let trimmed = one_line.trim();
    let count = trimmed.chars().count();
    if count > max {
        let head: String = trimmed.chars().take(max).collect();
        format!("{head}…")
    } else {
        trimmed.to_string()
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
