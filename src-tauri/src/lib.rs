mod analysis;
mod db;
mod llm;
mod models;
mod commands;
mod progression;
mod strava;
mod oura;

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
      // Strava commands
      commands::strava::strava_start_auth,
      commands::strava::strava_complete_auth,
      commands::strava::strava_get_auth_status,
      commands::strava::strava_refresh_tokens,
      commands::strava::strava_disconnect,
      commands::strava::strava_sync_activities,
      // Oura commands
      commands::oura::oura_start_auth,
      commands::oura::oura_complete_auth,
      commands::oura::oura_get_auth_status,
      commands::oura::oura_refresh_auth,
      commands::oura::oura_disconnect,
      commands::analysis::get_user_settings,
      commands::analysis::update_user_settings,
      commands::analysis::compute_workout_metrics,
      commands::analysis::get_workouts_with_metrics,
      commands::analysis::get_training_context,
      commands::analysis::analyze_workout,
      commands::analysis::get_workout_analysis,
      commands::analysis::get_latest_analysis,
      // Progression commands
      commands::progression::get_progression_dimensions,
      commands::progression::get_progression_dimension,
      commands::progression::progress_dimension,
      commands::progression::regress_dimension,
      commands::progression::touch_ceiling,
      commands::progression::set_dimension_ceiling,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
