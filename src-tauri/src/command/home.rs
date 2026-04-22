use tauri::State;

use crate::model::stats::DashboardStats;
use crate::service::stats_service;
use crate::AppState;

#[tauri::command]
pub async fn get_dashboard_stats(
    state: State<'_, AppState>,
) -> Result<DashboardStats, String> {
    stats_service::get_dashboard_stats(&state.db).map_err(|e| e.to_string())
}
