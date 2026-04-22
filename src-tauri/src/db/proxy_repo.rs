use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::model::proxy::{ProxyGeoInfo, ProxyIp, ProxyType};

use super::{enum_to_str, str_to_enum};

// v7：proxies 通过 `migrate_proxies_stack_latency_and_merge_probed_at` 把
// `proxy_latency_probes` 行转列搬入，并把 `geo_updated_at` 与 `probed_at`
// 合并为 `last_probed_at`。当前列序：基础元数据 → geo* → 双探针 ms → 时间戳。
const SELECT_COLUMNS: &str = "id, address, proxy_type, remark, is_system, \
     geo_country, geo_region, geo_city, geo_isp, geo_ip, \
     cn_latency_ms, intl_latency_ms, last_probed_at, global_probe_ok";

pub fn list(conn: &Connection) -> Result<Vec<ProxyIp>, AppError> {
    // is_system 排第一保证本机直连固定置顶；后续按 id 字典序，避免每次刷新顺序抖动。
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM proxies ORDER BY is_system DESC, id ASC",
    ))?;

    let rows = stmt.query_map([], row_to_raw)?;

    rows.map(|r| {
        let raw = r?;
        raw.into_model()
    })
    .collect()
}

/// 按代理地址精确匹配（账号 `bound_ip` 存的是代理地址或本机 IP）。
#[allow(dead_code)]
pub fn get_by_address(conn: &Connection, address: &str) -> Result<Option<ProxyIp>, AppError> {
    let raw = conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM proxies WHERE address = ?1"),
        params![address],
        row_to_raw,
    );
    match raw {
        Ok(r) => Ok(Some(r.into_model()?)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<ProxyIp, AppError> {
    let raw = conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM proxies WHERE id = ?1"),
        params![id],
        row_to_raw,
    )?;
    raw.into_model()
}

pub fn insert(conn: &Connection, proxy: &ProxyIp) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO proxies (id, address, proxy_type, remark, is_system, \
         geo_country, geo_region, geo_city, geo_isp, geo_ip, \
         cn_latency_ms, intl_latency_ms, last_probed_at, global_probe_ok) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            proxy.id,
            proxy.address,
            enum_to_str(&proxy.proxy_type),
            proxy.remark,
            proxy.is_system as i64,
            proxy.geo_country,
            proxy.geo_region,
            proxy.geo_city,
            proxy.geo_isp,
            proxy.geo_ip,
            proxy.cn_latency_ms,
            proxy.intl_latency_ms,
            proxy.last_probed_at,
            proxy.global_probe_ok as i64,
        ],
    )?;
    Ok(())
}

/// 一次性写回 geo + 双探针 + 时间戳。**目前**所有写入路径
/// （add_proxy / update_proxy(address 变) / check_all_proxies_dual_health）
/// 都是把 geo / cn / intl 三件事并行做完后再统一落盘，所以只需要这一个写函数。
///
/// `info = None` 表示反查失败：此时 5 列 geo* 全部清空，但 `last_probed_at`
/// 仍然刷新（前端能展示「最近一次尝试时间」）。
/// `cn_ms / intl_ms = None` 表示对应探针未跑（本次不更新该列）。
/// 当**两者均为 Some** 时，若国内、国外均为失败哨兵（负数）则 `global_probe_ok = 0`，否则为 1；
/// 若任一为 None，则 `global_probe_ok` 列保持不变。
pub fn update_geo_and_latency(
    conn: &Connection,
    id: &str,
    info: Option<&ProxyGeoInfo>,
    cn_ms: Option<i64>,
    intl_ms: Option<i64>,
    probed_at: &str,
) -> Result<(), AppError> {
    let global_probe_ok: Option<i64> = match (cn_ms, intl_ms) {
        (Some(c), Some(i)) => Some(if c < 0 && i < 0 { 0 } else { 1 }),
        _ => None,
    };
    // 用 COALESCE(?, col) 让 None 表示「保留旧值」，Some 表示「写入新值」。
    // geo* 五列不走 COALESCE：`info = None` 时业务语义就是要清空。
    let n = conn.execute(
        "UPDATE proxies SET \
            geo_country     = ?2, \
            geo_region      = ?3, \
            geo_city        = ?4, \
            geo_isp         = ?5, \
            geo_ip          = ?6, \
            cn_latency_ms   = COALESCE(?7, cn_latency_ms), \
            intl_latency_ms = COALESCE(?8, intl_latency_ms), \
            last_probed_at  = ?9, \
            global_probe_ok = COALESCE(?10, global_probe_ok) \
         WHERE id = ?1",
        params![
            id,
            info.and_then(|i| i.country.as_deref()),
            info.and_then(|i| i.region.as_deref()),
            info.and_then(|i| i.city.as_deref()),
            info.and_then(|i| i.isp.as_deref()),
            info.and_then(|i| i.ip.as_deref()),
            cn_ms,
            intl_ms,
            probed_at,
            global_probe_ok,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("proxy {id}")));
    }
    Ok(())
}

