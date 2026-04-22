//! IP 地理信息反查（ip-api.com）。
//!
//! 设计取舍：
//! - **数据源**：`http://ip-api.com/json/{ip}?lang=zh-CN&fields=...`
//!   - 免费、不需 API Key、≤45 req/min/IP（足够前端按钮触发）；
//!   - 中文城市 / 省份名直接返回，UI 无需自己维护翻译表；
//!   - 仅 HTTP（HTTPS 收费）。本接口只读公开数据，无敏感信息。
//!   - 不传 IP 段（即 `http://ip-api.com/json/`）会反查**请求方自己**，省一次 ipify。
//! - **目标 IP**：解析用户填的 `address` 字段中的 host：
//!   - 形如 `socks5://user:pass@1.2.3.4:1080` → `1.2.3.4`；
//!   - 形如 `1.2.3.4:1080` / `[::1]:1080` → `1.2.3.4` / `::1`；
//!   - 域名（如 `proxy.example.com:1080`）：先 DNS 解析到 IP，再反查；
//!   - 系统内置 `local-direct` 行：让 ip-api 直接反查请求方 IP（一跳）。
//! - **超时**：单次 5 秒；失败返回 `None`，不抛错——前端只需展示「—」。
//!   与 `proxy_service::PROBE_TIMEOUT_EACH` 对齐——add_proxy 时三件事
//!   （geo / cn / intl）并行做完，最长那条决定总耗时，统一上限便于估算。
//! - **不走代理**：直接用本机出口请求 ip-api.com，避免代理本身挂掉时反查也跟着挂。
//! - **必须在独立 OS 线程内执行**：`reqwest::blocking::Client` 自带一个 tokio 运行时；
//!   如果它在 Tauri 的 async 命令（也跑在 tokio 上）里被 drop，会触发
//!   `Cannot drop a runtime in a context where blocking is not allowed` 的 panic。
//!   `lookup` 因此用 `std::thread::spawn` 把整次反查丢到普通线程上跑、再 `join`
//!   回来——与 `proxy_service::probe_one` 走 `std::thread::spawn` 的理由相同。

use std::net::{IpAddr, ToSocketAddrs};
use std::time::Duration;

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::model::proxy::{ProxyGeoInfo, ProxyIp, ProxyType};

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
/// ip-api.com 的查询端点。`/{ip}` 段为空时反查请求方自身（关键：本机直连一跳搞定）。
const IP_API_BASE: &str = "http://ip-api.com/json/";
const IP_API_FIELDS: &str = "status,message,country,regionName,city,isp,query";

/// 反查给定代理的实际地理位置 / ISP。失败时返回 `None`，调用方应继续保留旧值或写空。
///
/// **务必从普通同步上下文调用**——内部已经把所有 reqwest 工作迁到独立 OS 线程，
/// 所以即便外层是 `#[tauri::command] pub async fn`，链路上也不会出现「在 tokio
/// 运行时里 drop 另一个 tokio 运行时」的 panic。
pub fn lookup(proxy: &ProxyIp) -> Option<ProxyGeoInfo> {
    // clone 出独立可发到子线程的副本——`ProxyIp` 不是 Copy，但已 Clone。
    let p = proxy.clone();
    // 子线程 panic 时返回 None，跟 HTTP 失败的处理保持一致（不抛错给前端）。
    std::thread::spawn(move || lookup_blocking(&p))
        .join()
        .unwrap_or_else(|panic_payload| {
            log::warn!("[geoip] lookup worker panicked: {panic_payload:?}");
            None
        })
}

/// 真正的同步反查实现：解析 host → 取目标 IP → 调 ip-api.com。
/// 该函数体内才会构造 / drop `reqwest::blocking::Client`。
///
/// **必须保证调用线程不是 tokio worker**。如果调用方已经身处普通 OS 线程
/// （例如 `proxy_service::probe_one_full` 里给 geo 单独分配的 `std::thread::spawn`），
/// 直接调本函数即可，省一次 `std::thread::spawn` + `join`。其余场景请用
/// 高层 [`lookup`]。
pub fn lookup_blocking(proxy: &ProxyIp) -> Option<ProxyGeoInfo> {
    // 本机直连：一跳到 ip-api，让它反查请求方自己，省掉 ipify 这条易挂的链路。
    if matches!(proxy.proxy_type, ProxyType::Direct) {
        return fetch_geo(None);
    }

    // 其它代理：先把 address 解析成 IP，再查 geo。
    let host = parse_host_from_address(&proxy.address)?;
    let ip = if host.parse::<IpAddr>().is_ok() {
        host
    } else {
        // 域名：解析到首个 IP（端口随便填，只为了走 ToSocketAddrs）
        let probe = format!("{host}:80");
        match probe.to_socket_addrs() {
            Ok(mut iter) => match iter.next() {
                Some(addr) => addr.ip().to_string(),
                None => {
                    log::warn!("[geoip] dns resolve {host} returned 0 records");
                    return None;
                }
            },
            Err(err) => {
                log::warn!("[geoip] dns resolve {host} failed: {err}");
                return None;
            }
        }
    };
    fetch_geo(Some(&ip))
}

