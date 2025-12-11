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
  let callback = tokio::task::spawn_blocking(|| wait_for_callback())
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
  .bind(&tokens.expires_at)
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

  // Fetch daily sleep data
  match fetch_daily_sleep(&tokens.access_token, &start_str, &end_str).await {
    Ok(response) => {
      for sleep_data in response.data {
        save_sleep_data(&state.db, &sleep_data.day, &sleep_data).await?;
        sleep_count += 1;
      }
      println!("Saved {} sleep records", sleep_count);
    }
    Err(e) => {
      eprintln!("Failed to fetch sleep data: {}", e);
    }
  }

  // Fetch sleep periods for HRV data
  match fetch_sleep_periods(&tokens.access_token, &start_str, &end_str).await {
    Ok(response) => {
      // Group periods by date and average HRV for each day
      let mut hrv_by_date: std::collections::HashMap<String, Vec<f64>> =
        std::collections::HashMap::new();

      for period in response.data {
        if let Some(hrv) = period.average_hrv {
          // Extract date from bedtime_start (ISO timestamp)
          if let Ok(bedtime) = chrono::DateTime::parse_from_rfc3339(&period.bedtime_start) {
            let date = bedtime.date_naive().format("%Y-%m-%d").to_string();
            hrv_by_date.entry(date).or_insert_with(Vec::new).push(hrv);
          }
        }
      }

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
      eprintln!("Failed to fetch HRV data: {}", e);
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
