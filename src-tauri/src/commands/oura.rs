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