/// 编辑代理：仅更新 `address / proxy_type / remark` 三个用户可改字段，
/// **不**触碰 `is_system / geo_*` 等运行期数据。
/// 系统行（`is_system != 0`）在此被强制拦截：与 [`delete`] 保持一致的护栏。
pub fn update(
    conn: &Connection,
    id: &str,
    address: &str,
    proxy_type: &ProxyType,
    remark: Option<&str>,
) -> Result<(), AppError> {
    let is_system: i64 = conn
        .query_row(
            "SELECT is_system FROM proxies WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("proxy {id}")),
            other => other.into(),
        })?;
    if is_system != 0 {
        return Err(AppError::Internal(format!(
            "代理 {id} 是系统内置行，不允许修改"
        )));
    }
    let n = conn.execute(
        "UPDATE proxies SET address = ?2, proxy_type = ?3, remark = ?4 WHERE id = ?1",
        params![id, address, enum_to_str(proxy_type), remark],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("proxy {id}")));
    }
    Ok(())
}

/// 删除前自动拦截系统行（`local-direct` 等），调用方拿到 `Forbidden` 错误时
/// 应该向前端反馈而不是吞掉。
pub fn delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    let is_system: i64 = conn
        .query_row(
            "SELECT is_system FROM proxies WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if is_system != 0 {
        return Err(AppError::Internal(format!(
            "代理 {id} 是系统内置行，不允许删除"
        )));
    }
    conn.execute("DELETE FROM proxies WHERE id = ?1", params![id])?;
    Ok(())
}

struct RawProxy {
    id: String,
    address: String,
    proxy_type: String,
    remark: Option<String>,
    is_system: i64,
    geo_country: Option<String>,
    geo_region: Option<String>,
    geo_city: Option<String>,
    geo_isp: Option<String>,
    geo_ip: Option<String>,
    cn_latency_ms: Option<i64>,
    intl_latency_ms: Option<i64>,
    last_probed_at: Option<String>,
    global_probe_ok: i64,
}

fn row_to_raw(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawProxy> {
    Ok(RawProxy {
        id: row.get(0)?,
        address: row.get(1)?,
        proxy_type: row.get(2)?,
        remark: row.get(3)?,
        is_system: row.get(4).unwrap_or(0),
        geo_country: row.get(5).ok(),
        geo_region: row.get(6).ok(),
        geo_city: row.get(7).ok(),
        geo_isp: row.get(8).ok(),
        geo_ip: row.get(9).ok(),
        cn_latency_ms: row.get(10).ok(),
        intl_latency_ms: row.get(11).ok(),
        last_probed_at: row.get(12).ok(),
        global_probe_ok: row.get(13).unwrap_or(1),
    })
}

impl RawProxy {
    fn into_model(self) -> Result<ProxyIp, AppError> {
        Ok(ProxyIp {
            id: self.id,
            address: self.address,
            proxy_type: str_to_enum::<ProxyType>(&self.proxy_type)?,
            remark: self.remark,
            is_system: self.is_system != 0,
            geo_country: self.geo_country,
            geo_region: self.geo_region,
            geo_city: self.geo_city,
            geo_isp: self.geo_isp,
            geo_ip: self.geo_ip,
            cn_latency_ms: self.cn_latency_ms,
            intl_latency_ms: self.intl_latency_ms,
            last_probed_at: self.last_probed_at,
            global_probe_ok: self.global_probe_ok != 0,
        })
    }
}
