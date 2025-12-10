use crate::analysis::{
  ContextPackage, HrZone, TrainingContext, TrainingFlags, UserSettings, WorkoutMetrics,
  WorkoutSummary,
};
use crate::llm::{ClaudeClient, LlmError};
use crate::db::AppState;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

/// ---------------------------------------------------------------------------
/// User Settings Commands
/// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_user_settings(
  state: State<'_, Arc<AppState>>,
) -> Result<UserSettings, String> {
  let row: Option<(Option<i64>, Option<i64>, Option<i64>, i64)> = sqlx::query_as(
    "SELECT max_hr, lthr, ftp, training_days_per_week FROM user_settings WHERE id = 1",
  )
  .fetch_optional(&state.db)
  .await
  .map_err(|e| format!("Failed to get settings: {}", e))?;

  match row {
    Some((max_hr, lthr, ftp, days)) => Ok(UserSettings {
      max_hr,
      lthr,
      ftp,
      training_days_per_week: days,
    }),
    None => Ok(UserSettings::default()),
  }
}

#[tauri::command]
pub async fn update_user_settings(
  state: State<'_, Arc<AppState>>,
  max_hr: Option<i64>,
  lthr: Option<i64>,
  ftp: Option<i64>,
  training_days_per_week: Option<i64>,
) -> Result<(), String> {
  sqlx::query(
    r#"
    UPDATE user_settings SET
      max_hr = COALESCE(?1, max_hr),
      lthr = COALESCE(?2, lthr),
      ftp = COALESCE(?3, ftp),
      training_days_per_week = COALESCE(?4, training_days_per_week),
      updated_at = CURRENT_TIMESTAMP
    WHERE id = 1
    "#,
  )
  .bind(max_hr)
  .bind(lthr)
  .bind(ftp)
  .bind(training_days_per_week)
  .execute(&state.db)
  .await
  .map_err(|e| format!("Failed to update settings: {}", e))?;

  Ok(())
}

/// ---------------------------------------------------------------------------
/// Compute Metrics for Workouts
/// ---------------------------------------------------------------------------

/// Compute and store metrics for all workouts that don't have them yet
#[tauri::command]
pub async fn compute_workout_metrics(
  state: State<'_, Arc<AppState>>,
) -> Result<ComputeResult, String> {
  // Get user settings
  let settings = get_user_settings(state.clone()).await?;

  // Find workouts without computed metrics
  let workouts: Vec<(i64, String, Option<i64>, Option<f64>, Option<i64>, Option<f64>)> =
    sqlx::query_as(
      r#"
      SELECT id, activity_type, duration_seconds, distance_meters,
             average_heartrate, average_watts
      FROM workouts
      WHERE metrics_computed_at IS NULL
      "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to fetch workouts: {}", e))?;

  let total = workouts.len();
  let mut computed = 0;

  for (id, activity_type, duration, distance, hr, watts) in workouts {
    let metrics = WorkoutMetrics::compute(
      &activity_type,
      duration,
      distance,
      hr,
      watts,
      &settings,
    );

    // Store computed metrics
    sqlx::query(
      r#"
      UPDATE workouts SET
        pace_min_per_km = ?1,
        speed_kmh = ?2,
        kj = ?3,
        rtss = ?4,
        efficiency = ?5,
        cardiac_cost = ?6,
        hr_zone = ?7,
        metrics_computed_at = ?8
      WHERE id = ?9
      "#,
    )
    .bind(metrics.pace_min_per_km)
    .bind(metrics.speed_kmh)
    .bind(metrics.kj)
    .bind(metrics.rtss)
    .bind(metrics.efficiency)
    .bind(metrics.cardiac_cost)
    .bind(metrics.hr_zone.map(|z| z.as_str()))
    .bind(Utc::now())
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to update workout {}: {}", id, e))?;

    computed += 1;
  }

  Ok(ComputeResult { total, computed })
}

