use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Recovery {
  pub id: i64,
  pub date: NaiveDate,
  pub hrv_average: Option<i64>,
  pub hrv_balance: Option<f64>,
  pub resting_hr: Option<i64>,
  pub sleep_score: Option<i64>,
  pub sleep_duration_seconds: Option<i64>,
  pub readiness_score: Option<i64>,
  pub raw_json: Option<String>,
  pub created_at: Option<DateTime<Utc>>,
}

/// For inserting new recovery records (without id, created_at)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRecovery {
  pub date: NaiveDate,
  pub hrv_average: Option<i64>,
  pub hrv_balance: Option<f64>,
  pub resting_hr: Option<i64>,
  pub sleep_score: Option<i64>,
  pub sleep_duration_seconds: Option<i64>,
  pub readiness_score: Option<i64>,
  pub raw_json: Option<String>,
}
