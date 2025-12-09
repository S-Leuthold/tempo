mod db;
mod models;
mod commands;
mod strava;

use db::AppState;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // Load environment variables from .env file
  dotenvy::dotenv().ok();

  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .setup(|app| {
      // Initialize database
      let app_handle = app.handle().clone();
      tauri::async_runtime::block_on(async move {
        match db::initialize_db(&app_handle).await {
          Ok(pool) => {
            let state = Arc::new(AppState { db: pool });
            app_handle.manage(state);
            println!("Database ready");
          }
          Err(e) => {
            eprintln!("Failed to initialize database: {}", e);
          }
        }
      });
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      commands::get_workouts,
      commands::get_sync_state,
      commands::strava::strava_start_auth,
      commands::strava::strava_complete_auth,
      commands::strava::strava_get_auth_status,
      commands::strava::strava_refresh_tokens,
      commands::strava::strava_disconnect,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
