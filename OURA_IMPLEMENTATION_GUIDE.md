# Oura Integration Implementation Guide

## Current Status

**✅ Complete:**
- Oura data model (`OuraContext`) with sleep, HRV, resting HR fields
- OAuth scaffolding (config, tokens, error types)
- Integration point in `ContextPackage`
- V4 cards ready to display Oura insights

**📋 To Do:**
1. Complete OAuth implementation (start_auth, complete_auth, refresh)
2. Database migration (oura_auth, sleep, HRV, resting HR tables)
3. Tauri commands (connect, sync, get_status)
4. Data sync from Oura API
5. Update V4 prompt with Oura usage rules
6. Frontend UI (connection button, status display)

---

## OAuth Implementation (Complete in oura.rs)

Following the Strava pattern, add these functions:

```rust
/// Start OAuth flow - returns URL for user to authorize
pub fn start_oauth(config: &OuraConfig) -> Result<String, OuraError> {
  let scopes = "personal daily";  // Sleep, HRV, readiness data

  let auth_url = format!(
    "{}?client_id={}&redirect_uri={}&response_type=code&scope={}",
    OURA_AUTH_URL,
    config.client_id,
    urlencoding::encode(&config.redirect_uri),
    urlencoding::encode(scopes)
  );

  Ok(auth_url)
}

/// Listen for OAuth callback
pub async fn wait_for_callback() -> Result<String, OuraError> {
  let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
    .map_err(|e| OuraError::Server(e.to_string()))?;

  // Accept one connection
  let (mut stream, _) = listener
    .incoming()
    .next()
    .ok_or_else(|| OuraError::Server("No connection received".into()))?
    .map_err(|e| OuraError::Server(e.to_string()))?;

  // Read request
  let mut buffer = [0; 1024];
  stream.read(&mut buffer).map_err(|e| OuraError::Server(e.to_string()))?;

  let request = String::from_utf8_lossy(&buffer);

  // Extract code from query string
  let code = request
    .lines()
    .next()
    .and_then(|line| {
      let url_part = line.split_whitespace().nth(1)?;
      Url::parse(&format!("http://localhost{}", url_part)).ok()
    })
    .and_then(|url| {
      url.query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, code)| code.to_string())
    })
    .ok_or_else(|| OuraError::OAuth("No code in callback".into()))?;

  // Send success response to browser
  let response = "HTTP/1.1 200 OK\r\n\r\nOura connected! You can close this window.";
  stream.write_all(response.as_bytes()).ok();

  Ok(code)
}

/// Exchange authorization code for tokens
pub async fn exchange_code(
  config: &OuraConfig,
  code: &str,
) -> Result<OuraTokens, OuraError> {
  let client = Client::new();

  let params = [
    ("grant_type", "authorization_code"),
    ("code", code),
    ("redirect_uri", &config.redirect_uri),
    ("client_id", &config.client_id),
    ("client_secret", &config.client_secret),
  ];

  let resp = client
    .post(OURA_TOKEN_URL)
    .form(&params)
    .send()
    .await?;

  if !resp.status().is_success() {
    let error_text = resp.text().await?;
    return Err(OuraError::OAuth(format!("Token exchange failed: {}", error_text)));
  }

  let token_resp: TokenResponse = resp.json().await?;
  Ok(OuraTokens::from_response(token_resp))
}

/// Refresh access token
pub async fn refresh_token(
  config: &OuraConfig,
  refresh_token: &str,
) -> Result<OuraTokens, OuraError> {
  let client = Client::new();

  let params = [
    ("grant_type", "refresh_token"),
    ("refresh_token", refresh_token),
    ("client_id", &config.client_id),
    ("client_secret", &config.client_secret),
  ];

  let resp = client
    .post(OURA_TOKEN_URL)
    .form(&params)
    .send()
    .await?;

  if !resp.status().is_success() {
    let error_text = resp.text().await?;
    return Err(OuraError::OAuth(format!("Token refresh failed: {}", error_text)));
  }

  let token_resp: TokenResponse = resp.json().await?;
  Ok(OuraTokens::from_response(token_resp))
}
```

---

## Oura API Data Structures

Based on Oura API v2 documentation:

