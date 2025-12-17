//! Test script for recovery signal computation
//!
//! Run with: cargo run --bin test_recovery

use sqlx::sqlite::SqlitePool;
use std::error::Error;
use trainer_log_lib::oura::compute_recovery_signals_from_db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  println!("=== Recovery Signal Test ===\n");

  // Load environment variables
  dotenvy::dotenv().ok();

  // Get database path from environment or use default macOS app data location
  let db_path = std::env::var("DATABASE_URL")
    .unwrap_or_else(|_| {
      // Default to macOS app data directory
      let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
      let app_data = format!("{}/Library/Application Support/com.trainer-log.app/trainer-log.db", home);
      format!("sqlite://{}?mode=rwc", app_data)
    });

  println!("Connecting to database: {}", db_path);

  // Connect to database
  let pool = SqlitePool::connect(&db_path).await?;

  println!("✅ Connected to database\n");

  // Compute recovery signals
  println!("Computing recovery signals (sleep_target=7.0h, max_age=36h)...\n");

  let signals = compute_recovery_signals_from_db(&pool, 7.0, 36).await?;

  match signals {
    Some(s) => {
      println!("✅ RECOVERY SIGNALS COMPUTED\n");
      println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
      println!("🎯 OVERALL BAND: {:?}", s.overall_band);
      println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

      println!("💤 SLEEP AXIS");
      println!("  Current:       {:>8.1}h", s.sleep.current_value.unwrap_or(0.0));
      println!("  7d Average:    {:>8.1}h", s.sleep.avg_7d.unwrap_or(0.0));
      println!("  28d Baseline:  {:>8.1}h", s.sleep.avg_28d.unwrap_or(0.0));
      println!("  Sleep Debt:    {:>8.1}h (7-day cumulative)", s.sleep.debt.unwrap_or(0.0));
      if let Some(slope) = s.sleep.trend_7d_slope {
        println!("  7d Trend:      {:>8.2}h/day", slope);
      }
      println!();

      println!("❤️  HRV AXIS");
      println!("  Current:       {:>8.1}ms", s.hrv.current_value.unwrap_or(0.0));
      println!("  7d Average:    {:>8.1}ms", s.hrv.avg_7d.unwrap_or(0.0));
      println!("  28d Baseline:  {:>8.1}ms", s.hrv.avg_28d.unwrap_or(0.0));
      if let Some(delta) = s.hrv.state_vs_28d {
        println!("  vs Baseline:   {:>8.1}ms ({})", delta, if delta < 0.0 { "⚠️ BELOW" } else { "✅ ABOVE" });
      }
      if let Some(slope) = s.hrv.trend_7d_slope {
        println!("  7d Trend:      {:>8.2}ms/day", slope);
      }
      if let Some(days) = s.hrv.consecutive_declining_days {
        if days > 0 {
          println!("  Declining:     {} consecutive days", days);
        }
      }
      println!();

      println!("🫀 RESTING HR AXIS");
      println!("  Current:       {:>8.1}bpm", s.rhr.current_value.unwrap_or(0.0));
      println!("  7d Average:    {:>8.1}bpm", s.rhr.avg_7d.unwrap_or(0.0));
      println!("  28d Baseline:  {:>8.1}bpm", s.rhr.avg_28d.unwrap_or(0.0));
      if let Some(delta) = s.rhr.state_vs_28d {
        println!("  vs Baseline:   {:>8.1}bpm ({})", delta, if delta > 0.0 { "⚠️ ELEVATED" } else { "✅ NORMAL" });
      }
      if let Some(slope) = s.rhr.trend_7d_slope {
        println!("  7d Trend:      {:>8.2}bpm/day", slope);
      }
      if let Some(days) = s.rhr.consecutive_declining_days {
        if days > 0 {
          println!("  Rising:        {} consecutive days", days);
        }
      }
      println!();

      println!("🔍 RECOVERY CONSTRAINTS");
      println!("  Intensity Cap:     {:?}", s.constraints.intensity_cap);
      println!("  Duration Bias:     {:?}", s.constraints.duration_bias);
      println!("  Progression Gate:  {}", if s.constraints.progression_gate { "⚠️ ACTIVE" } else { "✅ INACTIVE" });
      if let Some(note) = &s.constraints.caution_note {
        println!("  Caution:           {}", note);
      }
      println!();

      if !s.flags.is_empty() {
        println!("⚠️  ACTIVE FLAGS");
        for flag in &s.flags {
          println!("  • {:?}", flag);
        }
        println!();
      } else {
        println!("✅ NO ACTIVE FLAGS\n");
      }

      println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
      println!("Last Updated: {}", s.last_updated);
      println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
    None => {
      println!("⚠️  NO RECOVERY SIGNALS AVAILABLE");
      println!("\nPossible reasons:");
      println!("  • Oura data is stale (>36 hours old)");
      println!("  • Oura data is incomplete (missing current or baseline metrics)");
      println!("  • Oura not connected or no data synced");
      println!("\nNext steps:");
      println!("  1. Connect Oura in the app");
      println!("  2. Click 'Sync Oura' to fetch recent data");
      println!("  3. Run this script again");
    }
  }

  pool.close().await;
  println!("\n✅ Test complete");

  Ok(())
}
