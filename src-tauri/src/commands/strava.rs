use crate::db::AppState;
use crate::strava::{
  build_auth_url, exchange_code_for_tokens, fetch_activities, refresh_tokens, wait_for_callback,
  StravaActivity, StravaConfig, StravaError, StravaTokens,
};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

/// ---------------------------------------------------------------------------
/// Start OAuth Flow
/// ---------------------------------------------------------------------------

/// Initiates Strava OAuth by returning the authorization URL.
/// Frontend should open this URL in the default browser.
#[tauri::command]
pub async fn strava_start_auth() -> Result<String, StravaError> {
  let config = StravaConfig::from_env()?;
  let auth_url = build_auth_url(&config)?;
  Ok(auth_url)
}

/// ---------------------------------------------------------------------------
/// Wait for Callback and Exchange Code
/// ---------------------------------------------------------------------------

/// Waits for the OAuth callback, exchanges the code for tokens, and stores them.
/// This should be called immediately after strava_start_auth.
#[tauri::command]
pub async fn strava_complete_auth(state: State<'_, Arc<AppState>>) -> Result<(), StravaError> {
  let config = StravaConfig::from_env()?;

  // Wait for callback (blocking - runs in Tauri's async runtime)
  let callback = tokio::task::spawn_blocking(|| wait_for_callback(120))
    .await
    .map_err(|e| StravaError::Server(e.to_string()))??;

  // Exchange authorization code for tokens
  let tokens = exchange_code_for_tokens(&config, &callback.code).await?;

  // Store tokens in database
  save_tokens(&state.db, &tokens).await?;

  println!("Strava OAuth completed successfully");
  Ok(())
}

/// ---------------------------------------------------------------------------
/// Check Authentication Status
/// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct StravaAuthStatus {
  pub is_authenticated: bool,
  pub expires_at: Option<String>,
  pub needs_refresh: bool,
}

#[tauri::command]
pub async fn strava_get_auth_status(
  state: State<'_, Arc<AppState>>,
) -> Result<StravaAuthStatus, StravaError> {
  match load_tokens(&state.db).await? {
    Some(tokens) => Ok(StravaAuthStatus {
      is_authenticated: true,
      expires_at: Some(tokens.expires_at.to_rfc3339()),
      needs_refresh: tokens.needs_refresh(),
    }),
    None => Ok(StravaAuthStatus {
      is_authenticated: false,
      expires_at: None,
      needs_refresh: false,
    }),
  }
}

/// ---------------------------------------------------------------------------
/// Refresh Tokens
/// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn strava_refresh_tokens(state: State<'_, Arc<AppState>>) -> Result<(), StravaError> {
  let config = StravaConfig::from_env()?;

  let existing = load_tokens(&state.db)
    .await?
    .ok_or(StravaError::NotAuthenticated)?;

  let new_tokens = refresh_tokens(&config, &existing.refresh_token).await?;
  save_tokens(&state.db, &new_tokens).await?;

  println!("Strava tokens refreshed successfully");
  Ok(())
}

/// ---------------------------------------------------------------------------
/// Disconnect Strava
/// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn strava_disconnect(state: State<'_, Arc<AppState>>) -> Result<(), StravaError> {
  sqlx::query(
    "UPDATE sync_state SET access_token = NULL, refresh_token = NULL,
         token_expires_at = NULL WHERE source = 'strava'",
  )
  .execute(&state.db)
  .await
  .map_err(|e| StravaError::Database(e.to_string()))?;

  println!("Strava disconnected");
  Ok(())
}

/// ---------------------------------------------------------------------------
/// Get Valid Access Token (with auto-refresh)
/// ---------------------------------------------------------------------------

/// Internal helper: get a valid access token, refreshing if necessary.
/// This will be used by activity-fetching commands.
pub async fn get_valid_access_token(db: &crate::db::DbPool) -> Result<String, StravaError> {
  let mut tokens = load_tokens(db).await?.ok_or(StravaError::NotAuthenticated)?;

  if tokens.needs_refresh() {
    let config = StravaConfig::from_env()?;
    tokens = refresh_tokens(&config, &tokens.refresh_token).await?;
    save_tokens(db, &tokens).await?;
    println!("Strava tokens auto-refreshed");
  }

  Ok(tokens.access_token)
}

/// ---------------------------------------------------------------------------
/// Database Helpers
/// ---------------------------------------------------------------------------