#[derive(Serialize)]
pub struct ComputeResult {
  pub total: usize,
  pub computed: usize,
}

/// ---------------------------------------------------------------------------
/// Get Workout with Computed Metrics
/// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct WorkoutWithMetrics {
  pub id: i64,
  pub strava_id: String,
  pub activity_type: String,
  pub started_at: String,
  pub duration_seconds: Option<i64>,
  pub distance_meters: Option<f64>,
  pub average_heartrate: Option<i64>,
  pub average_watts: Option<f64>,
  pub suffer_score: Option<f64>,
  // Computed metrics
  pub pace_min_per_km: Option<f64>,
  pub speed_kmh: Option<f64>,
  pub kj: Option<f64>,
  pub rtss: Option<f64>,
  pub efficiency: Option<f64>,
  pub cardiac_cost: Option<f64>,
  pub hr_zone: Option<String>,
}

#[tauri::command]
pub async fn get_workouts_with_metrics(
  state: State<'_, Arc<AppState>>,
  limit: Option<i64>,
) -> Result<Vec<WorkoutWithMetrics>, String> {
  let limit = limit.unwrap_or(50);

  println!("Fetching workouts with limit: {}", limit);

  let rows: Vec<(
    i64, String, String, String, Option<i64>, Option<f64>,
    Option<i64>, Option<f64>, Option<f64>,
    Option<f64>, Option<f64>, Option<f64>, Option<f64>,
    Option<f64>, Option<f64>, Option<String>,
  )> = sqlx::query_as(
    r#"
    SELECT
      id, strava_id, activity_type, started_at,
      duration_seconds, CAST(distance_meters AS REAL), average_heartrate,
      CAST(average_watts AS REAL), CAST(suffer_score AS REAL),
      CAST(pace_min_per_km AS REAL), CAST(speed_kmh AS REAL), CAST(kj AS REAL),
      CAST(rtss AS REAL), CAST(efficiency AS REAL), CAST(cardiac_cost AS REAL), hr_zone
    FROM workouts
    ORDER BY started_at DESC
    LIMIT ?1
    "#,
  )
  .bind(limit)
  .fetch_all(&state.db)
  .await
  .map_err(|e| {
    println!("Query error: {}", e);
    format!("Failed to fetch workouts: {}", e)
  })?;

  println!("Fetched {} rows", rows.len());

  let workouts = rows
    .into_iter()
    .map(|(
      id, strava_id, activity_type, started_at,
      duration_seconds, distance_meters, average_heartrate, average_watts, suffer_score,
      pace_min_per_km, speed_kmh, kj, rtss, efficiency, cardiac_cost, hr_zone,
    )| WorkoutWithMetrics {
      id,
      strava_id,
      activity_type,
      started_at,
      duration_seconds,
      distance_meters,
      average_heartrate,
      average_watts,
      suffer_score,
      pace_min_per_km,
      speed_kmh,
      kj,
      rtss,
      efficiency,
      cardiac_cost,
      hr_zone,
    })
    .collect();

  Ok(workouts)
}

