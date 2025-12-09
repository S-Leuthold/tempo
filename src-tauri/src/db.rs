use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

pub type DbPool = SqlitePool;

/// Application state holding the database connection pool
pub struct AppState {
  pub db: DbPool,
}

/// Get the path to the database file
/// Stored in: ~/Library/Application Support/com.samleuthold.trainer-log/trainer-log.db
fn get_db_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, Box<dyn std::error::Error>> {
  let data_dir = app
    .path()
    .app_data_dir()
    .map_err(|e| format!("Failed to get app data dir: {}", e))?;

  // Create directory if it doesn't exist
  fs::create_dir_all(&data_dir)?;

  Ok(data_dir.join("trainer-log.db"))
}

/// Initialize the database connection pool and run migrations
pub async fn initialize_db<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<DbPool, Box<dyn std::error::Error>> {
  let db_path = get_db_path(app)?;
  let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

  println!("Initializing database at: {}", db_path.display());

  // Create connection pool
  let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .connect(&db_url)
    .await?;

  // Run migrations
  sqlx::migrate!("./migrations").run(&pool).await?;

  println!("Database initialized successfully");

  Ok(pool)
}
