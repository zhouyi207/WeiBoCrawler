use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::model::record::CrawledRecord;

use super::{enum_to_str, str_to_enum};

const SELECT_RECORD_COLS: &str = "SELECT id, platform, task_name, keyword, blog_id, content_preview, author, \
         crawled_at, json_data, parent_record_id, entity_type ";

pub fn query(
    conn: &Connection,
    platform: Option<&str>,
    keyword: Option<&str>,
) -> Result<Vec<CrawledRecord>, AppError> {
    let mut sql = String::from(
        "SELECT id, platform, task_name, keyword, blog_id, content_preview, author, \
         crawled_at, json_data, parent_record_id, entity_type \
         FROM records WHERE 1=1",
    );
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(p) = platform {
        sql.push_str(&format!(" AND platform = ?{idx}"));
        values.push(Box::new(p.to_string()));
        idx += 1;
    }

    if let Some(kw) = keyword {
        sql.push_str(&format!(
            " AND (keyword LIKE ?{idx} OR blog_id LIKE ?{idx} OR content_preview LIKE ?{idx} OR author LIKE ?{idx})"
        ));
        values.push(Box::new(format!("%{kw}%")));
        idx += 1;
    }

    let _ = idx;
    sql.push_str(" ORDER BY crawled_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), |row| {
        Ok(RawRecord {
            id: row.get(0)?,
            platform: row.get(1)?,
            task_name: row.get(2)?,
            keyword: row.get(3)?,
            blog_id: row.get(4)?,
            content_preview: row.get(5)?,
            author: row.get(6)?,
            crawled_at: row.get(7)?,
            json_data: row.get(8)?,
            parent_record_id: row.get(9)?,
            entity_type: row.get(10)?,
        })
    })?;

    rows.map(|r| {
        let raw = r?;
        raw.into_model()
    })
    .collect()
}

/// `records` 表中出现过的任务名（去重），用于数据管理筛选下拉框；不依赖 `tasks` 表。
pub fn list_distinct_task_names(
    conn: &Connection,
    platform: Option<&str>,
) -> Result<Vec<String>, AppError> {
    let mut sql = String::from(
        "SELECT DISTINCT task_name FROM records WHERE task_name != ''",
    );
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(p) = platform {
        sql.push_str(&format!(" AND platform = ?{idx}"));
        values.push(Box::new(p.to_string()));
        idx += 1;
    }
    let _ = idx;
    sql.push_str(" ORDER BY task_name COLLATE NOCASE");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), |row| {
        row.get::<_, String>(0)
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Build the shared WHERE clause (returns SQL fragment + bind values).
fn build_where_clause(
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut where_sql = String::from(" WHERE 1=1");
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(p) = platform {
        where_sql.push_str(&format!(" AND platform = ?{idx}"));
        values.push(Box::new(p.to_string()));
        idx += 1;
    }
    if let Some(tn) = task_name.filter(|s| !s.is_empty()) {
        where_sql.push_str(&format!(" AND task_name = ?{idx}"));
        values.push(Box::new(tn.to_string()));
        idx += 1;
    }
    if let Some(et) = entity_type.filter(|s| !s.is_empty()) {
        where_sql.push_str(&format!(" AND entity_type = ?{idx}"));
        values.push(Box::new(et.to_string()));
        idx += 1;
    }
    if let Some(kw) = keyword {
        where_sql.push_str(&format!(
            " AND (keyword LIKE ?{idx} OR blog_id LIKE ?{idx} OR content_preview LIKE ?{idx} OR author LIKE ?{idx})"
        ));
        values.push(Box::new(format!("%{kw}%")));
        idx += 1;
    }
    let _ = idx;
    (where_sql, values)
}

pub fn query_paged(
    conn: &Connection,
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
    offset: i64,
    limit: i64,
) -> Result<(Vec<CrawledRecord>, i64), AppError> {
    let (where_sql, values) = build_where_clause(platform, keyword, task_name, entity_type);

    let count_sql = format!("SELECT COUNT(*) FROM records{where_sql}");
    let total: i64 = conn.query_row(
        &count_sql,
        rusqlite::params_from_iter(values.iter()),
        |r| r.get(0),
    )?;

    let data_sql = format!(
        "{}FROM records{where_sql} ORDER BY crawled_at DESC LIMIT {limit} OFFSET {offset}",
        SELECT_RECORD_COLS
    );

    let mut stmt = conn.prepare(&data_sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), |row| {
        Ok(RawRecord {
            id: row.get(0)?,
            platform: row.get(1)?,
            task_name: row.get(2)?,
            keyword: row.get(3)?,
            blog_id: row.get(4)?,
            content_preview: row.get(5)?,
            author: row.get(6)?,
            crawled_at: row.get(7)?,
            json_data: row.get(8)?,
            parent_record_id: row.get(9)?,
            entity_type: row.get(10)?,
        })
    })?;

    let items: Vec<CrawledRecord> = rows
        .map(|r| {
            let raw = r?;
            raw.into_model()
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((items, total))
}

/// 与 [`query_paged`] 相同的筛选条件，返回全部匹配行（无分页），用于导出 / 按筛选去重。
pub fn query_filtered(
    conn: &Connection,
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
) -> Result<Vec<CrawledRecord>, AppError> {
    let (where_sql, values) = build_where_clause(platform, keyword, task_name, entity_type);
    let sql = format!(
        "{}FROM records{where_sql} ORDER BY crawled_at DESC",
        SELECT_RECORD_COLS
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), |row| {
        Ok(RawRecord {
            id: row.get(0)?,
            platform: row.get(1)?,
            task_name: row.get(2)?,
            keyword: row.get(3)?,
            blog_id: row.get(4)?,
            content_preview: row.get(5)?,
            author: row.get(6)?,
            crawled_at: row.get(7)?,
            json_data: row.get(8)?,
            parent_record_id: row.get(9)?,
            entity_type: row.get(10)?,
        })
    })?;

    rows.map(|r| {
        let raw = r?;
        raw.into_model()
    })
    .collect()
}

/// 仅在当前筛选结果内，按 `platform + keyword + blog_id + content_preview + author` 去重（保留 `rowid` 最小的一条）。
pub fn deduplicate_filtered(
    conn: &Connection,
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
) -> Result<u64, AppError> {
    let (where_sql, values) = build_where_clause(platform, keyword, task_name, entity_type);
    let delete_sql = format!(
        "DELETE FROM records WHERE rowid IN (
            SELECT rowid FROM (
                SELECT rowid,
                       ROW_NUMBER() OVER (
                           PARTITION BY platform, COALESCE(keyword, ''), COALESCE(blog_id, ''), content_preview, author
                           ORDER BY rowid ASC
                       ) AS rn
                FROM records{where_sql}
            ) WHERE rn > 1
        )"
    );
    let deleted = conn.execute(
        delete_sql.as_str(),
        rusqlite::params_from_iter(values.iter()),
    )?;
    Ok(deleted as u64)
}