```rust
/// Daily sleep response from Oura API
#[derive(Debug, Deserialize)]
pub struct DailySleepResponse {
  pub data: Vec<DailySleep>,
}

#[derive(Debug, Deserialize)]
pub struct DailySleep {
  pub day: String,  // ISO date
  pub contributors: SleepContributors,
}

#[derive(Debug, Deserialize)]
pub struct SleepContributors {
  pub deep_sleep: Option<i64>,  // seconds
  pub rem_sleep: Option<i64>,
  pub light_sleep: Option<i64>,
  pub total_sleep: Option<i64>,
  pub efficiency: Option<i64>,  // percentage
}

/// Sleep periods response (for HRV data)
#[derive(Debug, Deserialize)]
pub struct SleepPeriodsResponse {
  pub data: Vec<SleepPeriod>,
}

#[derive(Debug, Deserialize)]
pub struct SleepPeriod {
  pub bedtime_start: String,
  pub bedtime_end: String,
  pub heart_rate: Option<HeartRateData>,
  pub hrv: Option<HrvData>,
  pub average_heart_rate: Option<f64>,
  pub average_hrv: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct HeartRateData {
  pub items: Vec<Option<i64>>,  // 5-min samples
}

#[derive(Debug, Deserialize)]
pub struct HrvData {
  pub items: Vec<Option<f64>>,  // HRV in milliseconds
}

/// Daily readiness response (includes resting HR)
#[derive(Debug, Deserialize)]
pub struct DailyReadinessResponse {
  pub data: Vec<DailyReadiness>,
}

#[derive(Debug, Deserialize)]
pub struct DailyReadiness {
  pub day: String,
  pub contributors: ReadinessContributors,
}

#[derive(Debug, Deserialize)]
pub struct ReadinessContributors {
  pub resting_heart_rate: Option<i64>,
  pub hrv_balance: Option<i64>,  // We can skip this (proprietary)
}
```

---

## Database Migration

**File:** `src-tauri/migrations/20241211000001_add_oura_tables.sql`

```sql
-- Oura OAuth tokens
CREATE TABLE IF NOT EXISTS oura_auth (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  access_token TEXT NOT NULL,
  refresh_token TEXT NOT NULL,
  expires_at TIMESTAMP NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily sleep data
CREATE TABLE IF NOT EXISTS oura_sleep (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date DATE NOT NULL UNIQUE,
  total_sleep_seconds INTEGER,
  deep_sleep_seconds INTEGER,
  rem_sleep_seconds INTEGER,
  light_sleep_seconds INTEGER,
  efficiency_pct INTEGER,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily HRV data
CREATE TABLE IF NOT EXISTS oura_hrv (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date DATE NOT NULL UNIQUE,
  average_hrv_ms REAL NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily resting HR data
CREATE TABLE IF NOT EXISTS oura_resting_hr (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date DATE NOT NULL UNIQUE,
  resting_hr INTEGER NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_oura_sleep_date ON oura_sleep(date DESC);
CREATE INDEX IF NOT EXISTS idx_oura_hrv_date ON oura_hrv(date DESC);
CREATE INDEX IF NOT EXISTS idx_oura_resting_hr_date ON oura_resting_hr(date DESC);
```

---

## Tauri Commands

Add to `src-tauri/src/commands/oura.rs` (create new file):

