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

/// ---------------------------------------------------------------------------
/// Tests
/// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_utils::*;
  use serial_test::serial;
  use tauri::Manager;

  #[tokio::test]
  #[serial]
  async fn test_get_progression_dimensions() {
    let pool = setup_test_db().await;
    let _dims = seed_test_progression_dimensions(&pool).await;
    let state = Arc::new(AppState { db: pool.clone() });

    let app = tauri::test::mock_app();
    app.manage(state);

    let result = get_progression_dimensions(app.state()).await;
    assert!(result.is_ok());

    let dimensions = result.unwrap();
    assert_eq!(
      dimensions.len(),
      3,
      "Should return 3 seeded progression dimensions"
    );

    teardown_test_db(pool).await;
  }

  #[tokio::test]
  #[serial]
  async fn test_get_progression_dimension_exists() {
    let pool = setup_test_db().await;
    let _dims = seed_test_progression_dimensions(&pool).await;
    let state = Arc::new(AppState { db: pool.clone() });

    let app = tauri::test::mock_app();
    app.manage(state);

    let result = get_progression_dimension(app.state(), "long_run".to_string()).await;
    assert!(result.is_ok());

    let dimension = result.unwrap();
    assert_eq!(dimension.name, "long_run", "Should return the long_run dimension");
    assert_eq!(dimension.current_value, "30", "Current value should be 30");
    assert_eq!(dimension.ceiling_value, "90", "Ceiling value should be 90");

    teardown_test_db(pool).await;
  }

  #[tokio::test]
  #[serial]
  async fn test_get_progression_dimension_not_found() {
    let pool = setup_test_db().await;
    let state = Arc::new(AppState { db: pool.clone() });

    let app = tauri::test::mock_app();
    app.manage(state);

    let result = get_progression_dimension(app.state(), "nonexistent".to_string()).await;
    assert!(result.is_err());

    teardown_test_db(pool).await;
  }

  #[tokio::test]
  #[serial]
  async fn test_progress_dimension() {
    let pool = setup_test_db().await;
    seed_test_progression_dimensions(&pool).await;
    let state = Arc::new(AppState { db: pool.clone() });

    let app = tauri::test::mock_app();
    app.manage(state);

    // run_interval is seeded as "building" with current_value "4:1" and sequence progression
    let result = progress_dimension(app.state(), "run_interval".to_string(), None).await;
    assert!(result.is_ok(), "Progress should succeed for run_interval in building state");

    // Verify dimension progressed in DB
    let dimension: (String,) = sqlx::query_as(
      "SELECT current_value FROM progression_dimensions WHERE name = 'run_interval'"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch dimension");

    assert_eq!(
      dimension.0, "5:1",
      "run_interval should progress from 4:1 to 5:1"
    );

    teardown_test_db(pool).await;
  }

  #[tokio::test]
  #[serial]
  async fn test_regress_dimension() {
    let pool = setup_test_db().await;
    seed_test_progression_dimensions(&pool).await;
    let state = Arc::new(AppState { db: pool.clone() });

    let app = tauri::test::mock_app();
    app.manage(state);

    // long_run is seeded with current_value "30" and increment of 5
    let result = regress_dimension(app.state(), "long_run".to_string()).await;
    assert!(result.is_ok(), "Regression should succeed for long_run");

    // Verify dimension regressed in DB
    let dimension: (String,) = sqlx::query_as(
      "SELECT current_value FROM progression_dimensions WHERE name = 'long_run'"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch dimension");

    assert_eq!(
      dimension.0, "25",
      "long_run should regress from 30 to 25"
    );

    teardown_test_db(pool).await;
  }

  #[tokio::test]
  #[serial]
  async fn test_touch_ceiling() {
    let pool = setup_test_db().await;
    let _dims = seed_test_progression_dimensions(&pool).await;
    let state = Arc::new(AppState { db: pool.clone() });

    let app = tauri::test::mock_app();
    app.manage(state);

    // z2_ride is seeded as "at_ceiling" so touch should succeed
    let result = touch_ceiling(app.state(), "z2_ride".to_string()).await;
    assert!(result.is_ok(), "touch_ceiling should succeed for z2_ride which is at_ceiling");

    // Verify last_ceiling_touch_at was updated in DB
    let last_touch: Option<String> = sqlx::query_scalar(
      "SELECT last_ceiling_touch_at FROM progression_dimensions WHERE name = 'z2_ride'"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch dimension");

    assert!(
      last_touch.is_some(),
      "last_ceiling_touch_at should be set after touching ceiling"
    );

    teardown_test_db(pool).await;
  }

  #[tokio::test]
  #[serial]
  async fn test_set_dimension_ceiling() {
    let pool = setup_test_db().await;
    let _dims = seed_test_progression_dimensions(&pool).await;
    let state = Arc::new(AppState { db: pool.clone() });

    let app = tauri::test::mock_app();
    app.manage(state);

    let result = set_dimension_ceiling(app.state(), "long_run".to_string(), "120".to_string()).await;
    assert!(result.is_ok(), "set_dimension_ceiling should succeed");

    // Verify ceiling_value was updated in DB
    let dimension: (String,) = sqlx::query_as(
      "SELECT ceiling_value FROM progression_dimensions WHERE name = 'long_run'"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch dimension");

    assert_eq!(
      dimension.0, "120",
      "ceiling_value should be updated to 120"
    );

    teardown_test_db(pool).await;
  }
}