/// 删除与 [`query_filtered`] 相同筛选条件下的所有行。
pub fn delete_filtered(
    conn: &Connection,
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
) -> Result<u64, AppError> {
    let (where_sql, values) = build_where_clause(platform, keyword, task_name, entity_type);
    let delete_sql = format!("DELETE FROM records{where_sql}");
    let n = conn.execute(
        delete_sql.as_str(),
        rusqlite::params_from_iter(values.iter()),
    )?;
    Ok(n as u64)
}

pub fn insert(conn: &Connection, record: &CrawledRecord) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO records (id, platform, task_name, keyword, blog_id, content_preview, \
         author, crawled_at, json_data, parent_record_id, entity_type) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            record.id,
            enum_to_str(&record.platform),
            record.task_name,
            record.keyword,
            record.blog_id,
            record.content_preview,
            record.author,
            record.crawled_at,
            record.json_data,
            record.parent_record_id,
            record.entity_type,
        ],
    )?;
    Ok(())
}

pub fn total_count(conn: &Connection) -> Result<i64, AppError> {
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))?;
    Ok(count)
}

struct RawRecord {
    id: String,
    platform: String,
    task_name: String,
    keyword: String,
    blog_id: Option<String>,
    content_preview: String,
    author: String,
    crawled_at: String,
    json_data: Option<String>,
    parent_record_id: Option<String>,
    entity_type: Option<String>,
}

impl RawRecord {
    fn into_model(self) -> Result<CrawledRecord, AppError> {
        Ok(CrawledRecord {
            id: self.id,
            platform: str_to_enum(&self.platform)?,
            task_name: self.task_name,
            keyword: self.keyword,
            blog_id: self.blog_id,
            content_preview: self.content_preview,
            author: self.author,
            crawled_at: self.crawled_at,
            json_data: self.json_data,
            parent_record_id: self.parent_record_id,
            entity_type: self.entity_type,
        })
    }
}
