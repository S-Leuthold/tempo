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
#[allow(dead_code)]
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

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum OuraError {
  #[error("Missing configuration: {0}")]
  MissingConfig(String),

  #[error("HTTP request failed: {0}")]
  Request(String),

  #[error("OAuth error: {0}")]
  OAuth(String),

  #[error("Callback server error: {0}")]
  Server(String),

  #[error("Database error: {0}")]
  Database(String),

  #[error("API error: {0}")]
  Api(String),
}

// Convert reqwest::Error to OuraError
impl From<reqwest::Error> for OuraError {
  fn from(e: reqwest::Error) -> Self {
    OuraError::Request(e.to_string())
  }
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
  #[allow(dead_code)]
  pub fn has_data(&self) -> bool {
    self.sleep_duration_hours.is_some()
      || self.hrv_last_night.is_some()
      || self.resting_hr.is_some()
  }

  /// Compute sleep debt (hours below 8hr target over last 7 days)
  #[allow(dead_code)]
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
  #[allow(dead_code)]
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
  #[allow(dead_code)]
  pub fn count_hrv_declining_days() -> Option<u8> {
    None  // Placeholder
  }

  /// Determine resting HR trend
  #[allow(dead_code)]
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

/// ---------------------------------------------------------------------------
/// OAuth URL Generation
/// ---------------------------------------------------------------------------

pub fn build_auth_url(config: &OuraConfig) -> Result<String, OuraError> {
  let mut url = url::Url::parse(OURA_AUTH_URL)
    .map_err(|e| OuraError::OAuth(e.to_string()))?;

  url
    .query_pairs_mut()
    .append_pair("client_id", &config.client_id)
    .append_pair("redirect_uri", &config.redirect_uri)
    .append_pair("response_type", "code")
    .append_pair("scope", "personal daily");  // Sleep, readiness, activity data

  Ok(url.to_string())
}

/// ---------------------------------------------------------------------------
/// Token Exchange (Authorization Code -> Tokens)
/// ---------------------------------------------------------------------------

pub async fn exchange_code_for_tokens(
  config: &OuraConfig,
  code: &str,
) -> Result<OuraTokens, OuraError> {
  let client = Client::new();

  let response = client
    .post(OURA_TOKEN_URL)
    .form(&[
      ("client_id", config.client_id.as_str()),
      ("client_secret", config.client_secret.as_str()),
      ("code", code),
      ("grant_type", "authorization_code"),
      ("redirect_uri", config.redirect_uri.as_str()),
    ])
    .send()
    .await?;

  if !response.status().is_success() {
    let error_text = response.text().await.unwrap_or_default();
    return Err(OuraError::OAuth(format!(
      "Token exchange failed: {}",
      error_text
    )));
  }

  let token_response: TokenResponse = response.json().await?;
  Ok(OuraTokens::from_response(token_response))
}

/// ---------------------------------------------------------------------------
/// Token Refresh
/// ---------------------------------------------------------------------------

pub async fn refresh_tokens(
  config: &OuraConfig,
  refresh_token: &str,
) -> Result<OuraTokens, OuraError> {
  let client = Client::new();

  let response = client
    .post(OURA_TOKEN_URL)
    .form(&[
      ("client_id", config.client_id.as_str()),
      ("client_secret", config.client_secret.as_str()),
      ("refresh_token", refresh_token),
      ("grant_type", "refresh_token"),
    ])
    .send()
    .await?;

  if !response.status().is_success() {
    let error_text = response.text().await.unwrap_or_default();
    return Err(OuraError::OAuth(format!(
      "Token refresh failed: {}",
      error_text
    )));
  }

  let token_response: TokenResponse = response.json().await?;
  Ok(OuraTokens::from_response(token_response))
}

/// ---------------------------------------------------------------------------
/// OAuth Callback Server
/// ---------------------------------------------------------------------------

pub struct CallbackResult {
  pub code: String,
}

pub fn wait_for_callback() -> Result<CallbackResult, String> {
  let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
    .map_err(|e| format!("Failed to bind: {}", e))?;

  println!("Listening for OAuth callback on port {}...", REDIRECT_PORT);

  // Accept one connection
  let mut stream = listener
    .incoming()
    .next()
    .ok_or_else(|| "No connection received".to_string())?
    .map_err(|e| format!("Connection error: {}", e))?;

  // Read HTTP request
  let mut buffer = [0; 1024];
  let bytes_read = stream
    .read(&mut buffer)
    .map_err(|e| format!("Failed to read: {}", e))?;

  let request = String::from_utf8_lossy(&buffer[..bytes_read]);

  // Extract code from query string
  let code = request
    .lines()
    .next()
    .and_then(|line| {
      // Parse "GET /callback?code=XXX HTTP/1.1"
      let parts: Vec<&str> = line.split_whitespace().collect();
      if parts.len() >= 2 {
        let path = parts[1];
        if let Some(query_start) = path.find('?') {
          let query = &path[query_start + 1..];
          for pair in query.split('&') {
            let kv: Vec<&str> = pair.split('=').collect();
            if kv.len() == 2 && kv[0] == "code" {
              return Some(kv[1].to_string());
            }
          }
        }
      }
      None
    })
    .ok_or_else(|| "No code in callback".to_string())?;

  // Send success response
  let response = "HTTP/1.1 200 OK\r\n\r\n<html><body><h1>Oura Connected!</h1><p>You can close this window.</p></body></html>";
  stream
    .write_all(response.as_bytes())
    .map_err(|e| format!("Failed to write response: {}", e))?;

  println!("Received authorization code");

  Ok(CallbackResult { code })
}

/// ---------------------------------------------------------------------------
/// Oura API Data Structures
/// ---------------------------------------------------------------------------

/// Daily sleep response from Oura API v2
#[derive(Debug, Deserialize)]
pub struct DailySleepResponse {
  pub data: Vec<DailySleepData>,
}

#[derive(Debug, Deserialize)]
pub struct DailySleepData {
  pub day: String,  // ISO date (YYYY-MM-DD)
  pub contributors: SleepContributors,
}

#[derive(Debug, Deserialize)]
pub struct SleepContributors {
  pub deep_sleep: Option<i64>,       // seconds
  pub rem_sleep: Option<i64>,        // seconds
  pub light_sleep: Option<i64>,      // seconds
  pub total_sleep: Option<i64>,      // seconds
  pub sleep_efficiency: Option<i64>, // percentage (0-100)
}

/// Sleep periods response (contains HRV data)
#[derive(Debug, Deserialize)]
pub struct SleepPeriodsResponse {
  pub data: Vec<SleepPeriod>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SleepPeriod {
  pub bedtime_start: String,  // ISO timestamp
  pub bedtime_end: String,    // ISO timestamp
  pub average_hrv: Option<f64>, // HRV in milliseconds
}

/// Daily readiness response (contains resting HR)
#[derive(Debug, Deserialize)]
pub struct DailyReadinessResponse {
  pub data: Vec<DailyReadinessData>,
}

#[derive(Debug, Deserialize)]
pub struct DailyReadinessData {
  pub day: String,  // ISO date (YYYY-MM-DD)
  pub contributors: ReadinessContributors,
}

#[derive(Debug, Deserialize)]
pub struct ReadinessContributors {
  pub resting_heart_rate: Option<i64>,  // beats per minute
}

/// ---------------------------------------------------------------------------
/// Oura API Data Fetching
/// ---------------------------------------------------------------------------

/// Fetch daily sleep data from Oura API for a date range
pub async fn fetch_daily_sleep(
  access_token: &str,
  start_date: &str,  // YYYY-MM-DD
  end_date: &str,    // YYYY-MM-DD
) -> Result<DailySleepResponse, OuraError> {
  let client = Client::new();
  let url = format!(
    "{}/daily_sleep?start_date={}&end_date={}",
    OURA_API_BASE, start_date, end_date
  );

  let response = client
    .get(&url)
    .bearer_auth(access_token)
    .send()
    .await?;

  if !response.status().is_success() {
    let status = response.status();
    let error_text = response.text().await.unwrap_or_default();
    return Err(OuraError::Api(format!(
      "Daily sleep API error {}: {}",
      status, error_text
    )));
  }

  Ok(response.json().await?)
}

/// Fetch sleep periods data (contains HRV) from Oura API for a date range
pub async fn fetch_sleep_periods(
  access_token: &str,
  start_date: &str,  // YYYY-MM-DD
  end_date: &str,    // YYYY-MM-DD
) -> Result<SleepPeriodsResponse, OuraError> {
  let client = Client::new();
  let url = format!(
    "{}/sleep?start_date={}&end_date={}",
    OURA_API_BASE, start_date, end_date
  );

  let response = client
    .get(&url)
    .bearer_auth(access_token)
    .send()
    .await?;

  if !response.status().is_success() {
    let status = response.status();
    let error_text = response.text().await.unwrap_or_default();
    return Err(OuraError::Api(format!(
      "Sleep periods API error {}: {}",
      status, error_text
    )));
  }

  Ok(response.json().await?)
}

/// Fetch daily readiness data (contains resting HR) from Oura API for a date range
pub async fn fetch_daily_readiness(
  access_token: &str,
  start_date: &str,  // YYYY-MM-DD
  end_date: &str,    // YYYY-MM-DD
) -> Result<DailyReadinessResponse, OuraError> {
  let client = Client::new();
  let url = format!(
    "{}/daily_readiness?start_date={}&end_date={}",
    OURA_API_BASE, start_date, end_date
  );

  let response = client
    .get(&url)
    .bearer_auth(access_token)
    .send()
    .await?;

  if !response.status().is_success() {
    let status = response.status();
    let error_text = response.text().await.unwrap_or_default();
    return Err(OuraError::Api(format!(
      "Daily readiness API error {}: {}",
      status, error_text
    )));
  }

  Ok(response.json().await?)
}

/// ---------------------------------------------------------------------------
/// Tests
/// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_compute_sleep_debt_with_deficit() {
    // 6.5hr average vs 8hr target = 1.5hr * 7 days = 10.5hr debt
    let result = OuraContext::compute_sleep_debt(Some(6.5));
    assert_eq!(result, Some(10.5));
  }

