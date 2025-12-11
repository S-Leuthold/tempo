use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration as StdDuration;
use url::Url;

/// ---------------------------------------------------------------------------
/// Configuration Constants
/// ---------------------------------------------------------------------------

const STRAVA_AUTH_URL: &str = "https://www.strava.com/oauth/authorize";
const STRAVA_TOKEN_URL: &str = "https://www.strava.com/oauth/token";
const STRAVA_API_BASE: &str = "https://www.strava.com/api/v3";
const REDIRECT_PORT: u16 = 8765;
const TOKEN_REFRESH_BUFFER_MINUTES: i64 = 5;

/// ---------------------------------------------------------------------------
/// OAuth Data Structures
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StravaConfig {
  pub client_id: String,
  pub client_secret: String,
  pub redirect_uri: String,
}

impl StravaConfig {
  pub fn from_env() -> Result<Self, StravaError> {
    Ok(Self {
      client_id: env::var("STRAVA_CLIENT_ID")
        .map_err(|_| StravaError::MissingConfig("STRAVA_CLIENT_ID".into()))?,
      client_secret: env::var("STRAVA_CLIENT_SECRET")
        .map_err(|_| StravaError::MissingConfig("STRAVA_CLIENT_SECRET".into()))?,
      redirect_uri: format!("http://localhost:{}/callback", REDIRECT_PORT),
    })
  }
}

/// Response from Strava token endpoint
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
  pub access_token: String,
  pub refresh_token: String,
  pub expires_at: i64,
  pub token_type: String,
  pub athlete: Option<AthleteInfo>,
}

/// Basic athlete info returned with tokens
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AthleteInfo {
  pub id: i64,
  pub firstname: Option<String>,
  pub lastname: Option<String>,
}

/// Stored token state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StravaTokens {
  pub access_token: String,
  pub refresh_token: String,
  pub expires_at: DateTime<Utc>,
}

impl StravaTokens {
  pub fn from_response(resp: TokenResponse) -> Self {
    Self {
      access_token: resp.access_token,
      refresh_token: resp.refresh_token,
      expires_at: DateTime::from_timestamp(resp.expires_at, 0).unwrap_or_else(Utc::now),
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
pub enum StravaError {
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

  #[error("Not authenticated with Strava")]
  NotAuthenticated,
}

impl Serialize for StravaError {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_str(&self.to_string())
  }
}

/// ---------------------------------------------------------------------------
/// OAuth URL Generation
/// ---------------------------------------------------------------------------

pub fn build_auth_url(config: &StravaConfig) -> Result<String, StravaError> {
  let mut url = Url::parse(STRAVA_AUTH_URL).map_err(|e| StravaError::OAuth(e.to_string()))?;

  url
    .query_pairs_mut()
    .append_pair("client_id", &config.client_id)
    .append_pair("redirect_uri", &config.redirect_uri)
    .append_pair("response_type", "code")
    .append_pair("scope", "activity:read_all")
    .append_pair("approval_prompt", "auto");

  Ok(url.to_string())
}

/// ---------------------------------------------------------------------------
/// Token Exchange (Authorization Code -> Tokens)
/// ---------------------------------------------------------------------------

pub async fn exchange_code_for_tokens(
  config: &StravaConfig,
  code: &str,
) -> Result<StravaTokens, StravaError> {
  let client = Client::new();

  let response = client
    .post(STRAVA_TOKEN_URL)
    .form(&[
      ("client_id", config.client_id.as_str()),
      ("client_secret", config.client_secret.as_str()),
      ("code", code),
      ("grant_type", "authorization_code"),
    ])
    .send()
    .await?;

  if !response.status().is_success() {
    let error_text = response.text().await.unwrap_or_default();
    return Err(StravaError::OAuth(format!(
      "Token exchange failed: {}",
      error_text
    )));
  }

  let token_response: TokenResponse = response.json().await?;
  Ok(StravaTokens::from_response(token_response))
}

/// ---------------------------------------------------------------------------
/// Token Refresh
/// ---------------------------------------------------------------------------

pub async fn refresh_tokens(
  config: &StravaConfig,
  refresh_token: &str,
) -> Result<StravaTokens, StravaError> {
  let client = Client::new();

  let response = client
    .post(STRAVA_TOKEN_URL)
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
    return Err(StravaError::OAuth(format!(
      "Token refresh failed: {}",
      error_text
    )));
  }

  let token_response: TokenResponse = response.json().await?;
  Ok(StravaTokens::from_response(token_response))
}

/// ---------------------------------------------------------------------------
/// OAuth Callback Server
/// ---------------------------------------------------------------------------

pub struct CallbackResult {
  pub code: String,
}

/// Start a temporary HTTP server, wait for callback, extract auth code
pub fn wait_for_callback(timeout_seconds: u64) -> Result<CallbackResult, StravaError> {
  let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
    .map_err(|e| StravaError::Server(format!("Failed to bind port {}: {}", REDIRECT_PORT, e)))?;

  listener
    .set_nonblocking(true)
    .map_err(|e| StravaError::Server(e.to_string()))?;

  let start = std::time::Instant::now();
  let timeout = StdDuration::from_secs(timeout_seconds);

  loop {
    if start.elapsed() > timeout {
      return Err(StravaError::Server("Callback timeout - no response received".into()));
    }

    match listener.accept() {
      Ok((mut stream, _)) => {
        let mut buffer = [0; 2048];
        stream.read(&mut buffer).ok();

        let request = String::from_utf8_lossy(&buffer);

        if let Some(code) = extract_code_from_request(&request) {
          let response = build_success_response();
          stream.write_all(response.as_bytes()).ok();
          stream.flush().ok();

          return Ok(CallbackResult { code });
        } else if request.contains("error=") {
          let error =
            extract_error_from_request(&request).unwrap_or_else(|| "Unknown error".to_string());

          let response = build_error_response(&error);
          stream.write_all(response.as_bytes()).ok();
          stream.flush().ok();

          return Err(StravaError::OAuth(error));
        }
      }
      Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
        std::thread::sleep(StdDuration::from_millis(100));
        continue;
      }
      Err(e) => {
        return Err(StravaError::Server(e.to_string()));
      }
    }
  }
}

