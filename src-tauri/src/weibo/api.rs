//! 与 WeiBoCrawler `request/get_list_request.py`、`get_body_request.py`、`get_comment_request.py` 对齐的 URL / Query / Header 构造。
//!
//! **列表采集**（`get_list_request.build_list_params`）：使用 `request.toml` 的 **`[body_headers]`**（`request_headers.body_headers`），
//! 首页带其中默认 `referer`；`page > 1` 时仅将 `referer` 设为上一页完整 URL。另保留 `[list_headers]` 风格的导航头作备选。

use reqwest::Url;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

/// 与 `request/request.toml` `[body_headers]` 中 `referer` 一致，供列表首页使用（见 `get_list_request.py`）。
pub const WEIBO_LIST_BODY_DEFAULT_REFERER: &str =
    "https://weibo.com/1644114654/OiZre8dir?refer_flag=1001030103_";

/// `[body_headers]` 全文，**不含** `referer`、`x-xsrf-token` / `x-csrf-token`（登录后由 `merge_weibo_stored_cookies` 注入）。
fn body_style_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    let insert = |m: &mut HeaderMap, k: &str, v: &str| {
        if let (Ok(name), Ok(val)) = (
            HeaderName::try_from(k),
            HeaderValue::try_from(v),
        ) {
            m.insert(name, val);
        }
    };
    insert(
        &mut h,
        "accept",
        "application/json, text/plain, */*",
    );
    insert(
        &mut h,
        "accept-language",
        "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6",
    );
    insert(&mut h, "client-version", "v2.47.25");
    insert(&mut h, "priority", "u=1, i");
    insert(
        &mut h,
        "sec-ch-ua",
        r#""Not A(Brand";v="8", "Chromium";v="132", "Microsoft Edge";v="132"#,
    );
    insert(&mut h, "sec-ch-ua-mobile", "?0");
    insert(&mut h, "sec-ch-ua-platform", r#""Windows""#);
    insert(&mut h, "sec-fetch-dest", "empty");
    insert(&mut h, "sec-fetch-mode", "cors");
    insert(&mut h, "sec-fetch-site", "same-origin");
    insert(&mut h, "server-version", "v2025.01.23.1");
    insert(
        &mut h,
        "user-agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0",
    );
    insert(
        &mut h,
        "x-requested-with",
        "XMLHttpRequest",
    );
    h
}