/// ---------------------------------------------------------------------------
/// Get Training Context (Tier 2 Rolling Metrics)
/// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_training_context(
  state: State<'_, Arc<AppState>>,
) -> Result<TrainingContext, String> {
  // Get user settings
  let settings = get_user_settings(state.clone()).await?;

  // Fetch workouts from last 42 days (needed for CTL calculation)
  let rows: Vec<(String, String, Option<i64>, Option<f64>, Option<String>)> = sqlx::query_as(
    r#"
    SELECT
      started_at,
      activity_type,
      duration_seconds,
      CAST(rtss AS REAL),
      hr_zone
    FROM workouts
    WHERE started_at >= datetime('now', '-42 days')
    ORDER BY started_at DESC
    "#,
  )
  .fetch_all(&state.db)
  .await
  .map_err(|e| format!("Failed to fetch workouts for context: {}", e))?;

  // Convert to WorkoutSummary
  let workouts: Vec<WorkoutSummary> = rows
    .into_iter()
    .filter_map(|(started_at, activity_type, duration_seconds, rtss, hr_zone)| {
      // Parse the started_at timestamp
      let dt = DateTime::parse_from_rfc3339(&started_at)
        .or_else(|_| DateTime::parse_from_str(&started_at, "%Y-%m-%dT%H:%M:%SZ"))
        .or_else(|_| DateTime::parse_from_str(&format!("{}+00:00", started_at), "%Y-%m-%d %H:%M:%S%:z"))
        .ok()?;

      let hr_zone_enum = hr_zone.as_ref().and_then(|z| match z.as_str() {
        "Z1" => Some(HrZone::Z1),
        "Z2" => Some(HrZone::Z2),
        "Z3" => Some(HrZone::Z3),
        "Z4" => Some(HrZone::Z4),
        "Z5" => Some(HrZone::Z5),
        _ => None,
      });

      Some(WorkoutSummary {
        started_at: dt.with_timezone(&Utc),
        activity_type,
        duration_seconds,
        rtss,
        hr_zone: hr_zone_enum,
      })
    })
    .collect();

  Ok(TrainingContext::compute(&workouts, &settings))
}

/// ---------------------------------------------------------------------------
/// LLM Workout Analysis Commands
/// ---------------------------------------------------------------------------

/// Error type that can be serialized for Tauri
#[derive(Debug, Serialize)]
pub struct AnalysisError {
  pub message: String,
}

impl From<LlmError> for AnalysisError {
  fn from(e: LlmError) -> Self {
    Self {
      message: e.to_string(),
    }
  }
}

impl From<String> for AnalysisError {
  fn from(s: String) -> Self {
    Self { message: s }
  }
}

/// Result of analyzing a workout with Claude
#[derive(Serialize)]
pub struct WorkoutAnalysisResult {
  pub workout_id: i64,
  pub analysis: StoredWorkoutAnalysis,
  pub input_tokens: u32,
  pub output_tokens: u32,
}

/// Stored analysis with ID for frontend
#[derive(Debug, Clone, Serialize)]
pub struct StoredWorkoutAnalysis {
  pub id: Option<i64>,
  pub workout_id: i64,
  pub summary: String,
  pub tomorrow_recommendation: String,
  pub risk_flags: Vec<String>,
  pub goal_notes: Option<String>,
  pub created_at: Option<String>,
}

