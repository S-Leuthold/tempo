//! Tauri commands for ceiling-based progression engine

use std::sync::Arc;
use tauri::State;

use crate::db::AppState;
use crate::progression::{
    apply_progression, apply_regression, load_all_dimensions, load_dimension,
    record_ceiling_touch, update_ceiling, ProgressionDimension,
};

/// Get all progression dimensions
#[tauri::command]
pub async fn get_progression_dimensions(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProgressionDimension>, String> {
    load_all_dimensions(&state.db).await
}

/// Get a single dimension by name
#[tauri::command]
pub async fn get_progression_dimension(
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<ProgressionDimension, String> {
    load_dimension(&state.db, &name).await
}

/// Apply a progression to a dimension (advance to next value)
#[tauri::command]
pub async fn progress_dimension(
    state: State<'_, Arc<AppState>>,
    dimension_name: String,
    trigger_workout_id: Option<i64>,
) -> Result<String, String> {
    apply_progression(&state.db, &dimension_name, trigger_workout_id).await
}

/// Apply a regression to a dimension (step back)
#[tauri::command]
pub async fn regress_dimension(
    state: State<'_, Arc<AppState>>,
    dimension_name: String,
) -> Result<String, String> {
    apply_regression(&state.db, &dimension_name).await
}

/// Record a ceiling touch (maintenance workout at ceiling level)
#[tauri::command]
pub async fn touch_ceiling(
    state: State<'_, Arc<AppState>>,
    dimension_name: String,
) -> Result<(), String> {
    record_ceiling_touch(&state.db, &dimension_name).await
}

/// Update the ceiling for a dimension
#[tauri::command]
pub async fn set_dimension_ceiling(
    state: State<'_, Arc<AppState>>,
    dimension_name: String,
    new_ceiling: String,
) -> Result<(), String> {
    update_ceiling(&state.db, &dimension_name, &new_ceiling).await
}
