use tauri::State;

use crate::service::settings_service;
use crate::AppState;

#[tauri::command]
pub async fn get_ui_theme(state: State<'_, AppState>) -> Result<Option<String>, String> {
    settings_service::get_ui_theme(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_ui_theme(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    settings_service::set_ui_theme(&state.db, &mode).map_err(|e| e.to_string())
}
