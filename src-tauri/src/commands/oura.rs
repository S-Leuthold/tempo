use crate::db::AppState;
use crate::oura::{
  build_auth_url, exchange_code_for_tokens, refresh_tokens, wait_for_callback,
  OuraConfig, OuraTokens,
};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

/// ---------------------------------------------------------------------------
/// Start OAuth Flow
/// ---------------------------------------------------------------------------

/// Initiates Oura OAuth by returning the authorization URL.
/// Frontend should open this URL in the default browser.
#[tauri::command]
pub async fn oura_start_auth() -> Result<String, String> {
  let config = OuraConfig::from_env()
    .map_err(|e| e.to_string())?;
  let auth_url = build_auth_url(&config)
    .map_err(|e| e.to_string())?;
  Ok(auth_url)
}

/// ---------------------------------------------------------------------------
/// Wait for Callback and Exchange Code
/// ---------------------------------------------------------------------------

/// Waits for the OAuth callback, exchanges the code for tokens, and stores them.
/// This should be called immediately after oura_start_auth.
#[tauri::command]
pub async fn oura_complete_auth(state: State<'_, Arc<AppState>>) -> Result<(), String> {
  let config = OuraConfig::from_env()
    .map_err(|e| e.to_string())?;

  // Wait for callback (blocking - runs in Tauri's async runtime)
  let callback = tokio::task::spawn_blocking(wait_for_callback)
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

  // Exchange authorization code for tokens
  let tokens = exchange_code_for_tokens(&config, &callback.code).await
    .map_err(|e| e.to_string())?;

  // Store tokens in database
  save_tokens(&state.db, &tokens).await
    .map_err(|e| e.to_string())?;

  println!("Oura OAuth completed successfully");
  Ok(())
}

/// ---------------------------------------------------------------------------
/// Check Authentication Status
/// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct OuraAuthStatus {
  pub is_authenticated: bool,
  pub expires_at: Option<String>,
  pub needs_refresh: bool,
}

#[tauri::command]
pub async fn oura_get_auth_status(
  state: State<'_, Arc<AppState>>,
) -> Result<OuraAuthStatus, String> {
  match load_tokens(&state.db).await.map_err(|e| e.to_string())? {
    Some(tokens) => Ok(OuraAuthStatus {
      is_authenticated: true,
      expires_at: Some(tokens.expires_at.to_rfc3339()),
      needs_refresh: tokens.needs_refresh(),
    }),
    None => Ok(OuraAuthStatus {
      is_authenticated: false,
      expires_at: None,
      needs_refresh: false,
    }),
  }
}

/// ---------------------------------------------------------------------------
/// Disconnect Oura
/// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn oura_disconnect(state: State<'_, Arc<AppState>>) -> Result<(), String> {
  sqlx::query("DELETE FROM oura_auth WHERE id = 1")
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;

  println!("Oura disconnected");
  Ok(())
}

/// ---------------------------------------------------------------------------
/// Token Management (Database Helpers)
/// ---------------------------------------------------------------------------

async fn load_tokens(db: &crate::db::DbPool) -> Result<Option<OuraTokens>, String> {
  let row: Option<(String, String, chrono::DateTime<Utc>)> = sqlx::query_as(
    "SELECT access_token, refresh_token, expires_at FROM oura_auth WHERE id = 1",
  )
  .fetch_optional(db)
  .await
  .map_err(|e| e.to_string())?;

  Ok(row.map(|(access, refresh, expires)| OuraTokens {
    access_token: access,
    refresh_token: refresh,
    expires_at: expires,
  }))
}

async fn save_tokens(db: &crate::db::DbPool, tokens: &OuraTokens) -> Result<(), String> {
  sqlx::query(
    r#"
    INSERT INTO oura_auth (id, access_token, refresh_token, expires_at)
    VALUES (1, ?1, ?2, ?3)
    ON CONFLICT(id) DO UPDATE SET
      access_token = excluded.access_token,
      refresh_token = excluded.refresh_token,
      expires_at = excluded.expires_at,
      updated_at = CURRENT_TIMESTAMP
    "#,
  )
  .bind(&tokens.access_token)
  .bind(&tokens.refresh_token)
  .bind(tokens.expires_at)
  .execute(db)
  .await
  .map_err(|e| e.to_string())?;

  Ok(())
}

/// ---------------------------------------------------------------------------
/// Token Refresh
/// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn oura_refresh_auth(state: State<'_, Arc<AppState>>) -> Result<(), String> {
  let config = OuraConfig::from_env()
    .map_err(|e| e.to_string())?;

  let current_tokens = load_tokens(&state.db)
    .await?
    .ok_or_else(|| "No tokens to refresh".to_string())?;

  let new_tokens = refresh_tokens(&config, &current_tokens.refresh_token).await
    .map_err(|e| e.to_string())?;

  save_tokens(&state.db, &new_tokens).await?;

  println!("Oura tokens refreshed");
  Ok(())
}