  #[test]
  fn test_compute_sleep_debt_no_deficit() {
    // 8.5hr average = no debt
    let result = OuraContext::compute_sleep_debt(Some(8.5));
    assert_eq!(result, None);
  }

  #[test]
  fn test_compute_sleep_debt_none() {
    let result = OuraContext::compute_sleep_debt(None);
    assert_eq!(result, None);
  }

  #[test]
  fn test_hrv_trend_declining() {
    // Current 45ms vs avg 55ms = -18% = declining
    let result = OuraContext::determine_hrv_trend(Some(45.0), Some(55.0));
    assert_eq!(result, Some("declining".to_string()));
  }

  #[test]
  fn test_hrv_trend_improving() {
    // Current 60ms vs avg 55ms = +9% = improving
    let result = OuraContext::determine_hrv_trend(Some(60.0), Some(55.0));
    assert_eq!(result, Some("improving".to_string()));
  }

  #[test]
  fn test_hrv_trend_stable() {
    // Current 56ms vs avg 55ms = +1.8% = stable
    let result = OuraContext::determine_hrv_trend(Some(56.0), Some(55.0));
    assert_eq!(result, Some("stable".to_string()));
  }

  #[test]
  fn test_resting_hr_trend_up() {
    // Current 55 vs avg 50 = +5 = up
    let result = OuraContext::determine_resting_hr_trend(Some(55), Some(50));
    assert_eq!(result, Some("up".to_string()));
  }

  #[test]
  fn test_resting_hr_trend_down() {
    // Current 48 vs avg 52 = -4 = down
    let result = OuraContext::determine_resting_hr_trend(Some(48), Some(52));
    assert_eq!(result, Some("down".to_string()));
  }

  #[test]
  fn test_resting_hr_trend_stable() {
    // Current 51 vs avg 50 = +1 = stable
    let result = OuraContext::determine_resting_hr_trend(Some(51), Some(50));
    assert_eq!(result, Some("stable".to_string()));
  }
}