fn extract_code_from_request(request: &str) -> Option<String> {
  let first_line = request.lines().next()?;

  if !first_line.contains("/callback?") {
    return None;
  }

  let url_part = first_line.split_whitespace().nth(1)?;

  for param in url_part.split('?').nth(1)?.split('&') {
    let mut kv = param.split('=');
    if kv.next() == Some("code") {
      return kv.next().map(String::from);
    }
  }
  None
}

fn extract_error_from_request(request: &str) -> Option<String> {
  let first_line = request.lines().next()?;
  let url_part = first_line.split_whitespace().nth(1)?;

  for param in url_part.split('?').nth(1)?.split('&') {
    let mut kv = param.split('=');
    if kv.next() == Some("error") {
      return kv.next().map(|s| s.replace("%20", " "));
    }
  }
  None
}

fn build_success_response() -> String {
  let body = r#"<!DOCTYPE html>
<html>
<head><title>Trainer Log - Connected!</title></head>
<body style="font-family: system-ui; text-align: center; padding: 50px;">
  <h1>Successfully Connected to Strava!</h1>
  <p>You can close this window and return to Trainer Log.</p>
</body>
</html>"#;
  format!(
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
    body.len(),
    body
  )
}

fn build_error_response(error: &str) -> String {
  let body = format!(
    r#"<!DOCTYPE html>
<html>
<head><title>Trainer Log - Error</title></head>
<body style="font-family: system-ui; text-align: center; padding: 50px;">
  <h1>Connection Failed</h1>
  <p>Error: {}</p>
  <p>Please try again.</p>
</body>
</html>"#,
    error
  );
  format!(
    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
    body.len(),
    body
  )
}

/// ---------------------------------------------------------------------------
/// Strava API - Activity Fetching
/// ---------------------------------------------------------------------------

