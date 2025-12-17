//! Test utilities and helpers for integration and unit testing
//!
//! This module provides common test infrastructure including:
//! - Database setup/teardown
//! - Mock data factories
//! - Test fixtures
//! - Helper assertions

use crate::analysis::{UserSettings, TrainingContext, WorkoutSummary};
use crate::strava::StravaActivity;
use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;

/// ---------------------------------------------------------------------------
/// Database Test Utilities
/// ---------------------------------------------------------------------------

/// Create an in-memory SQLite database for testing
/// Runs all migrations and returns a ready-to-use pool
///
/// Uses max_connections(1) to prevent multiple pool connections from creating
/// isolated in-memory databases, which would cause intermittent test failures
pub async fn setup_test_db() -> SqlitePool {
  let pool = sqlx::sqlite::SqlitePoolOptions::new()
    .max_connections(1)
    .connect("sqlite::memory:")
    .await
    .expect("Failed to create in-memory database");

  // Run migrations
  sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .expect("Failed to run migrations");

  pool
}

/// Close a test database pool
pub async fn teardown_test_db(pool: SqlitePool) {
  pool.close().await;
}

/// Seed the database with test workouts
/// Returns the IDs of created workouts
pub async fn seed_test_workouts(pool: &SqlitePool, count: usize) -> Vec<i64> {
  let mut workout_ids = Vec::new();

  for i in 0..count {
    let days_ago = i as i64;
    let activity_type = if i % 2 == 0 { "Run" } else { "Ride" };
    let started_at = Utc::now() - Duration::days(days_ago);

    // Base metrics
    let duration = 3600; // 1 hour
    let distance = if activity_type == "Run" {
      Some(10000.0) // 10km
    } else {
      None
    };
    let avg_hr = Some(140 + (i % 20) as i64);
    let avg_watts = if activity_type == "Ride" {
      Some(150.0 + (i as f64))
    } else {
      None
    };

    let result = sqlx::query(
      r#"
      INSERT INTO workouts (
        strava_id, activity_type, started_at, duration_seconds,
        distance_meters, average_heartrate, average_watts, suffer_score
      )
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
      "#,
    )
    .bind(format!("test_{}", i))
    .bind(activity_type)
    .bind(started_at)
    .bind(duration)
    .bind(distance)
    .bind(avg_hr)
    .bind(avg_watts)
    .bind(50 + i as i64)
    .execute(pool)
    .await
    .expect("Failed to insert test workout");

    workout_ids.push(result.last_insert_rowid());
  }

  workout_ids
}

/// Seed the database with test user settings
pub async fn seed_test_user_settings(pool: &SqlitePool) -> UserSettings {
  let settings = UserSettings {
    max_hr: Some(190),
    lthr: Some(170),
    ftp: Some(250),
    training_days_per_week: 6,
  };

  sqlx::query(
    r#"
    INSERT INTO user_settings (id, max_hr, lthr, ftp, training_days_per_week)
    VALUES (1, ?1, ?2, ?3, ?4)
    ON CONFLICT(id) DO UPDATE SET
      max_hr = excluded.max_hr,
      lthr = excluded.lthr,
      ftp = excluded.ftp,
      training_days_per_week = excluded.training_days_per_week
    "#,
  )
  .bind(settings.max_hr)
  .bind(settings.lthr)
  .bind(settings.ftp)
  .bind(settings.training_days_per_week)
  .execute(pool)
  .await
  .expect("Failed to seed user settings");

  settings
}

/// Seed the database with test progression dimensions
/// Uses INSERT OR REPLACE to handle duplicate seeds
pub async fn seed_test_progression_dimensions(pool: &SqlitePool) -> Vec<String> {
  let dimensions = vec![
    (
      "run_interval",
      "4:1",
      "continuous_45",
      "building",
      r#"{"type":"sequence","sequence":["4:1","5:1","6:1","continuous_45"]}"#,
    ),
    (
      "long_run",
      "30",
      "90",
      "building",
      r#"{"type":"increment","increment":5,"unit":"min"}"#,
    ),
    (
      "z2_ride",
      "45",
      "60",
      "at_ceiling",
      r#"{"type":"regulated","options":[30,45,60],"unit":"min"}"#,
    ),
  ];

  let mut names = Vec::new();

  for (name, current, ceiling, status, step_config_json) in dimensions {
    sqlx::query(
      r#"
      INSERT OR REPLACE INTO progression_dimensions (
        name, current_value, ceiling_value, step_config_json,
        status, last_change_at
      )
      VALUES (?1, ?2, ?3, ?4, ?5, ?6)
      "#,
    )
    .bind(name)
    .bind(current)
    .bind(ceiling)
    .bind(step_config_json)
    .bind(status)
    .bind(Utc::now())
    .execute(pool)
    .await
    .expect("Failed to seed progression dimension");

    names.push(name.to_string());
  }

  names
}

/// ---------------------------------------------------------------------------
/// Mock Data Factories
/// ---------------------------------------------------------------------------

