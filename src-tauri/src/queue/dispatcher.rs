use tokio::sync::mpsc;

use crate::error::AppError;

use super::message::CrawlCommand;

pub fn dispatch(
    tx: &mpsc::Sender<CrawlCommand>,
    cmd: CrawlCommand,
) -> Result<(), AppError> {
    tx.try_send(cmd).map_err(|e| {
        AppError::Internal(format!("Failed to enqueue crawl command: {e}"))
    })
}
