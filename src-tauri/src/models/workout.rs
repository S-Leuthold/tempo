use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Workout {
  pub id: i64,
  pub strava_id: String,
  pub activity_type: String,
  pub started_at: DateTime<Utc>,
  pub duration_seconds: Option<i64>,
  pub distance_meters: Option<f64>,
  pub elevation_gain_meters: Option<f64>,
  pub average_heartrate: Option<i64>,
  pub max_heartrate: Option<i64>,
  pub average_watts: Option<f64>,
  pub suffer_score: Option<i64>,
  pub raw_json: Option<String>,
  pub created_at: Option<DateTime<Utc>>,
}

/// For inserting new workouts (without id, created_at)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewWorkout {
  pub strava_id: String,
  pub activity_type: String,
  pub started_at: DateTime<Utc>,
  pub duration_seconds: Option<i64>,
  pub distance_meters: Option<f64>,
  pub elevation_gain_meters: Option<f64>,
  pub average_heartrate: Option<i64>,
  pub max_heartrate: Option<i64>,
  pub average_watts: Option<f64>,
  pub suffer_score: Option<i64>,
  pub raw_json: Option<String>,
}