```rust
use crate::oura::{OuraConfig, start_oauth, wait_for_callback, exchange_code, refresh_token};
use crate::db::AppState;
use tauri::State;
use std::sync::Arc;

#[tauri::command]
pub async fn oura_start_auth() -> Result<String, String> {
  let config = OuraConfig::from_env()
    .map_err(|e| format!("Config error: {}", e))?;

  let auth_url = start_oauth(&config)
    .map_err(|e| format!("OAuth error: {}", e))?;

  Ok(auth_url)
}

#[tauri::command]
pub async fn oura_complete_auth(
  state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
  let config = OuraConfig::from_env()
    .map_err(|e| format!("Config error: {}", e))?;

  // Wait for callback
  let code = wait_for_callback().await
    .map_err(|e| format!("Callback error: {}", e))?;

  // Exchange code for tokens
  let tokens = exchange_code(&config, &code).await
    .map_err(|e| format!("Token exchange error: {}", e))?;

  // Store tokens in database
  sqlx::query(
    r#"
    INSERT INTO oura_auth (id, access_token, refresh_token, expires_at)
    VALUES (1, ?1, ?2, ?3)
    ON CONFLICT(id) DO UPDATE SET
      access_token = excluded.access_token,
      refresh_token = excluded.refresh_token,
      expires_at = excluded.expires_at,
      updated_at = CURRENT_TIMESTAMP
    "#
  )
  .bind(&tokens.access_token)
  .bind(&tokens.refresh_token)
  .bind(&tokens.expires_at)
  .execute(&state.db)
  .await
  .map_err(|e| format!("Database error: {}", e))?;

  Ok(())
}

#[tauri::command]
pub async fn oura_get_auth_status(
  state: State<'_, Arc<AppState>>,
) -> Result<OuraAuthStatus, String> {
  let row: Option<(String, String, DateTime<Utc>)> = sqlx::query_as(
    "SELECT access_token, refresh_token, expires_at FROM oura_auth WHERE id = 1"
  )
  .fetch_optional(&state.db)
  .await
  .map_err(|e| format!("Database error: {}", e))?;

  match row {
    Some((access, refresh, expires)) => {
      let tokens = OuraTokens {
        access_token: access,
        refresh_token: refresh,
        expires_at: expires,
      };

      Ok(OuraAuthStatus {
        is_authenticated: true,
        expires_at: Some(tokens.expires_at.to_rfc3339()),
        needs_refresh: tokens.needs_refresh(),
      })
    }
    None => Ok(OuraAuthStatus {
      is_authenticated: false,
      expires_at: None,
      needs_refresh: false,
    }),
  }
}

#[derive(Serialize)]
pub struct OuraAuthStatus {
  pub is_authenticated: bool,
  pub expires_at: Option<String>,
  pub needs_refresh: bool,
}
```

---

## Data Sync Implementation

```rust
/// Fetch sleep data from Oura API for date range
pub async fn fetch_sleep_data(
  access_token: &str,
  start_date: &str,
  end_date: &str,
) -> Result<DailySleepResponse, OuraError> {
  let client = Client::new();
  let url = format!(
    "{}/daily_sleep?start_date={}&end_date={}",
    OURA_API_BASE, start_date, end_date
  );

  let resp = client
    .get(&url)
    .bearer_auth(access_token)
    .send()
    .await?;

  if !resp.status().is_success() {
    return Err(OuraError::Api(format!("Sleep API error: {}", resp.status())));
  }

  Ok(resp.json().await?)
}

/// Sync Oura data to database
#[tauri::command]
pub async fn oura_sync_data(
  state: State<'_, Arc<AppState>>,
) -> Result<OuraSyncResult, String> {
  // 1. Get tokens from DB
  // 2. Refresh if needed
  // 3. Fetch last 7 days of sleep, HRV, resting HR
  // 4. Store in database
  // 5. Return counts
}
```

---

## Required Scopes

From Oura API documentation:
- `personal` - Basic user info
- `daily` - Daily sleep, readiness, activity

---

## What You Need To Do

1. **Register Oura App:**
   - Go to https://cloud.ouraring.com/oauth/applications
   - Create new application
   - Get client_id and client_secret
   - Set redirect URI: `http://localhost:8766/callback`

2. **Add to .env file:**
   ```
   OURA_CLIENT_ID=your_client_id_here
   OURA_CLIENT_SECRET=your_client_secret_here
   ```

3. **Run migration:**
   ```bash
   # Migration will auto-run on next app start
   ```

4. **Test OAuth flow:**
   - Click "Connect Oura" in UI
   - Authorize in browser
   - Tokens stored in DB
   - Sync data

---

## Next Implementation Steps

1. Complete OAuth functions in `oura.rs`
2. Create `commands/oura.rs` with Tauri commands
3. Add commands to main.rs
4. Create database migration
5. Implement data sync
6. Add frontend UI
7. Update V4 prompt with Oura rules

The architecture is in place - just needs the OAuth plumbing completed following the Strava pattern.