/// Analyze a specific workout with Claude
#[tauri::command]
pub async fn analyze_workout(
  state: State<'_, Arc<AppState>>,
  workout_id: i64,
) -> Result<WorkoutAnalysisResult, AnalysisError> {
  // Get the workout data
  let workout: Option<(
    i64,
    String,
    String,
    Option<i64>,
    Option<f64>,
    Option<i64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<String>,
  )> = sqlx::query_as(
    r#"
    SELECT
      id, activity_type, started_at, duration_seconds,
      CAST(distance_meters AS REAL), average_heartrate,
      CAST(average_watts AS REAL), CAST(rtss AS REAL),
      CAST(pace_min_per_km AS REAL), hr_zone
    FROM workouts
    WHERE id = ?1
    "#,
  )
  .bind(workout_id)
  .fetch_optional(&state.db)
  .await
  .map_err(|e| AnalysisError::from(format!("Failed to fetch workout: {}", e)))?;

  let (
    _id,
    activity_type,
    started_at_str,
    duration_seconds,
    distance_meters,
    average_hr,
    average_watts,
    rtss,
    pace_min_per_km,
    hr_zone,
  ) = workout.ok_or_else(|| AnalysisError::from("Workout not found".to_string()))?;

  // Parse the started_at timestamp
  let started_at = DateTime::parse_from_rfc3339(&started_at_str)
    .or_else(|_| DateTime::parse_from_str(&started_at_str, "%Y-%m-%dT%H:%M:%SZ"))
    .map(|dt| dt.with_timezone(&Utc))
    .map_err(|e| AnalysisError::from(format!("Failed to parse date: {}", e)))?;

  // Get user settings
  let settings = get_user_settings(state.clone())
    .await
    .map_err(AnalysisError::from)?;

  // Reconstruct metrics (we stored them, but need WorkoutMetrics for the package)
  let metrics = WorkoutMetrics {
    pace_min_per_km,
    speed_kmh: None,
    kj: None,
    rtss,
    efficiency: None,
    cardiac_cost: None,
    hr_zone: hr_zone.as_ref().and_then(|z| match z.as_str() {
      "Z1" => Some(HrZone::Z1),
      "Z2" => Some(HrZone::Z2),
      "Z3" => Some(HrZone::Z3),
      "Z4" => Some(HrZone::Z4),
      "Z5" => Some(HrZone::Z5),
      _ => None,
    }),
  };

  // Get training context (includes all workouts for rolling calculations)
  let training_context = get_training_context(state.clone())
    .await
    .map_err(AnalysisError::from)?;

  // Get all workouts for flag computation
  let workouts_for_flags = get_workout_summaries(&state.db)
    .await
    .map_err(|e| AnalysisError::from(format!("Failed to get workout summaries: {}", e)))?;

  // Compute flags
  let flags = TrainingFlags::compute(&workouts_for_flags, &training_context, &settings);

  // Build context package
  let context_package = ContextPackage::build(
    &activity_type,
    &started_at,
    duration_seconds,
    distance_meters,
    average_hr,
    average_watts,
    &metrics,
    training_context,
    flags,
    &settings,
  );

  // Call Claude
  let client = ClaudeClient::from_env()?;
  let (analysis, usage) = client.analyze_workout(&context_package.to_json()).await?;

  // Store the analysis
  let risk_flags_json = serde_json::to_string(&analysis.risk_flags).unwrap_or_default();

  sqlx::query(
    r#"
    INSERT INTO workout_analysis (
      workout_id, summary, tomorrow_recommendation, risk_flags_json,
      goal_notes, model_version, input_tokens, output_tokens
    )
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
    ON CONFLICT(workout_id) DO UPDATE SET
      summary = excluded.summary,
      tomorrow_recommendation = excluded.tomorrow_recommendation,
      risk_flags_json = excluded.risk_flags_json,
      goal_notes = excluded.goal_notes,
      model_version = excluded.model_version,
      input_tokens = excluded.input_tokens,
      output_tokens = excluded.output_tokens,
      created_at = CURRENT_TIMESTAMP
    "#,
  )
  .bind(workout_id)
  .bind(&analysis.summary)
  .bind(&analysis.tomorrow_recommendation)
  .bind(&risk_flags_json)
  .bind(&analysis.goal_notes)
  .bind("claude-sonnet-4-20250514")
  .bind(usage.input_tokens as i64)
  .bind(usage.output_tokens as i64)
  .execute(&state.db)
  .await
  .map_err(|e| AnalysisError::from(format!("Failed to store analysis: {}", e)))?;

  println!(
    "Analyzed workout {}: {} tokens in, {} tokens out",
    workout_id, usage.input_tokens, usage.output_tokens
  );

  Ok(WorkoutAnalysisResult {
    workout_id,
    analysis: StoredWorkoutAnalysis {
      id: None,
      workout_id,
      summary: analysis.summary,
      tomorrow_recommendation: analysis.tomorrow_recommendation,
      risk_flags: analysis.risk_flags,
      goal_notes: analysis.goal_notes,
      created_at: None,
    },
    input_tokens: usage.input_tokens,
    output_tokens: usage.output_tokens,
  })
}