/// 解析用户填的代理地址 → host。支持：
/// - `scheme://user:pass@host:port` / `scheme://host:port`
/// - `host:port`
/// - `[ipv6]:port`
/// - 纯 host
pub(crate) fn parse_host_from_address(address: &str) -> Option<String> {
    let s = address.trim();
    if s.is_empty() {
        return None;
    }
    // 去掉 scheme
    let s = match s.split_once("://") {
        Some((_, rest)) => rest,
        None => s,
    };
    // 去掉 user:pass@
    let s = match s.rsplit_once('@') {
        Some((_, host_port)) => host_port,
        None => s,
    };
    // 去掉 path / query
    let s = s.split('/').next().unwrap_or(s);
    let s = s.split('?').next().unwrap_or(s);

    // [ipv6]:port
    if let Some(rest) = s.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return Some(rest[..end].to_string());
        }
    }
    // host:port → 取 host
    let host = s.rsplit_once(':').map(|(h, _)| h).unwrap_or(s);
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct IpApiResponse {
    status: String,
    #[serde(default)]
    country: Option<String>,
    #[serde(default, rename = "regionName")]
    region_name: Option<String>,
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    isp: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// 反查指定 IP 的地理信息。`ip = None` 表示让 ip-api 反查请求方自己（用于本机直连）。
///
/// 失败原因（HTTP 错、JSON 解析错、ip-api 业务 fail）都会打到 stderr，便于排查；
/// 对前端依然只暴露 `None`，由调用方决定是否给 toast 提示。
fn fetch_geo(ip: Option<&str>) -> Option<ProxyGeoInfo> {
    // 内网 / 回环不走外网反查，直接给个稳定标签。
    if let Some(ip_str) = ip {
        if let Ok(addr) = ip_str.parse::<IpAddr>() {
            if is_private_or_local(addr) {
                return Some(ProxyGeoInfo {
                    country: Some("内网".to_string()),
                    region: None,
                    city: None,
                    isp: Some("局域网 / 回环".to_string()),
                    ip: Some(ip_str.to_string()),
                });
            }
        }
    }

    let label = ip.unwrap_or("self");
    let client = match Client::builder().timeout(LOOKUP_TIMEOUT).build() {
        Ok(c) => c,
        Err(err) => {
            log::warn!("[geoip] build reqwest client failed: {err}");
            return None;
        }
    };
    let url = match ip {
        Some(ip_str) => format!("{IP_API_BASE}{ip_str}?lang=zh-CN&fields={IP_API_FIELDS}"),
        None => format!("{IP_API_BASE}?lang=zh-CN&fields={IP_API_FIELDS}"),
    };
    let resp = match client.get(&url).send() {
        Ok(r) => r,
        Err(err) => {
            log::warn!("[geoip] request {label} failed: {err}");
            return None;
        }
    };
    if !resp.status().is_success() {
        log::warn!("[geoip] lookup {label} HTTP {}", resp.status());
        return None;
    }
    let body: IpApiResponse = match resp.json() {
        Ok(b) => b,
        Err(err) => {
            log::warn!("[geoip] parse response of {label} failed: {err}");
            return None;
        }
    };
    if body.status != "success" {
        log::warn!(
            "[geoip] lookup {label} status={} message={:?}",
            body.status, body.message
        );
        return None;
    }
    let resolved_ip = body
        .query
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| ip.map(|s| s.to_string()));
    Some(ProxyGeoInfo {
        country: body.country.filter(|s| !s.is_empty()),
        region: body.region_name.filter(|s| !s.is_empty()),
        city: body.city.filter(|s| !s.is_empty()),
        isp: body.isp.filter(|s| !s.is_empty()),
        ip: resolved_ip,
    })
}

fn is_private_or_local(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_handles_common_shapes() {
        assert_eq!(
            parse_host_from_address("socks5://u:p@1.2.3.4:1080").as_deref(),
            Some("1.2.3.4")
        );
        assert_eq!(
            parse_host_from_address("http://1.2.3.4:8080").as_deref(),
            Some("1.2.3.4")
        );
        assert_eq!(
            parse_host_from_address("1.2.3.4:1080").as_deref(),
            Some("1.2.3.4")
        );
        assert_eq!(
            parse_host_from_address("proxy.example.com:1080").as_deref(),
            Some("proxy.example.com")
        );
        assert_eq!(
            parse_host_from_address("[2001:db8::1]:1080").as_deref(),
            Some("2001:db8::1")
        );
        assert_eq!(
            parse_host_from_address("socks5://user:p@ss@host:1080").as_deref(),
            Some("host")
        );
        assert_eq!(parse_host_from_address("").as_deref(), None);
    }
}