async fn save_tokens(db: &crate::db::DbPool, tokens: &StravaTokens) -> Result<(), StravaError> {
  sqlx::query(
    r#"
        INSERT INTO sync_state (source, access_token, refresh_token, token_expires_at)
        VALUES ('strava', ?1, ?2, ?3)
        ON CONFLICT(source) DO UPDATE SET
            access_token = excluded.access_token,
            refresh_token = excluded.refresh_token,
            token_expires_at = excluded.token_expires_at
        "#,
  )
  .bind(&tokens.access_token)
  .bind(&tokens.refresh_token)
  .bind(&tokens.expires_at)
  .execute(db)
  .await
  .map_err(|e| StravaError::Database(e.to_string()))?;

  Ok(())
}

async fn load_tokens(db: &crate::db::DbPool) -> Result<Option<StravaTokens>, StravaError> {
  let row: Option<(Option<String>, Option<String>, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
    "SELECT access_token, refresh_token, token_expires_at
             FROM sync_state WHERE source = 'strava'",
  )
  .fetch_optional(db)
  .await
  .map_err(|e| StravaError::Database(e.to_string()))?;

  match row {
    Some((Some(access), Some(refresh), Some(expires))) => Ok(Some(StravaTokens {
      access_token: access,
      refresh_token: refresh,
      expires_at: expires,
    })),
    _ => Ok(None),
  }
}

/// ---------------------------------------------------------------------------
/// Sync Activities from Strava
/// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SyncResult {
  pub new_activities: usize,
  pub total_fetched: usize,
}

/// Sync recent activities from Strava and store them in the database
#[tauri::command]
pub async fn strava_sync_activities(
  state: State<'_, Arc<AppState>>,
) -> Result<SyncResult, StravaError> {
  // Get valid access token (auto-refreshes if needed)
  let access_token = get_valid_access_token(&state.db).await?;

  // Get the timestamp of the most recent workout we have
  let last_activity_timestamp: Option<i64> = sqlx::query_scalar(
    "SELECT CAST(strftime('%s', MAX(started_at)) AS INTEGER) FROM workouts",
  )
  .fetch_one(&state.db)
  .await
  .map_err(|e| StravaError::Database(e.to_string()))?;

  // Fetch activities from Strava (after our last known activity, or all if first sync)
  let activities = fetch_activities(&access_token, last_activity_timestamp, 50).await?;
  let total_fetched = activities.len();

  // Store each activity in the database
  let mut new_count = 0;
  for activity in activities {
    let inserted = save_activity(&state.db, &activity).await?;
    if inserted {
      new_count += 1;
    }
  }

  // Update last sync time
  update_sync_time(&state.db).await?;

  println!(
    "Strava sync complete: {} new activities (fetched {})",
    new_count, total_fetched
  );

  Ok(SyncResult {
    new_activities: new_count,
    total_fetched,
  })
}

/// Save a single activity to the database (returns true if inserted, false if already exists)
async fn save_activity(
  db: &crate::db::DbPool,
  activity: &StravaActivity,
) -> Result<bool, StravaError> {
  let raw_json = serde_json::to_string(activity).unwrap_or_default();

  let result = sqlx::query(
    r#"
    INSERT INTO workouts (
      strava_id, activity_type, started_at, duration_seconds,
      distance_meters, elevation_gain_meters, average_heartrate,
      max_heartrate, average_watts, suffer_score, raw_json
    )
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
    ON CONFLICT(strava_id) DO NOTHING
    "#,
  )
  .bind(activity.id.to_string())
  .bind(&activity.activity_type)
  .bind(&activity.start_date)
  .bind(activity.moving_time)
  .bind(activity.distance)
  .bind(activity.total_elevation_gain)
  .bind(activity.average_heartrate.map(|hr| hr as i64))
  .bind(activity.max_heartrate.map(|hr| hr as i64))
  .bind(activity.average_watts)
  .bind(activity.suffer_score)
  .bind(&raw_json)
  .execute(db)
  .await
  .map_err(|e| StravaError::Database(e.to_string()))?;

  Ok(result.rows_affected() > 0)
}

/// Update the last sync time for Strava
async fn update_sync_time(db: &crate::db::DbPool) -> Result<(), StravaError> {
  sqlx::query(
    "UPDATE sync_state SET last_sync_at = CURRENT_TIMESTAMP WHERE source = 'strava'",
  )
  .execute(db)
  .await
  .map_err(|e| StravaError::Database(e.to_string()))?;

  Ok(())
}