/// Activity summary from Strava API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StravaActivity {
  pub id: i64,
  pub name: String,
  /// Strava uses "type" for legacy and "sport_type" for newer activities
  #[serde(rename = "type", default)]
  pub activity_type: String,
  pub start_date: DateTime<Utc>,
  #[serde(default)]
  pub elapsed_time: i64,
  #[serde(default)]
  pub moving_time: i64,
  #[serde(default)]
  pub distance: Option<f64>,
  #[serde(default)]
  pub total_elevation_gain: Option<f64>,
  #[serde(default)]
  pub average_heartrate: Option<f64>,
  #[serde(default)]
  pub max_heartrate: Option<f64>,
  #[serde(default)]
  pub average_watts: Option<f64>,
  #[serde(default)]
  pub suffer_score: Option<f64>,
}

/// ---------------------------------------------------------------------------
/// Strava API - Activity Streams (time series data)
/// ---------------------------------------------------------------------------

/// Raw stream data from Strava API (array format)
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct StravaStream {
  #[serde(rename = "type")]
  pub stream_type: String,
  pub data: Vec<serde_json::Value>,
  pub series_type: Option<String>,
  pub original_size: Option<i64>,
  pub resolution: Option<String>,
}

/// Alternative stream format when key_by_type=true (object format)
#[derive(Debug, Clone, Deserialize)]
pub struct StravaStreamKeyed {
  pub data: Vec<serde_json::Value>,
  pub series_type: Option<String>,
  pub original_size: Option<i64>,
  pub resolution: Option<String>,
}

/// Downsampled workout samples for charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutSamples {
  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub hr: Vec<i64>,
  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub watts: Vec<i64>,
  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub pace: Vec<f64>,  // min/km
}

impl WorkoutSamples {
  pub fn is_empty(&self) -> bool {
    self.hr.is_empty() && self.watts.is_empty() && self.pace.is_empty()
  }

  pub fn to_json(&self) -> String {
    serde_json::to_string(self).unwrap_or_default()
  }
}

/// Fetch activity streams (heartrate, watts, velocity) from Strava
pub async fn fetch_activity_streams(
  access_token: &str,
  activity_id: i64,
) -> Result<Vec<StravaStream>, StravaError> {
  let client = Client::new();

  // Request time, heartrate, watts, and velocity_smooth streams
  // Use key_by_type=true for more predictable object format
  let url = format!(
    "{}/activities/{}/streams?keys=time,heartrate,watts,velocity_smooth&key_by_type=true",
    STRAVA_API_BASE, activity_id
  );

  let response = client
    .get(&url)
    .header("Authorization", format!("Bearer {}", access_token))
    .send()
    .await?;

  if response.status() == reqwest::StatusCode::UNAUTHORIZED {
    return Err(StravaError::NotAuthenticated);
  }

  // 404 means no streams available for this activity (manual entry, etc.)
  if response.status() == reqwest::StatusCode::NOT_FOUND {
    return Ok(vec![]);
  }

  if !response.status().is_success() {
    let error_text = response.text().await.unwrap_or_default();
    return Err(StravaError::OAuth(format!(
      "Failed to fetch streams: {}",
      error_text
    )));
  }

  let response_text = response.text().await?;

  // Strava returns object format with key_by_type=true:
  // {"time": {"data": [...]}, "heartrate": {"data": [...]}, ...}
  let keyed: std::collections::HashMap<String, StravaStreamKeyed> =
    serde_json::from_str(&response_text).map_err(|e| {
      eprintln!("Failed to parse streams response: {}", e);
      eprintln!(
        "Raw response (first 500 chars): {}",
        &response_text[..response_text.len().min(500)]
      );
      StravaError::OAuth(format!("Failed to parse streams: {}", e))
    })?;

  // Convert keyed format to array format for compatibility with downsample_streams
  let streams: Vec<StravaStream> = keyed
    .into_iter()
    .map(|(key, value)| StravaStream {
      stream_type: key,
      data: value.data,
      series_type: value.series_type,
      original_size: value.original_size,
      resolution: value.resolution,
    })
    .collect();

  Ok(streams)
}

