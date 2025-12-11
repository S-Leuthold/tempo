use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkoutAnalysis {
  pub id: i64,
  pub workout_id: i64,
  pub summary: Option<String>,
  pub tomorrow_recommendation: Option<String>,
  pub risk_flags_json: Option<String>,
  pub kilimanjaro_notes: Option<String>,
  pub model_version: Option<String>,
  pub prompt_hash: Option<String>,
  pub created_at: Option<DateTime<Utc>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewWorkoutAnalysis {
  pub workout_id: i64,
  pub summary: Option<String>,
  pub tomorrow_recommendation: Option<String>,
  pub risk_flags_json: Option<String>,
  pub kilimanjaro_notes: Option<String>,
  pub model_version: Option<String>,
  pub prompt_hash: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WeeklySummary {
  pub id: i64,
  pub week_start: NaiveDate,
  pub total_duration_seconds: Option<i64>,
  pub run_duration_seconds: Option<i64>,
  pub ride_duration_seconds: Option<i64>,
  pub avg_hrv: Option<i64>,
  pub training_load_trend: Option<String>,
  pub llm_summary: Option<String>,
  pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SyncState {
  pub id: i64,
  pub source: String,
  pub last_sync_at: Option<DateTime<Utc>>,
  pub last_activity_at: Option<DateTime<Utc>>,
  pub access_token: Option<String>,
  pub refresh_token: Option<String>,
  pub token_expires_at: Option<DateTime<Utc>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Goal {
  pub id: i64,
  pub name: String,
  pub target_date: Option<NaiveDate>,
  pub description: Option<String>,
  pub active: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewGoal {
  pub name: String,
  pub target_date: Option<NaiveDate>,
  pub description: Option<String>,
  pub active: Option<bool>,
}
