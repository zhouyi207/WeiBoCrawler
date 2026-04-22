use rust_xlsxwriter::{Workbook, XlsxError};

use crate::db::record_repo;
use crate::db::Database;
use crate::error::AppError;
use crate::model::record::CrawledRecord;

pub fn query_records(
    db: &Database,
    platform: Option<&str>,
    keyword: Option<&str>,
) -> Result<Vec<CrawledRecord>, AppError> {
    let conn = db.conn();
    record_repo::query(&conn, platform, keyword)
}

pub fn list_distinct_task_names(
    db: &Database,
    platform: Option<&str>,
) -> Result<Vec<String>, AppError> {
    let conn = db.conn();
    record_repo::list_distinct_task_names(&conn, platform)
}

pub fn query_records_paged(
    db: &Database,
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
    offset: i64,
    limit: i64,
) -> Result<(Vec<CrawledRecord>, i64), AppError> {
    let conn = db.conn();
    record_repo::query_paged(
        &conn,
        platform,
        keyword,
        task_name,
        entity_type,
        offset,
        limit,
    )
}

pub fn deduplicate(
    db: &Database,
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
) -> Result<u64, AppError> {
    let conn = db.conn();
    record_repo::deduplicate_filtered(&conn, platform, keyword, task_name, entity_type)
}

pub fn delete_filtered(
    db: &Database,
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
) -> Result<u64, AppError> {
    let conn = db.conn();
    record_repo::delete_filtered(&conn, platform, keyword, task_name, entity_type)
}

/// 当前筛选条件下的记录，格式化为 JSON 数组（含完整 `jsonData` 等字段）。
pub fn export_json(
    db: &Database,
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
) -> Result<String, AppError> {
    let conn = db.conn();
    let records = record_repo::query_filtered(
        &conn,
        platform,
        keyword,
        task_name,
        entity_type,
    )?;
    let json = serde_json::to_string_pretty(&records)?;
    Ok(json)
}

/// 导出为 Excel `.xlsx`，包含 [`CrawledRecord`] 全部字段（与前端 `jsonData` 等一致），仅当前筛选条件。
pub fn export_xlsx(
    db: &Database,
    platform: Option<&str>,
    keyword: Option<&str>,
    task_name: Option<&str>,
    entity_type: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let conn = db.conn();
    let records = record_repo::query_filtered(
        &conn,
        platform,
        keyword,
        task_name,
        entity_type,
    )?;

    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();

    // Excel 单格最大约 32767 字符，超长时截断避免写入失败。
    fn excel_cell_str(s: &str) -> String {
        const MAX_CHARS: usize = 32700;
        if s.chars().count() <= MAX_CHARS {
            return s.to_string();
        }
        let mut t: String = s.chars().take(MAX_CHARS).collect();
        t.push_str("…(truncated)");
        t
    }

    const HEADERS: &[&str] = &[
        "id",
        "platform",
        "taskName",
        "keyword",
        "blogId",
        "contentPreview",
        "author",
        "crawledAt",
        "jsonData",
        "parentRecordId",
        "entityType",
    ];

    for (c, h) in HEADERS.iter().enumerate() {
        sheet.write(0, c as u16, *h).map_err(xlsx_err)?;
    }

    for (ri, r) in records.iter().enumerate() {
        let row = (ri + 1) as u32;
        let parent = r.parent_record_id.as_deref().unwrap_or("");
        let entity = r.entity_type.as_deref().unwrap_or("");
        let json = r.json_data.as_deref().unwrap_or("");
        let blog = r.blog_id.as_deref().unwrap_or("");
        let plat = crate::db::enum_to_str(&r.platform);
        sheet.write(row, 0, r.id.as_str()).map_err(xlsx_err)?;
        sheet.write(row, 1, plat.as_str()).map_err(xlsx_err)?;
        sheet.write(row, 2, r.task_name.as_str()).map_err(xlsx_err)?;
        sheet.write(row, 3, r.keyword.as_str()).map_err(xlsx_err)?;
        sheet.write(row, 4, blog).map_err(xlsx_err)?;
        sheet.write(row, 5, excel_cell_str(r.content_preview.as_str()).as_str()).map_err(xlsx_err)?;
        sheet.write(row, 6, r.author.as_str()).map_err(xlsx_err)?;
        sheet.write(row, 7, r.crawled_at.as_str()).map_err(xlsx_err)?;
        let json_cell = excel_cell_str(json);
        sheet.write(row, 8, json_cell.as_str()).map_err(xlsx_err)?;
        sheet.write(row, 9, parent).map_err(xlsx_err)?;
        sheet.write(row, 10, entity).map_err(xlsx_err)?;
    }

    workbook.save_to_buffer().map_err(xlsx_err)
}

fn xlsx_err(e: XlsxError) -> AppError {
    AppError::Internal(e.to_string())
}