/// Create mock user settings for testing
pub fn mock_user_settings() -> UserSettings {
  UserSettings {
    max_hr: Some(190),
    lthr: Some(170),
    ftp: Some(250),
    training_days_per_week: 6,
  }
}

/// Create a mock workout summary for testing
pub fn mock_workout_summary(activity_type: &str, days_ago: i64) -> WorkoutSummary {
  WorkoutSummary {
    started_at: Utc::now() - Duration::days(days_ago),
    activity_type: activity_type.to_string(),
    duration_seconds: Some(3600),
    rtss: Some(50.0),
    hr_zone: Some(crate::analysis::HrZone::Z2),
  }
}

/// Create a mock Strava activity for testing
pub fn mock_strava_activity() -> StravaActivity {
  StravaActivity {
    id: 123456,
    name: "Morning Run".to_string(),
    activity_type: "Run".to_string(),
    start_date: Utc::now(),
    elapsed_time: 3600,
    moving_time: 3600,
    distance: Some(10000.0),
    total_elevation_gain: Some(100.0),
    average_heartrate: Some(145.0),
    max_heartrate: Some(165.0),
    average_watts: None,
    suffer_score: Some(50.0),
  }
}

/// Create a mock training context for testing
pub fn mock_training_context() -> TrainingContext {
  TrainingContext {
    atl: Some(280.0),
    ctl: Some(250.0),
    tsb: Some(-30.0),
    weekly_volume: crate::analysis::WeeklyVolume {
      total_hrs: 6.5,
      run_hrs: 3.2,
      ride_hrs: 3.3,
      other_hrs: 0.0,
    },
    week_over_week_delta_pct: Some(10.0),
    intensity_distribution: crate::analysis::IntensityDistribution {
      z1_pct: 20.0,
      z2_pct: 60.0,
      z3_pct: 15.0,
      z4_pct: 4.0,
      z5_pct: 1.0,
    },
    longest_session: crate::analysis::LongestSession {
      run_min: Some(60.0),
      ride_min: Some(90.0),
    },
    consistency_pct: Some(85.0),
    workouts_this_week: 5,
  }
}

/// ---------------------------------------------------------------------------
/// Time Helpers
/// ---------------------------------------------------------------------------

/// Create a DateTime N days ago from now
pub fn datetime_days_ago(days: i64) -> DateTime<Utc> {
  Utc::now() - Duration::days(days)
}

/// Create a DateTime representing now
pub fn datetime_now() -> DateTime<Utc> {
  Utc::now()
}

/// ---------------------------------------------------------------------------
/// Test Macros
/// ---------------------------------------------------------------------------

/// Assert two floats are approximately equal within a tolerance
#[macro_export]
macro_rules! assert_approx_eq {
  ($left:expr, $right:expr, $tolerance:expr) => {
    let diff = ($left - $right).abs();
    assert!(
      diff < $tolerance,
      "Values not approximately equal: {} vs {} (diff: {}, tolerance: {})",
      $left,
      $right,
      diff,
      $tolerance
    );
  };
}

/// ---------------------------------------------------------------------------
/// Tests for Test Utilities
/// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_setup_db_creates_schema() {
    let pool = setup_test_db().await;

    // Verify key tables exist
    let tables: Vec<(String,)> = sqlx::query_as(
      "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('workouts', 'user_settings', 'progression_dimensions')"
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query tables");

    assert!(tables.len() >= 3, "Expected at least 3 tables, got {}", tables.len());

    teardown_test_db(pool).await;
  }

  #[tokio::test]
  async fn test_seed_workouts_returns_correct_count() {
    let pool = setup_test_db().await;

    let ids = seed_test_workouts(&pool, 5).await;
    assert_eq!(ids.len(), 5);

    // Verify workouts were inserted
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workouts")
      .fetch_one(&pool)
      .await
      .expect("Failed to count workouts");

    assert_eq!(count, 5);

    teardown_test_db(pool).await;
  }

  #[test]
  fn test_mock_factories_create_valid_data() {
    let settings = mock_user_settings();
    assert_eq!(settings.max_hr, Some(190));
    assert_eq!(settings.lthr, Some(170));

    let workout = mock_workout_summary("Run", 1);
    assert_eq!(workout.activity_type, "Run");
    assert!(workout.duration_seconds.is_some());

    let activity = mock_strava_activity();
    assert_eq!(activity.activity_type, "Run");
    assert_eq!(activity.distance, Some(10000.0));

    let context = mock_training_context();
    assert!(context.atl.is_some());
    assert!(context.ctl.is_some());
  }

  #[test]
  fn test_datetime_helpers_produce_correct_dates() {
    let now = datetime_now();
    let past = datetime_days_ago(7);

    let diff = now - past;
    // Allow for slight timing differences (6-8 days is acceptable)
    assert!(diff.num_days() >= 6 && diff.num_days() <= 8,
            "Expected ~7 days difference, got {}", diff.num_days());
  }
}