/// 搜索列表页 GET：与 `request.toml` `[list_headers]` 一致（文档导航）。
/// `sec_fetch_site` 取 `none` / `same-site` / `same-origin`，需与 `referer` 场景一致（子域跳转用 `same-site`）。
fn list_navigation_headers(referer: Option<&str>, sec_fetch_site: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    let insert = |m: &mut HeaderMap, k: &str, v: &str| {
        if let (Ok(name), Ok(val)) = (
            HeaderName::try_from(k),
            HeaderValue::try_from(v),
        ) {
            m.insert(name, val);
        }
    };
    insert(
        &mut h,
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
    );
    insert(
        &mut h,
        "accept-language",
        "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6",
    );
    insert(&mut h, "priority", "u=0, i");
    insert(
        &mut h,
        "sec-ch-ua",
        r#""Not A(Brand";v="8", "Chromium";v="132", "Microsoft Edge";v="132"#,
    );
    insert(&mut h, "sec-ch-ua-mobile", "?0");
    insert(&mut h, "sec-ch-ua-platform", r#""Windows""#);
    insert(&mut h, "sec-fetch-dest", "document");
    insert(&mut h, "sec-fetch-mode", "navigate");
    insert(&mut h, "sec-fetch-site", sec_fetch_site);
    insert(&mut h, "sec-fetch-user", "?1");
    insert(&mut h, "upgrade-insecure-requests", "1");
    insert(
        &mut h,
        "user-agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0",
    );
    if let Some(r) = referer {
        if let Ok(v) = HeaderValue::try_from(r.to_string()) {
            h.insert(HeaderName::from_static("referer"), v);
        }
    }
    h
}

/// 首页：从 `weibo.com` 进入搜索子域（`Sec-Fetch-Site: same-site`）。
pub fn list_headers_page1_from_weibo() -> HeaderMap {
    list_navigation_headers(Some("https://weibo.com/"), "same-site")
}

/// 首页：无 referer（地址栏直达）。
pub fn list_headers_page1_direct() -> HeaderMap {
    list_navigation_headers(None, "none")
}

/// 翻页：上一页完整 URL 作为 referer。
pub fn list_headers_followup(prev_list_url: &str) -> HeaderMap {
    list_navigation_headers(Some(prev_list_url), "same-origin")
}

/// 列表：`body_headers` + 可选 `referer`（与 `build_list_params` 中 `headers` 逻辑一致）。
pub fn list_xhr_headers(referer: Option<&str>) -> HeaderMap {
    let mut h = body_style_headers();
    if let Some(r) = referer {
        if let Ok(v) = HeaderValue::try_from(r.to_string()) {
            h.insert(HeaderName::from_static("referer"), v);
        }
    }
    h
}

/// 列表第 1 页：`body_headers` + TOML 默认 `referer`（对齐 WeiBoCrawler `get_list_request`）。
pub fn list_headers_weibo_python_page1() -> HeaderMap {
    list_xhr_headers(Some(WEIBO_LIST_BODY_DEFAULT_REFERER))
}

fn timescope_part(t: Option<&str>) -> String {
    let Some(s) = t.map(str::trim).filter(|x| !x.is_empty()) else {
        return String::new();
    };
    if s.len() <= 10 && s.matches('-').count() == 2 {
        format!("{s}-0")
    } else {
        s.to_string()
    }
}

/// 构造列表页 GET URL（单页）。`page_index` 从 1 开始，与 Python 一致。
pub fn build_list_url(
    search_for: &str,
    page_index: i32,
    list_kind: &str,
    advanced_kind: Option<&str>,
    time_start: Option<&str>,
    time_end: Option<&str>,
) -> Result<Url, String> {
    if page_index < 1 {
        return Err("page_index 必须 >= 1".into());
    }

    let (base, pairs): (&str, Vec<(&str, String)>) = match list_kind {
        "综合" => (
            "https://s.weibo.com/weibo",
            vec![
                ("q", search_for.to_string()),
                ("Refer", "weibo_weibo".into()),
                ("page", page_index.to_string()),
            ],
        ),
        "实时" => (
            "https://s.weibo.com/realtime",
            vec![
                ("q", search_for.to_string()),
                ("rd", "realtime".into()),
                ("tw", "realtime".into()),
                ("Refer", "weibo_realtime".into()),
                ("page", page_index.to_string()),
            ],
        ),
        "高级" => {
            let mut p = vec![
                ("q", search_for.to_string()),
                ("suball", "1".into()),
                ("Refer", "g".into()),
                ("page", page_index.to_string()),
            ];
            let ak = advanced_kind.unwrap_or("综合");
            match ak {
                "热度" => p.push(("xsort", "hot".into())),
                "原创" => p.push(("scope", "ori".into())),
                _ => p.push(("typeall", "1".into())),
            }
            let ts = format!(
                "custom:{}:{}",
                timescope_part(time_start),
                timescope_part(time_end)
            );
            p.push(("timescope", ts));
            ("https://s.weibo.com/weibo", p)
        }
        _ => return Err(format!("未知的 list_kind: {list_kind}（应为 综合 / 实时 / 高级）")),
    };

    let mut url = Url::parse(base).map_err(|e| e.to_string())?;
    {
        let mut q = url.query_pairs_mut();
        for (k, v) in pairs {
            q.append_pair(k, &v);
        }
    }
    Ok(url)
}

/// 列表页：URL + 一组默认请求头（仅用于预览等；采集端会对首页做多策略重试）。
pub fn build_list_request(
    search_for: &str,
    page_index: i32,
    list_kind: &str,
    advanced_kind: Option<&str>,
    time_start: Option<&str>,
    time_end: Option<&str>,
) -> Result<(Url, HeaderMap), String> {
    let url = build_list_url(
        search_for,
        page_index,
        list_kind,
        advanced_kind,
        time_start,
        time_end,
    )?;
    let headers = if page_index > 1 {
        let prev = build_list_url(
            search_for,
            page_index - 1,
            list_kind,
            advanced_kind,
            time_start,
            time_end,
        )?;
        list_xhr_headers(Some(&prev.to_string()))
    } else {
        list_headers_weibo_python_page1()
    };
    Ok((url, headers))
}

/// `https://weibo.com/ajax/statuses/show`
pub fn build_body_request(status_id: &str) -> (Url, HeaderMap) {
    let mut url = Url::parse("https://weibo.com/ajax/statuses/show").unwrap();
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("id", status_id);
        q.append_pair("locale", "zh-CN");
        q.append_pair("isGetLongText", "true");
    }
    let mut h = body_style_headers();
    if let Ok(v) = HeaderValue::try_from(format!(
        "https://weibo.com/detail/{status_id}"
    )) {
        h.insert(HeaderName::from_static("referer"), v);
    }
    (url, h)
}