/// Get stored analysis for a workout
#[tauri::command]
pub async fn get_workout_analysis(
  state: State<'_, Arc<AppState>>,
  workout_id: i64,
) -> Result<Option<StoredWorkoutAnalysis>, String> {
  let row: Option<(i64, i64, String, String, Option<String>, Option<String>, String)> =
    sqlx::query_as(
      r#"
      SELECT id, workout_id, summary, tomorrow_recommendation,
             risk_flags_json, goal_notes, created_at
      FROM workout_analysis
      WHERE workout_id = ?1
      "#,
    )
    .bind(workout_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("Failed to fetch analysis: {}", e))?;

  match row {
    Some((id, wid, summary, rec, flags_json, notes, created)) => {
      let risk_flags: Vec<String> = flags_json
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

      Ok(Some(StoredWorkoutAnalysis {
        id: Some(id),
        workout_id: wid,
        summary,
        tomorrow_recommendation: rec,
        risk_flags,
        goal_notes: notes,
        created_at: Some(created),
      }))
    }
    None => Ok(None),
  }
}

/// Get the latest workout analysis (most recent workout that has an analysis)
#[tauri::command]
pub async fn get_latest_analysis(
  state: State<'_, Arc<AppState>>,
) -> Result<Option<StoredWorkoutAnalysis>, String> {
  let row: Option<(i64, i64, String, String, Option<String>, Option<String>, String)> =
    sqlx::query_as(
      r#"
      SELECT wa.id, wa.workout_id, wa.summary, wa.tomorrow_recommendation,
             wa.risk_flags_json, wa.goal_notes, wa.created_at
      FROM workout_analysis wa
      JOIN workouts w ON w.id = wa.workout_id
      ORDER BY w.started_at DESC
      LIMIT 1
      "#,
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("Failed to fetch analysis: {}", e))?;

  match row {
    Some((id, wid, summary, rec, flags_json, notes, created)) => {
      let risk_flags: Vec<String> = flags_json
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

      Ok(Some(StoredWorkoutAnalysis {
        id: Some(id),
        workout_id: wid,
        summary,
        tomorrow_recommendation: rec,
        risk_flags,
        goal_notes: notes,
        created_at: Some(created),
      }))
    }
    None => Ok(None),
  }
}

/// Helper: Get workout summaries for flag computation
async fn get_workout_summaries(
  db: &crate::db::DbPool,
) -> Result<Vec<WorkoutSummary>, sqlx::Error> {
  let rows: Vec<(String, String, Option<i64>, Option<f64>, Option<String>)> = sqlx::query_as(
    r#"
    SELECT started_at, activity_type, duration_seconds,
           CAST(rtss AS REAL), hr_zone
    FROM workouts
    WHERE started_at >= datetime('now', '-42 days')
    ORDER BY started_at DESC
    "#,
  )
  .fetch_all(db)
  .await?;

  let workouts: Vec<WorkoutSummary> = rows
    .into_iter()
    .filter_map(|(started_at, activity_type, duration_seconds, rtss, hr_zone)| {
      let dt = DateTime::parse_from_rfc3339(&started_at)
        .or_else(|_| DateTime::parse_from_str(&started_at, "%Y-%m-%dT%H:%M:%SZ"))
        .or_else(|_| {
          DateTime::parse_from_str(&format!("{}+00:00", started_at), "%Y-%m-%d %H:%M:%S%:z")
        })
        .ok()?;

      let hr_zone_enum = hr_zone.as_ref().and_then(|z| match z.as_str() {
        "Z1" => Some(HrZone::Z1),
        "Z2" => Some(HrZone::Z2),
        "Z3" => Some(HrZone::Z3),
        "Z4" => Some(HrZone::Z4),
        "Z5" => Some(HrZone::Z5),
        _ => None,
      });

      Some(WorkoutSummary {
        started_at: dt.with_timezone(&Utc),
        activity_type,
        duration_seconds,
        rtss,
        hr_zone: hr_zone_enum,
      })
    })
    .collect();

  Ok(workouts)
}
