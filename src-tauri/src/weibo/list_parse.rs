//! 对齐 WeiBoCrawler `parse/parse_list_html.py`：字段名与解析逻辑尽量一致。
//! 仍可在无 `div.m-page` 时解析（兼容改版页）；条目选择器含 `feed_list_item` / `card-wrap[mid]`。

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Timelike};
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::{json, Value};

/// 对齐 `WeiBoCrawler/util/process.py` `process_time_str`。
fn process_weibo_time_str(time_str: &str) -> Option<String> {
    let time_str = time_str.trim();
    if time_str.is_empty() {
        return None;
    }
    let re_year = Regex::new(r"(\d{4})年").ok()?;
    let re_month = Regex::new(r"(\d{1,2})月").ok()?;
    let re_day = Regex::new(r"(\d{1,2})日").ok()?;
    let re_hm = Regex::new(r"(\d{1,2}):(\d{1,2})").ok()?;
    let now = Local::now();
    let year = re_year
        .captures(time_str)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok())
        .unwrap_or_else(|| now.year());
    let month = re_month
        .captures(time_str)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .unwrap_or_else(|| now.month());
    let day = re_day
        .captures(time_str)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .unwrap_or_else(|| now.day());
    let (hour, minute) = if let Some(c) = re_hm.captures(time_str) {
        let h: u32 = c.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(now.hour());
        let mi: u32 = c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(now.minute());
        (h, mi)
    } else {
        (now.hour(), now.minute())
    };
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let mut dt: NaiveDateTime = date.and_hms_opt(hour, minute, 0)?;

    if let Ok(re_min) = Regex::new(r"(\d+)分钟前") {
        if let Some(c) = re_min.captures(time_str) {
            if let Ok(m) = c.get(1).map(|x| x.as_str()).unwrap_or("0").parse::<i64>() {
                dt = dt.checked_sub_signed(Duration::minutes(m))?;
            }
        }
    }
    if let Ok(re_h) = Regex::new(r"(\d+)小时前") {
        if let Some(c) = re_h.captures(time_str) {
            if let Ok(h) = c.get(1).map(|x| x.as_str()).unwrap_or("0").parse::<i64>() {
                dt = dt.checked_sub_signed(Duration::hours(h))?;
            }
        }
    }
    Some(dt.format("%Y-%m-%d %H:%M:%S").to_string())
}

fn first_int_in_text(s: &str) -> Option<i64> {
    Regex::new(r"\d+")
        .ok()?
        .find(s)
        .and_then(|m| m.as_str().parse().ok())
}

/// 与 WeiBoCrawler `get_uid`：`re.search(r"/(\d+)/?", href)`（数字后不一定还有 `/`）。
fn uid_from_href_python_style(href: &str) -> Option<String> {
    if let Some(c) = Regex::new(r"/u/(\d+)(?:/|$|\?|#|&)").ok()?.captures(href) {
        return c.get(1).map(|m| m.as_str().to_string());
    }
    Regex::new(r"/(\d{5,})(?:/|$|\?|#|&)").ok()?.captures(href).and_then(|c| {
        c.get(1).map(|m| m.as_str().to_string())
    })
}

/// `from` 区正文链接中的 mblogid（路径 `.../uid/mblogid?...`）；否则回退为 Python 的 `/(\w+)\?`。
fn mblogid_from_status_href(href: &str, mblog_re: &Regex) -> Option<String> {
    if let Some(c) = Regex::new(r"/\d+/([A-Za-z0-9]+)(?:\?|$|#|&)").ok()?.captures(href) {
        return c.get(1).map(|m| m.as_str().to_string());
    }
    mblog_re
        .captures(href)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

fn normalize_href(href: &str) -> String {
    let t = href.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        t.to_string()
    } else if t.starts_with("//") {
        format!("https:{t}")
    } else {
        format!("https://{t}")
    }
}

fn normalize_feed_text(raw_full: &str, raw_show: &str) -> String {
    let re_indent = Regex::new(r"\n[ \t]+").expect("regex");
    let re_shouqi = Regex::new(r"[ \t]*收起d[ \t]*").expect("regex");
    let re_block_nl = Regex::new(r"[ \t]*\n+[ \t]*").expect("regex");

    let mut ca = re_indent.replace_all(raw_full, "\n").to_string();
    let mut cs = re_indent.replace_all(raw_show, "\n").to_string();
    ca = re_shouqi.replace_all(&ca, "").to_string();
    cs = re_shouqi.replace_all(&cs, "").to_string();

    let mut content_final = if ca.trim().is_empty() {
        cs
    } else {
        ca
    };
    content_final = content_final.replace('\u{200b}', "");
    content_final = content_final.trim().to_string();
    re_block_nl.replace_all(&content_final, "\n\n").trim().to_string()
}

fn text_or_empty(div: &scraper::ElementRef<'_>, sel: &Selector) -> String {
    div.select(sel)
        .next()
        .map(|e| e.text().collect::<Vec<_>>().join("").trim().to_string())
        .unwrap_or_default()
}

