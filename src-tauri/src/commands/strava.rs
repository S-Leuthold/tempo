use crate::db::AppState;
use crate::strava::{
  build_auth_url, exchange_code_for_tokens, refresh_tokens, wait_for_callback, StravaConfig,
  StravaError, StravaTokens,
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
