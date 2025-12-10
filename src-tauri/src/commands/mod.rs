pub mod analysis;
pub mod progression;
pub mod strava;

use crate::db::AppState;
use crate::models::{Workout, SyncState};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_workouts(
  state: State<'_, Arc<AppState>>,
) -> Result<Vec<Workout>, String> {
  sqlx::query_as::<_, Workout>(
    "SELECT * FROM workouts ORDER BY started_at DESC LIMIT 50"
  )
  .fetch_all(&state.db)
  .await
  .map_err(|e| format!("Failed to fetch workouts: {}", e))
}

#[tauri::command]
pub async fn get_sync_state(
  state: State<'_, Arc<AppState>>,
) -> Result<Vec<SyncState>, String> {
  sqlx::query_as::<_, SyncState>(
    "SELECT * FROM sync_state"
  )
  .fetch_all(&state.db)
  .await
  .map_err(|e| format!("Failed to fetch sync state: {}", e))
}