fn parse_one_item(
    div: scraper::ElementRef<'_>,
    a_nick: &Selector,
    a_weibo_fallback: &Selector,
    from_links: &Selector,
    p_full: &Selector,
    p_short: &Selector,
    card_lis: Option<&Selector>,
    mblog_re: &Regex,
) -> Value {
    let mid = div
        .value()
        .attr("mid")
        .map(std::string::ToString::to_string);

    let mut uid = None;
    let mut personal_name = None;
    let mut personal_href = None;
    if let Some(a) = div.select(a_nick).next() {
        personal_name = a
            .value()
            .attr("nick-name")
            .or_else(|| a.value().attr("nick"))
            .map(str::to_string);
        if let Some(href) = a.value().attr("href") {
            personal_href = Some(normalize_href(href));
            uid = uid_from_href_python_style(href);
        }
    }
    if uid.is_none() {
        for a in div.select(a_weibo_fallback) {
            if let Some(href) = a.value().attr("href") {
                if let Some(u) = uid_from_href_python_style(href) {
                    uid = Some(u);
                    break;
                }
            }
        }
    }

    let mut mblogid = None;
    let mut weibo_href = None;
    let mut publish_time: Option<String> = None;
    let mut content_from: Option<String> = None;
    let links: Vec<_> = div.select(from_links).collect();
    if let Some(a) = links.first() {
        if let Some(href) = a.value().attr("href") {
            weibo_href = Some(normalize_href(href));
            mblogid = mblogid_from_status_href(href, mblog_re);
        }
        let t = a.text().collect::<Vec<_>>().join("").trim().to_string();
        if !t.is_empty() {
            publish_time = process_weibo_time_str(&t).or(Some(t));
        }
    }
    if let Some(a) = links.get(1) {
        let t = a.text().collect::<Vec<_>>().join("").trim().to_string();
        if !t.is_empty() {
            content_from = Some(t);
        }
    }
    if uid.is_none() {
        if let Some(ref h) = weibo_href {
            uid = uid_from_href_python_style(h);
        }
    }

    let raw_full = text_or_empty(&div, p_full);
    let raw_show = text_or_empty(&div, p_short);
    let content_all = normalize_feed_text(&raw_full, &raw_show);

    let mut retweet_num: Option<i64> = None;
    let mut comment_num: Option<i64> = None;
    let mut star_num: Option<i64> = None;
    let lis: Vec<_> = card_lis
        .map(|sel| div.select(sel).take(3).collect())
        .unwrap_or_default();
    if let Some(li) = lis.first() {
        let s = li.text().collect::<Vec<_>>().join("");
        retweet_num = first_int_in_text(&s);
    }
    if let Some(li) = lis.get(1) {
        let s = li.text().collect::<Vec<_>>().join("");
        comment_num = first_int_in_text(&s);
    }
    if let Some(li) = lis.get(2) {
        let s = li.text().collect::<Vec<_>>().join("");
        star_num = first_int_in_text(&s);
    }

    json!({
        "mid": mid,
        "uid": uid,
        "mblogid": mblogid,
        "personal_name": personal_name,
        "personal_href": personal_href,
        "weibo_href": weibo_href,
        "publish_time": publish_time,
        "content_from": content_from,
        "content_all": content_all,
        "retweet_num": retweet_num.unwrap_or(0),
        "comment_num": comment_num.unwrap_or(0),
        "star_num": star_num.unwrap_or(0),
    })
}

/// 从搜索列表 HTML 抽取条目（字段对齐 Python `parse_list_html` 返回的 dict）。
pub fn parse_list_html(html: &str) -> Vec<Value> {
    let doc = Html::parse_document(html);
    let a_nick = match Selector::parse(r#"a[nick-name]"#) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let a_weibo_fallback = match Selector::parse(r#"a[href*="weibo"]"#) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let from_links = match Selector::parse(r#"div.from a"#) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let p_full = match Selector::parse(r#"p[node-type="feed_list_content_full"]"#) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let p_short = match Selector::parse(r#"p[node-type="feed_list_content"]"#) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let card_lis = Selector::parse(r#"div[class*="card-act"] ul li"#).ok();

    let mblog_re = Regex::new(r"/(\w+)\?").expect("regex");

    let scoped = Selector::parse(r#"#pl_feedlist_index div[action-type="feed_list_item"]"#);
    let global = Selector::parse(r#"div[action-type="feed_list_item"]"#);
    let card_wrap = Selector::parse(r#"div.card-wrap[mid]"#);

    let mut item_selectors: Vec<Selector> = Vec::new();
    match (&scoped, &global) {
        (Ok(s), Ok(g)) => {
            if doc.select(s).next().is_some() {
                item_selectors.push(s.clone());
            } else {
                item_selectors.push(g.clone());
            }
        }
        (Ok(s), Err(_)) => item_selectors.push(s.clone()),
        (Err(_), Ok(g)) => item_selectors.push(g.clone()),
        (Err(_), Err(_)) => {}
    }
    if let Ok(c) = &card_wrap {
        item_selectors.push(c.clone());
    }

    let mut out = Vec::new();
    'outer: for item_sel in &item_selectors {
        for div in doc.select(item_sel) {
            out.push(parse_one_item(
                div,
                &a_nick,
                &a_weibo_fallback,
                &from_links,
                &p_full,
                &p_short,
                card_lis.as_ref(),
                &mblog_re,
            ));
        }
        if !out.is_empty() {
            break 'outer;
        }
        out.clear();
    }
    out
}