/// Downsample streams to 10-second intervals
pub fn downsample_streams(streams: &[StravaStream], interval_seconds: i64) -> WorkoutSamples {
  // Find each stream type
  let time_data: Vec<i64> = streams
    .iter()
    .find(|s| s.stream_type == "time")
    .map(|s| {
      s.data
        .iter()
        .filter_map(|v| v.as_i64())
        .collect()
    })
    .unwrap_or_default();

  let hr_data: Vec<i64> = streams
    .iter()
    .find(|s| s.stream_type == "heartrate")
    .map(|s| {
      s.data
        .iter()
        .filter_map(|v| v.as_i64())
        .collect()
    })
    .unwrap_or_default();

  let watts_data: Vec<i64> = streams
    .iter()
    .find(|s| s.stream_type == "watts")
    .map(|s| {
      s.data
        .iter()
        .filter_map(|v| v.as_i64())
        .collect()
    })
    .unwrap_or_default();

  // velocity_smooth is in m/s, we need min/km
  let velocity_data: Vec<f64> = streams
    .iter()
    .find(|s| s.stream_type == "velocity_smooth")
    .map(|s| {
      s.data
        .iter()
        .filter_map(|v| v.as_f64())
        .collect()
    })
    .unwrap_or_default();

  if time_data.is_empty() {
    return WorkoutSamples {
      hr: vec![],
      watts: vec![],
      pace: vec![],
    };
  }

  // Downsample by taking average of each interval bucket
  let mut samples = WorkoutSamples {
    hr: vec![],
    watts: vec![],
    pace: vec![],
  };

  let max_time = *time_data.last().unwrap_or(&0);
  let mut bucket_start = 0i64;

  while bucket_start <= max_time {
    let bucket_end = bucket_start + interval_seconds;

    // Find indices in this bucket
    let indices: Vec<usize> = time_data
      .iter()
      .enumerate()
      .filter(|(_, &t)| t >= bucket_start && t < bucket_end)
      .map(|(i, _)| i)
      .collect();

    if !indices.is_empty() {
      // Average HR for bucket
      if !hr_data.is_empty() {
        let sum: i64 = indices.iter().filter_map(|&i| hr_data.get(i)).sum();
        samples.hr.push(sum / indices.len() as i64);
      }

      // Average watts for bucket
      if !watts_data.is_empty() {
        let sum: i64 = indices.iter().filter_map(|&i| watts_data.get(i)).sum();
        samples.watts.push(sum / indices.len() as i64);
      }

      // Average pace for bucket (convert m/s to min/km)
      if !velocity_data.is_empty() {
        let sum: f64 = indices.iter().filter_map(|&i| velocity_data.get(i)).sum();
        let avg_mps = sum / indices.len() as f64;
        if avg_mps > 0.0 {
          let min_per_km = (1000.0 / avg_mps) / 60.0;
          samples.pace.push((min_per_km * 100.0).round() / 100.0); // 2 decimal places
        }
      }
    }

    bucket_start = bucket_end;
  }

  samples
}

/// Fetch recent activities from Strava
pub async fn fetch_activities(
  access_token: &str,
  after: Option<i64>,
  per_page: u32,
) -> Result<Vec<StravaActivity>, StravaError> {
  let client = Client::new();

  let mut url = format!("{}/athlete/activities?per_page={}", STRAVA_API_BASE, per_page);

  if let Some(after_timestamp) = after {
    url.push_str(&format!("&after={}", after_timestamp));
  }

  let response = client
    .get(&url)
    .header("Authorization", format!("Bearer {}", access_token))
    .send()
    .await?;

  if response.status() == reqwest::StatusCode::UNAUTHORIZED {
    return Err(StravaError::NotAuthenticated);
  }

  if !response.status().is_success() {
    let error_text = response.text().await.unwrap_or_default();
    return Err(StravaError::OAuth(format!(
      "Failed to fetch activities: {}",
      error_text
    )));
  }

  // Get raw text first for debugging
  let response_text = response.text().await?;

  // Try to parse, with better error message on failure
  let activities: Vec<StravaActivity> = serde_json::from_str(&response_text)
    .map_err(|e| {
      eprintln!("Failed to parse Strava response: {}", e);
      eprintln!("Raw response (first 1000 chars): {}", &response_text[..response_text.len().min(1000)]);
      StravaError::OAuth(format!("Failed to parse activities: {}", e))
    })?;

  Ok(activities)
}