fn comment_style_headers(referer_path: &str) -> HeaderMap {
    let mut h = body_style_headers();
    if let Ok(v) = HeaderValue::try_from(format!("https://weibo.com{referer_path}")) {
        h.insert(HeaderName::from_static("referer"), v);
    }
    h.insert(HeaderName::from_static("x-requested-with"), HeaderValue::from_static("XMLHttpRequest"));
    h
}

/// 一级评论：`buildComments`，首次请求 `max_id` 为 `None`。
pub fn build_comment_l1_url(uid: &str, mid: &str, max_id: Option<&str>) -> Url {
    let mut url = Url::parse("https://weibo.com/ajax/statuses/buildComments").unwrap();
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("is_reload", "1");
        q.append_pair("id", mid);
        q.append_pair("is_show_bulletin", "2");
        q.append_pair("is_mix", "0");
        q.append_pair("count", "20");
        q.append_pair("uid", uid);
        q.append_pair("fetch_level", "0");
        q.append_pair("locale", "zh-CN");
        if let Some(m) = max_id {
            q.append_pair("flow", "0");
            q.append_pair("max_id", m);
        }
    }
    url
}

pub fn build_comment_l1_request(
    uid: &str,
    mid: &str,
    max_id: Option<&str>,
    referer_weibo_path: &str,
) -> (Url, HeaderMap) {
    let u = build_comment_l1_url(uid, mid, max_id);
    (u, comment_style_headers(referer_weibo_path))
}

/// 二级评论。
pub fn build_comment_l2_url(uid: &str, mid: &str, max_id: Option<&str>) -> Url {
    let mut url = Url::parse("https://weibo.com/ajax/statuses/buildComments").unwrap();
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("flow", "0");
        q.append_pair("is_reload", "1");
        q.append_pair("id", mid);
        q.append_pair("is_show_bulletin", "2");
        q.append_pair("is_mix", "1");
        q.append_pair("fetch_level", "1");
        q.append_pair("count", "20");
        q.append_pair("uid", uid);
        q.append_pair("locale", "zh-CN");
        if let Some(m) = max_id {
            q.append_pair("max_id", m);
        } else {
            q.append_pair("max_id", "0");
        }
    }
    url
}

pub fn build_comment_l2_request(
    uid: &str,
    mid: &str,
    max_id: Option<&str>,
    referer_weibo_path: &str,
) -> (Url, HeaderMap) {
    let u = build_comment_l2_url(uid, mid, max_id);
    (u, comment_style_headers(referer_weibo_path))
}