/// ---------------------------------------------------------------------------
/// Database Helpers for Oura Data
/// ---------------------------------------------------------------------------

async fn save_sleep_data(
  db: &crate::db::DbPool,
  date: &str,
  sleep_data: &crate::oura::DailySleepData,
) -> Result<(), String> {
  let contributors = &sleep_data.contributors;

  sqlx::query(
    r#"
    INSERT INTO oura_sleep (
      date, total_sleep_seconds, deep_sleep_seconds,
      rem_sleep_seconds, light_sleep_seconds, efficiency_pct
    )
    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
    ON CONFLICT(date) DO UPDATE SET
      total_sleep_seconds = excluded.total_sleep_seconds,
      deep_sleep_seconds = excluded.deep_sleep_seconds,
      rem_sleep_seconds = excluded.rem_sleep_seconds,
      light_sleep_seconds = excluded.light_sleep_seconds,
      efficiency_pct = excluded.efficiency_pct
    "#,
  )
  .bind(date)
  .bind(contributors.total_sleep)
  .bind(contributors.deep_sleep)
  .bind(contributors.rem_sleep)
  .bind(contributors.light_sleep)
  .bind(contributors.sleep_efficiency)
  .execute(db)
  .await
  .map_err(|e| format!("Failed to save sleep data: {}", e))?;

  Ok(())
}

async fn save_hrv_data(
  db: &crate::db::DbPool,
  date: &str,
  hrv_ms: f64,
) -> Result<(), String> {
  sqlx::query(
    r#"
    INSERT INTO oura_hrv (date, average_hrv_ms)
    VALUES (?1, ?2)
    ON CONFLICT(date) DO UPDATE SET
      average_hrv_ms = excluded.average_hrv_ms
    "#,
  )
  .bind(date)
  .bind(hrv_ms)
  .execute(db)
  .await
  .map_err(|e| format!("Failed to save HRV data: {}", e))?;

  Ok(())
}

async fn save_resting_hr_data(
  db: &crate::db::DbPool,
  date: &str,
  resting_hr: i64,
) -> Result<(), String> {
  sqlx::query(
    r#"
    INSERT INTO oura_resting_hr (date, resting_hr)
    VALUES (?1, ?2)
    ON CONFLICT(date) DO UPDATE SET
      resting_hr = excluded.resting_hr
    "#,
  )
  .bind(date)
  .bind(resting_hr)
  .execute(db)
  .await
  .map_err(|e| format!("Failed to save resting HR data: {}", e))?;

  Ok(())
}

/// ---------------------------------------------------------------------------
/// Oura Data Sync Command
/// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct OuraSyncResult {
  pub sleep_records: usize,
  pub hrv_records: usize,
  pub resting_hr_records: usize,
}

