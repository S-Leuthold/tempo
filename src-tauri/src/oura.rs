//! Oura Ring integration for sleep and recovery data
//!
//! This module handles Oura OAuth, data sync, and context building.
//! We use raw sleep/HRV data, NOT proprietary readiness scores.

use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{Read, Write};
use std::net::TcpListener;

/// ---------------------------------------------------------------------------
/// Configuration Constants
/// ---------------------------------------------------------------------------

const OURA_AUTH_URL: &str = "https://cloud.ouraring.com/oauth/authorize";
const OURA_TOKEN_URL: &str = "https://api.ouraring.com/oauth/token";
const OURA_API_BASE: &str = "https://api.ouraring.com/v2/usercollection";
const REDIRECT_PORT: u16 = 8766;  // Different from Strava (8765)
const TOKEN_REFRESH_BUFFER_MINUTES: i64 = 5;

/// ---------------------------------------------------------------------------
/// OAuth Data Structures
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OuraConfig {
  pub client_id: String,
  pub client_secret: String,
  pub redirect_uri: String,
}

impl OuraConfig {
  pub fn from_env() -> Result<Self, OuraError> {
    Ok(Self {
      client_id: env::var("OURA_CLIENT_ID")
        .map_err(|_| OuraError::MissingConfig("OURA_CLIENT_ID".into()))?,
      client_secret: env::var("OURA_CLIENT_SECRET")
        .map_err(|_| OuraError::MissingConfig("OURA_CLIENT_SECRET".into()))?,
      redirect_uri: format!("http://localhost:{}/callback", REDIRECT_PORT),
    })
  }
}

/// Response from Oura token endpoint
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
  pub access_token: String,
  pub refresh_token: String,
  pub expires_in: i64,  // seconds
  pub token_type: String,
}

/// Stored token state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OuraTokens {
  pub access_token: String,
  pub refresh_token: String,
  pub expires_at: DateTime<Utc>,
}

impl OuraTokens {
  pub fn from_response(resp: TokenResponse) -> Self {
    let expires_at = Utc::now() + Duration::seconds(resp.expires_in);
    Self {
      access_token: resp.access_token,
      refresh_token: resp.refresh_token,
      expires_at,
    }
  }

  pub fn needs_refresh(&self) -> bool {
    let buffer = Duration::minutes(TOKEN_REFRESH_BUFFER_MINUTES);
    Utc::now() + buffer >= self.expires_at
  }
}

/// ---------------------------------------------------------------------------
/// Error Handling
/// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum OuraError {
  #[error("Missing configuration: {0}")]
  MissingConfig(String),

  #[error("HTTP request failed: {0}")]
  Request(#[from] reqwest::Error),

  #[error("OAuth error: {0}")]
  OAuth(String),

  #[error("Callback server error: {0}")]
  Server(String),

  #[error("Database error: {0}")]
  Database(String),

  #[error("API error: {0}")]
  Api(String),
}

/// Oura context for coach analysis (sleep and HRV data only, no proprietary scores)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OuraContext {
  // Sleep data (last night)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sleep_duration_hours: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub deep_sleep_hours: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub rem_sleep_hours: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sleep_efficiency_pct: Option<f64>,

  // 7-day trends
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sleep_avg_7d: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sleep_debt_hours: Option<f64>,  // cumulative shortfall vs 8hr target

  // HRV (raw values in milliseconds, not scores)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hrv_last_night: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hrv_avg_7d: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hrv_trend_direction: Option<String>, // "declining", "stable", "improving"
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hrv_declining_days: Option<u8>,  // consecutive days down

  // Resting HR
  #[serde(skip_serializing_if = "Option::is_none")]
  pub resting_hr: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub resting_hr_avg_7d: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub resting_hr_trend: Option<String>, // "up", "stable", "down"
}

impl Default for OuraContext {
  fn default() -> Self {
    Self {
      sleep_duration_hours: None,
      deep_sleep_hours: None,
      rem_sleep_hours: None,
      sleep_efficiency_pct: None,
      sleep_avg_7d: None,
      sleep_debt_hours: None,
      hrv_last_night: None,
      hrv_avg_7d: None,
      hrv_trend_direction: None,
      hrv_declining_days: None,
      resting_hr: None,
      resting_hr_avg_7d: None,
      resting_hr_trend: None,
    }
  }
}

impl OuraContext {
  /// Check if any Oura data is present
  pub fn has_data(&self) -> bool {
    self.sleep_duration_hours.is_some()
      || self.hrv_last_night.is_some()
      || self.resting_hr.is_some()
  }

  /// Compute sleep debt (hours below 8hr target over last 7 days)
  pub fn compute_sleep_debt(sleep_avg_7d: Option<f64>) -> Option<f64> {
    sleep_avg_7d.and_then(|avg| {
      let target = 8.0;
      let debt = (target - avg) * 7.0;
      if debt > 0.0 {
        Some(debt)
      } else {
        None
      }
    })
  }

  /// Determine HRV trend direction from recent data
  pub fn determine_hrv_trend(hrv_current: Option<f64>, hrv_avg: Option<f64>) -> Option<String> {
    match (hrv_current, hrv_avg) {
      (Some(current), Some(avg)) => {
        let delta = current - avg;
        let pct_change = (delta / avg) * 100.0;

        if pct_change < -5.0 {
          Some("declining".to_string())
        } else if pct_change > 5.0 {
          Some("improving".to_string())
        } else {
          Some("stable".to_string())
        }
      }
      _ => None,
    }
  }

  /// Count consecutive days HRV has declined
  /// TODO: Implement when we have daily HRV history
  pub fn count_hrv_declining_days() -> Option<u8> {
    None  // Placeholder
  }

  /// Determine resting HR trend
  pub fn determine_resting_hr_trend(
    current: Option<i64>,
    avg: Option<i64>,
  ) -> Option<String> {
    match (current, avg) {
      (Some(curr), Some(avg)) => {
        if curr > avg + 3 {
          Some("up".to_string())
        } else if curr < avg - 3 {
          Some("down".to_string())
        } else {
          Some("stable".to_string())
        }
      }
      _ => None,
    }
  }
}

// OAuth and API integration will be added in future commits
// For now, OuraContext is scaffolded and always returns None/default