#[tauri::command]
pub async fn oura_sync_data(
  state: State<'_, Arc<AppState>>,
) -> Result<OuraSyncResult, String> {
  use crate::oura::{fetch_daily_readiness, fetch_daily_sleep, fetch_sleep_periods, OuraConfig};
  use chrono::Local;

  let config = OuraConfig::from_env().map_err(|e| e.to_string())?;

  // Load tokens from database
  let mut tokens = load_tokens(&state.db)
    .await?
    .ok_or_else(|| "Not connected to Oura".to_string())?;

  // Refresh tokens if needed
  if tokens.needs_refresh() {
    tokens = crate::oura::refresh_tokens(&config, &tokens.refresh_token)
      .await
      .map_err(|e| e.to_string())?;
    save_tokens(&state.db, &tokens).await?;
  }

  // Calculate date range (last 7 days)
  let end_date = Local::now().naive_local().date();
  let start_date = end_date - chrono::Duration::days(7);
  let start_str = start_date.format("%Y-%m-%d").to_string();
  let end_str = end_date.format("%Y-%m-%d").to_string();

  println!("Syncing Oura data from {} to {}", start_str, end_str);

  let mut sleep_count = 0;
  let mut hrv_count = 0;
  let mut resting_hr_count = 0;

  // Fetch sleep periods (contains BOTH actual sleep duration AND HRV)
  // Note: daily_sleep endpoint has "contributors" which are scores (0-100), NOT durations
  match fetch_sleep_periods(&tokens.access_token, &start_str, &end_str).await {
    Ok(response) => {
      println!("DEBUG: Received {} sleep period records from Oura API", response.data.len());

      // Group periods by date for aggregation
      let mut sleep_by_date: std::collections::HashMap<String, Vec<i64>> = std::collections::HashMap::new();
      let mut hrv_by_date: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
      let mut deep_by_date: std::collections::HashMap<String, Vec<i64>> = std::collections::HashMap::new();
      let mut rem_by_date: std::collections::HashMap<String, Vec<i64>> = std::collections::HashMap::new();
      let mut light_by_date: std::collections::HashMap<String, Vec<i64>> = std::collections::HashMap::new();

      for period in response.data {
        // Extract date from bedtime_start (ISO timestamp)
        if let Ok(bedtime) = chrono::DateTime::parse_from_rfc3339(&period.bedtime_start) {
          let date = bedtime.date_naive().format("%Y-%m-%d").to_string();

          // Aggregate sleep durations
          if let Some(total) = period.total_sleep_duration {
            sleep_by_date.entry(date.clone()).or_default().push(total);
          }
          if let Some(deep) = period.deep_sleep_duration {
            deep_by_date.entry(date.clone()).or_default().push(deep);
          }
          if let Some(rem) = period.rem_sleep_duration {
            rem_by_date.entry(date.clone()).or_default().push(rem);
          }
          if let Some(light) = period.light_sleep_duration {
            light_by_date.entry(date.clone()).or_default().push(light);
          }

          // Aggregate HRV
          if let Some(hrv) = period.average_hrv {
            hrv_by_date.entry(date).or_default().push(hrv);
          }
        }
      }

      // Save aggregated sleep data for each date (sum durations from multiple periods)
      for (date, durations) in sleep_by_date {
        let total_sleep = durations.iter().sum::<i64>();
        let deep_sleep = deep_by_date.get(&date).map(|v| v.iter().sum()).unwrap_or(0);
        let rem_sleep = rem_by_date.get(&date).map(|v| v.iter().sum()).unwrap_or(0);
        let light_sleep = light_by_date.get(&date).map(|v| v.iter().sum()).unwrap_or(0);

        println!("DEBUG: Sleep for {}: total={}s ({}h), deep={}s, rem={}s, light={}s",
          date, total_sleep, total_sleep as f64 / 3600.0, deep_sleep, rem_sleep, light_sleep);

        // Create a DailySleepData-like structure for saving
        // We'll just directly insert into DB instead of using save_sleep_data
        sqlx::query(
          r#"
          INSERT INTO oura_sleep (
            date, total_sleep_seconds, deep_sleep_seconds,
            rem_sleep_seconds, light_sleep_seconds
          )
          VALUES (?1, ?2, ?3, ?4, ?5)
          ON CONFLICT(date) DO UPDATE SET
            total_sleep_seconds = excluded.total_sleep_seconds,
            deep_sleep_seconds = excluded.deep_sleep_seconds,
            rem_sleep_seconds = excluded.rem_sleep_seconds,
            light_sleep_seconds = excluded.light_sleep_seconds
          "#,
        )
        .bind(&date)
        .bind(total_sleep)
        .bind(deep_sleep)
        .bind(rem_sleep)
        .bind(light_sleep)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to save sleep data: {}", e))?;

        sleep_count += 1;
      }
      println!("Saved {} sleep records", sleep_count);

      // Save average HRV for each date
      for (date, hrv_values) in hrv_by_date {
        if !hrv_values.is_empty() {
          let avg_hrv = hrv_values.iter().sum::<f64>() / hrv_values.len() as f64;
          save_hrv_data(&state.db, &date, avg_hrv).await?;
          hrv_count += 1;
        }
      }
      println!("Saved {} HRV records", hrv_count);
    }
    Err(e) => {
      eprintln!("Failed to fetch sleep periods: {}", e);
    }
  }

  // Fetch daily readiness for resting HR
  match fetch_daily_readiness(&tokens.access_token, &start_str, &end_str).await {
    Ok(response) => {
      for readiness_data in response.data {
        if let Some(resting_hr) = readiness_data.contributors.resting_heart_rate {
          save_resting_hr_data(&state.db, &readiness_data.day, resting_hr).await?;
          resting_hr_count += 1;
        }
      }
      println!("Saved {} resting HR records", resting_hr_count);
    }
    Err(e) => {
      eprintln!("Failed to fetch resting HR data: {}", e);
    }
  }

  Ok(OuraSyncResult {
    sleep_records: sleep_count,
    hrv_records: hrv_count,
    resting_hr_records: resting_hr_count,
  })
}

/// ---------------------------------------------------------------------------
/// Get Recovery Signals
/// ---------------------------------------------------------------------------

/// Fetch computed recovery signals from Oura data
/// Returns None if data is stale (>36 hours old) or insufficient
#[tauri::command]
pub async fn get_recovery_signals(
  state: State<'_, Arc<AppState>>,
) -> Result<Option<crate::models::recovery::RecoverySignals>, String> {
  const SLEEP_TARGET_HOURS: f64 = 7.0;
  const MAX_AGE_HOURS: i64 = 36;

  crate::oura::compute_recovery_signals_from_db(
    &state.db,
    SLEEP_TARGET_HOURS,
    MAX_AGE_HOURS,
  )
  .await
  .map_err(|e| e.to_string())
}

/// Fetch 7-day Oura history for chart visualization
#[tauri::command]
pub async fn get_oura_history(
  state: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::models::recovery::OuraDay>, String> {
  crate::oura::get_oura_7d_history(&state.db)
    .await
    .map_err(|e| e.to_string())
}
