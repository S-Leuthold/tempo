//! Ceiling-Based Progression Engine
//!
//! Dimension-agnostic progression tracking where each dimension has:
//! - current value
//! - ceiling (max target for the goal)
//! - step configuration (sequence or increment)
//! - lifecycle status (building, at_ceiling, regressing)
//!
//! Key principles:
//! - Criteria-driven, not calendar-driven
//! - Ceilings prevent runaway progression
//! - No compensatory volume - miss days = hold or regress
//! - Cycling is regulated (TSB-based duration), not progressive

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::analysis::{TrainingContext, TrainingFlags};

#[cfg(test)]
use chrono::Duration;

// ---------------------------------------------------------------------------
/// Dimension Type: Progressive vs Regulated
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionType {
    /// Progresses toward ceiling when criteria met (run intervals, long runs)
    Progressive,
    /// Duration regulated by TSB, power drifts naturally (cycling)
    Regulated,
}

// ---------------------------------------------------------------------------
/// Lifecycle Status: Where are we in the progression journey
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum LifecycleStatus {
    /// current < ceiling, working toward goal
    #[default]
    Building,
    /// current == ceiling, maintaining capability
    AtCeiling,
    /// Detraining detected, stepping back to rebuild
    Regressing,
}


impl std::fmt::Display for LifecycleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Building => write!(f, "building"),
            Self::AtCeiling => write!(f, "at_ceiling"),
            Self::Regressing => write!(f, "regressing"),
        }
    }
}

impl std::str::FromStr for LifecycleStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "building" => Ok(Self::Building),
            "at_ceiling" => Ok(Self::AtCeiling),
            "regressing" => Ok(Self::Regressing),
            _ => Err(format!("Unknown lifecycle status: {}", s)),
        }
    }
}

// ---------------------------------------------------------------------------
/// Step Configuration: How to progress values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepConfig {
    /// Discrete sequence: ["4:1", "5:1", "6:1", ...]
    Sequence { sequence: Vec<String> },
    /// Linear increment: current + increment
    Increment { increment: i32, unit: String },
    /// Regulated: no progression, duration selected by TSB
    Regulated { options: Vec<i32>, unit: String },
}

impl StepConfig {
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse step config: {}", e))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Get the next value in the progression
    pub fn next_value(&self, current: &str) -> Option<String> {
        match self {
            StepConfig::Sequence { sequence } => {
                let idx = sequence.iter().position(|v| v == current)?;
                sequence.get(idx + 1).cloned()
            }
            StepConfig::Increment { increment, .. } => {
                let current_val: i32 = current.parse().ok()?;
                Some((current_val + increment).to_string())
            }
            StepConfig::Regulated { .. } => None, // No progression for regulated
        }
    }

    /// Get the previous value (for regression)
    pub fn prev_value(&self, current: &str) -> Option<String> {
        match self {
            StepConfig::Sequence { sequence } => {
                let idx = sequence.iter().position(|v| v == current)?;
                if idx > 0 {
                    sequence.get(idx - 1).cloned()
                } else {
                    None
                }
            }
            StepConfig::Increment { increment, .. } => {
                let current_val: i32 = current.parse().ok()?;
                let prev = current_val - increment;
                if prev > 0 {
                    Some(prev.to_string())
                } else {
                    None
                }
            }
            StepConfig::Regulated { .. } => None,
        }
    }

    /// Check if value is at or beyond ceiling
    pub fn is_at_ceiling(&self, current: &str, ceiling: &str) -> bool {
        match self {
            StepConfig::Sequence { sequence } => {
                let current_idx = sequence.iter().position(|v| v == current);
                let ceiling_idx = sequence.iter().position(|v| v == ceiling);
                match (current_idx, ceiling_idx) {
                    (Some(c), Some(ceil)) => c >= ceil,
                    _ => current == ceiling,
                }
            }
            StepConfig::Increment { .. } => {
                let current_val: i32 = current.parse().unwrap_or(0);
                let ceiling_val: i32 = ceiling.parse().unwrap_or(i32::MAX);
                current_val >= ceiling_val
            }
            StepConfig::Regulated { .. } => true, // Regulated is always "at ceiling"
        }
    }

    /// Get regulated duration based on TSB
    pub fn get_regulated_duration(&self, tsb: Option<f64>) -> Option<i32> {
        match self {
            StepConfig::Regulated { options, .. } => {
                let tsb_val = tsb.unwrap_or(0.0);
                if options.len() >= 2 {
                    if tsb_val >= 0.0 {
                        // Fresh: longest duration
                        options.last().copied()
                    } else if tsb_val >= -10.0 {
                        // Moderate fatigue: shorter duration
                        options.first().copied()
                    } else {
                        // High fatigue: recovery spin (30-40 min or first option)
                        Some(options.first().copied().unwrap_or(30).min(40))
                    }
                } else {
                    options.first().copied()
                }
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
/// Progression Dimension: Generic dimension from database
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressionDimension {
    pub id: i64,
    pub name: String,
    pub current_value: String,
    pub ceiling_value: String,
    pub step_config: StepConfig,
    pub status: LifecycleStatus,
    pub last_change_at: Option<DateTime<Utc>>,
    pub last_ceiling_touch_at: Option<DateTime<Utc>>,
    pub maintenance_cadence_days: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ProgressionDimension {
    /// Get the type of this dimension
    pub fn dimension_type(&self) -> DimensionType {
        match &self.step_config {
            StepConfig::Regulated { .. } => DimensionType::Regulated,
            _ => DimensionType::Progressive,
        }
    }

    /// Check if this dimension is at its ceiling
    pub fn is_at_ceiling(&self) -> bool {
        self.step_config.is_at_ceiling(&self.current_value, &self.ceiling_value)
    }

    /// Get next progression value (None if at ceiling or regulated)
    pub fn next_value(&self) -> Option<String> {
        if self.is_at_ceiling() {
            None
        } else {
            self.step_config.next_value(&self.current_value)
        }
    }

    /// Get previous value for regression
    pub fn prev_value(&self) -> Option<String> {
        self.step_config.prev_value(&self.current_value)
    }

    /// Check if maintenance is due (at ceiling and haven't touched in cadence period)
    pub fn maintenance_due(&self) -> bool {
        if self.status != LifecycleStatus::AtCeiling {
            return false;
        }
        match self.last_ceiling_touch_at {
            Some(last_touch) => {
                let days_since = (Utc::now() - last_touch).num_days();
                days_since >= self.maintenance_cadence_days as i64
            }
            None => true, // Never touched ceiling, maintenance due
        }
    }

    /// Check if regression is warranted (at ceiling but haven't touched in 21+ days)
    pub fn should_regress(&self) -> bool {
        if self.status != LifecycleStatus::AtCeiling {
            return false;
        }
        match self.last_ceiling_touch_at {
            Some(last_touch) => {
                let days_since = (Utc::now() - last_touch).num_days();
                days_since >= 21 // 3 weeks without ceiling touch = regression
            }
            None => false, // Can't regress if we've never reached ceiling
        }
    }

    /// Days since last change
    pub fn days_since_change(&self) -> i64 {
        self.last_change_at
            .map(|d| (Utc::now() - d).num_days())
            .unwrap_or(30)
    }

    /// Get regulated duration for cycling based on TSB
    pub fn get_regulated_duration(&self, tsb: Option<f64>) -> Option<i32> {
        self.step_config.get_regulated_duration(tsb)
    }
}

// ---------------------------------------------------------------------------
/// Adherence Summary (preserved from original)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdherenceSummary {
    pub total_expected: u8,
    pub total_completed: u8,
    pub key_expected: u8,
    pub key_completed: u8,
    pub adherence_pct: f32,
    pub key_adherence_good: bool,
    pub week_stable: bool,
    pub missed_workouts: u8,
    pub consecutive_low_adherence_weeks: u8,
}

impl AdherenceSummary {
    pub fn compute(
        total_expected: u8,
        total_completed: u8,
        key_expected: u8,
        key_completed: u8,
        consecutive_low_weeks: u8,
    ) -> Self {
        let adherence_pct = if total_expected > 0 {
            total_completed as f32 / total_expected as f32
        } else {
            1.0
        };

        let key_adherence_good = key_completed >= key_expected;
        let week_stable = adherence_pct >= 0.75 && key_adherence_good;
        let missed_workouts = total_expected.saturating_sub(total_completed);

        Self {
            total_expected,
            total_completed,
            key_expected,
            key_completed,
            adherence_pct,
            key_adherence_good,
            week_stable,
            missed_workouts,
            consecutive_low_adherence_weeks: consecutive_low_weeks,
        }
    }

    pub fn is_unstable(&self) -> bool {
        self.adherence_pct < 0.7
    }

    pub fn should_consider_regression(&self) -> bool {
        self.consecutive_low_adherence_weeks >= 2
    }
}

impl Default for AdherenceSummary {
    fn default() -> Self {
        Self {
            total_expected: 6,
            total_completed: 6,
            key_expected: 3,
            key_completed: 3,
            adherence_pct: 1.0,
            key_adherence_good: true,
            week_stable: true,
            missed_workouts: 0,
            consecutive_low_adherence_weeks: 0,
        }
    }
}

// ---------------------------------------------------------------------------
/// Engine Decision: What Rust allows
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineDecision {
    ProgressAllowed,           // Criteria met, can advance
    AtCeiling,                 // At max, maintenance mode
    MaintenanceDue,            // At ceiling but need to touch it soon
    HoldForNow,                // Criteria met but blocked by overlap rule
    HoldDueToUnstableWeek,     // Week had < 70% adherence
    HoldDueToMissedKeySession, // Key session missed
    Hold,                      // Criteria not met
    Regress,                   // Step back due to detraining
    Regulated,                 // Dimension is regulated, not progressive
}

// ---------------------------------------------------------------------------
/// Dimension Status: Status for one progression track
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionStatus {
    pub name: String,
    pub dimension_type: DimensionType,
    pub current: String,
    pub ceiling: String,
    pub status: LifecycleStatus,
    pub engine_decision: EngineDecision,
    pub reason: String,
    pub next_value: Option<String>,
    pub days_since_change: i64,
    pub maintenance_due: bool,
    /// For regulated dimensions: recommended duration based on TSB
    pub regulated_duration: Option<i32>,
}

// ---------------------------------------------------------------------------
/// Progression Summary: Sent to LLM
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressionSummary {
    pub dimensions: Vec<DimensionStatus>,
    pub last_progression_dimension: Option<String>,
    pub days_since_any_progression: i64,
    pub adherence: AdherenceSummary,
}

impl ProgressionSummary {
    /// Compute progression summary for all dimensions
    pub fn compute(
        dimensions: &[ProgressionDimension],
        context: &TrainingContext,
        flags: &TrainingFlags,
        adherence: AdherenceSummary,
    ) -> Self {
        // Find most recent progression (for overlap rule)
        let most_recent = dimensions
            .iter()
            .filter_map(|d| d.last_change_at.map(|dt| (d.name.clone(), dt)))
            .max_by_key(|(_, dt)| *dt);

        let last_progression_dimension = most_recent.as_ref().map(|(name, _)| name.clone());
        let days_since_any_progression = most_recent
            .map(|(_, dt)| (Utc::now() - dt).num_days())
            .unwrap_or(30);

        // Build status for each dimension
        let dimension_statuses: Vec<DimensionStatus> = dimensions
            .iter()
            .map(|dim| {
                Self::build_dimension_status(
                    dim,
                    context,
                    flags,
                    &adherence,
                    &last_progression_dimension,
                    days_since_any_progression,
                )
            })
            .collect();

        Self {
            dimensions: dimension_statuses,
            last_progression_dimension,
            days_since_any_progression,
            adherence,
        }
    }

    fn build_dimension_status(
        dim: &ProgressionDimension,
        context: &TrainingContext,
        flags: &TrainingFlags,
        adherence: &AdherenceSummary,
        last_prog_dim: &Option<String>,
        days_since_any: i64,
    ) -> DimensionStatus {
        let dim_type = dim.dimension_type();

        // For regulated dimensions (cycling), just report current state
        if dim_type == DimensionType::Regulated {
            let regulated_duration = dim.get_regulated_duration(context.tsb);
            let tsb_desc = match context.tsb {
                Some(t) if t >= 0.0 => "fresh",
                Some(t) if t >= -10.0 => "moderate fatigue",
                Some(_) => "fatigued",
                None => "unknown fatigue",
            };

            return DimensionStatus {
                name: dim.name.clone(),
                dimension_type: dim_type,
                current: dim.current_value.clone(),
                ceiling: dim.ceiling_value.clone(),
                status: LifecycleStatus::AtCeiling, // Regulated = always at "ceiling"
                engine_decision: EngineDecision::Regulated,
                reason: format!(
                    "Duration regulated by TSB ({}): {} min recommended",
                    tsb_desc,
                    regulated_duration.unwrap_or(45)
                ),
                next_value: None,
                days_since_change: dim.days_since_change(),
                maintenance_due: false,
                regulated_duration,
            };
        }

        // Progressive dimension logic
        let is_at_ceiling = dim.is_at_ceiling();
        let maintenance_due = dim.maintenance_due();
        let should_regress = dim.should_regress();

        // Check dimension-specific criteria
        let (criteria_met, criteria_reason) =
            Self::check_criteria(&dim.name, dim, context, flags);

        // Apply overlap rule: if another dimension progressed in last 7 days, hold
        let overlap_blocked = last_prog_dim.as_ref().is_some_and(|last| {
            last != &dim.name && days_since_any < 7
        });

        // Determine engine decision
        let (engine_decision, reason) = if should_regress {
            (
                EngineDecision::Regress,
                format!(
                    "Haven't touched ceiling in {} days, stepping back",
                    dim.last_ceiling_touch_at
                        .map(|d| (Utc::now() - d).num_days())
                        .unwrap_or(0)
                ),
            )
        } else if adherence.should_consider_regression() && dim.prev_value().is_some() {
            (
                EngineDecision::Regress,
                format!(
                    "{} consecutive low-adherence weeks",
                    adherence.consecutive_low_adherence_weeks
                ),
            )
        } else if is_at_ceiling {
            if maintenance_due {
                (
                    EngineDecision::MaintenanceDue,
                    format!(
                        "At ceiling ({}), maintenance due - touch this level soon",
                        dim.ceiling_value
                    ),
                )
            } else {
                (
                    EngineDecision::AtCeiling,
                    format!("At ceiling ({}), maintaining", dim.ceiling_value),
                )
            }
        } else if adherence.is_unstable() {
            (
                EngineDecision::HoldDueToUnstableWeek,
                format!(
                    "Week had {}% adherence (need 70%)",
                    (adherence.adherence_pct * 100.0) as i32
                ),
            )
        } else if !adherence.key_adherence_good && is_key_session_dimension(&dim.name) {
            (
                EngineDecision::HoldDueToMissedKeySession,
                "Key session missed this week".to_string(),
            )
        } else if overlap_blocked {
            (
                EngineDecision::HoldForNow,
                format!(
                    "Another dimension progressed {} days ago (need 7)",
                    days_since_any
                ),
            )
        } else if !criteria_met {
            (EngineDecision::Hold, criteria_reason)
        } else if !adherence.week_stable {
            (
                EngineDecision::HoldForNow,
                "Week not stable (missed sessions or low adherence)".to_string(),
            )
        } else {
            (EngineDecision::ProgressAllowed, "All criteria met".to_string())
        };

        DimensionStatus {
            name: dim.name.clone(),
            dimension_type: dim_type,
            current: dim.current_value.clone(),
            ceiling: dim.ceiling_value.clone(),
            status: dim.status,
            engine_decision,
            reason,
            next_value: dim.next_value(),
            days_since_change: dim.days_since_change(),
            maintenance_due,
            regulated_duration: None,
        }
    }

    /// Check dimension-specific criteria
    fn check_criteria(
        name: &str,
        dim: &ProgressionDimension,
        context: &TrainingContext,
        flags: &TrainingFlags,
    ) -> (bool, String) {
        let days_since_change = dim.days_since_change();
        let min_days = 7;

        let volume_stable = !flags.volume_spike && !flags.volume_drop;

        // Fatigue thresholds vary by dimension
        let (fatigue_low, fatigue_threshold) = match name {
            "run_interval" => (context.tsb.is_none_or(|t| t > -15.0), -15.0),
            "long_run" => (context.tsb.is_none_or(|t| t > -15.0), -15.0),
            _ => (context.tsb.is_none_or(|t| t > -10.0), -10.0),
        };

        // HR stability matters more for run intervals
        let hr_stability = if name == "run_interval" {
            !flags.intensity_heavy
        } else {
            true
        };

        let criteria_met =
            days_since_change >= min_days && volume_stable && fatigue_low && hr_stability;

        if criteria_met {
            (true, "All criteria met".to_string())
        } else {
            let mut reasons = Vec::new();
            if days_since_change < min_days {
                reasons.push(format!(
                    "{} days since last change (need {})",
                    days_since_change, min_days
                ));
            }
            if !volume_stable {
                reasons.push("volume unstable".to_string());
            }
            if !fatigue_low {
                reasons.push(format!(
                    "TSB too low ({:.1}, need > {:.1})",
                    context.tsb.unwrap_or(0.0),
                    fatigue_threshold
                ));
            }
            if !hr_stability {
                reasons.push("HR/intensity unstable".to_string());
            }
            (false, reasons.join(", "))
        }
    }

    /// Get status for a specific dimension by name
    #[allow(dead_code)]
    pub fn get_dimension(&self, name: &str) -> Option<&DimensionStatus> {
        self.dimensions.iter().find(|d| d.name == name)
    }
}

/// Check if a dimension is a "key session" for adherence purposes
fn is_key_session_dimension(name: &str) -> bool {
    matches!(name, "long_run")
}

// ---------------------------------------------------------------------------
// Database Operations
// ---------------------------------------------------------------------------

/// Load all progression dimensions from database
pub async fn load_all_dimensions(pool: &SqlitePool) -> Result<Vec<ProgressionDimension>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
            id, name, current_value, ceiling_value, step_config_json,
            status, last_change_at, last_ceiling_touch_at,
            maintenance_cadence_days, created_at, updated_at
        FROM progression_dimensions
        ORDER BY id
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to load dimensions: {}", e))?;

    let mut dimensions = Vec::new();
    for row in rows {
        let step_config_json: String = row.get("step_config_json");
        let step_config = StepConfig::from_json(&step_config_json)?;
        let status_str: String = row.get("status");
        let status: LifecycleStatus = status_str.parse().unwrap_or_default();

        let last_change_at: Option<String> = row.get("last_change_at");
        let last_ceiling_touch_at: Option<String> = row.get("last_ceiling_touch_at");
        let created_at: Option<String> = row.get("created_at");
        let updated_at: Option<String> = row.get("updated_at");

        dimensions.push(ProgressionDimension {
            id: row.get("id"),
            name: row.get("name"),
            current_value: row.get("current_value"),
            ceiling_value: row.get("ceiling_value"),
            step_config,
            status,
            last_change_at: last_change_at.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
            last_ceiling_touch_at: last_ceiling_touch_at.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
            maintenance_cadence_days: row
                .try_get::<i32, _>("maintenance_cadence_days")
                .unwrap_or(14),
            created_at: created_at
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            updated_at: updated_at
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        });
    }

    Ok(dimensions)
}

/// Load a single dimension by name
pub async fn load_dimension(
    pool: &SqlitePool,
    name: &str,
) -> Result<ProgressionDimension, String> {
    let dimensions = load_all_dimensions(pool).await?;
    dimensions
        .into_iter()
        .find(|d| d.name == name)
        .ok_or_else(|| format!("Dimension not found: {}", name))
}

/// Save a dimension back to database
pub async fn save_dimension(pool: &SqlitePool, dim: &ProgressionDimension) -> Result<(), String> {
    let step_config_json = dim.step_config.to_json();
    let status_str = dim.status.to_string();
    let last_change_str = dim.last_change_at.map(|d| d.to_rfc3339());
    let last_ceiling_str = dim.last_ceiling_touch_at.map(|d| d.to_rfc3339());
    let updated_at = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        UPDATE progression_dimensions
        SET current_value = ?,
            ceiling_value = ?,
            step_config_json = ?,
            status = ?,
            last_change_at = ?,
            last_ceiling_touch_at = ?,
            maintenance_cadence_days = ?,
            updated_at = ?
        WHERE name = ?
        "#,
    )
    .bind(&dim.current_value)
    .bind(&dim.ceiling_value)
    .bind(&step_config_json)
    .bind(&status_str)
    .bind(&last_change_str)
    .bind(&last_ceiling_str)
    .bind(dim.maintenance_cadence_days)
    .bind(&updated_at)
    .bind(&dim.name)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to save dimension: {}", e))?;

    Ok(())
}

/// Log a progression change to history
pub async fn log_progression(
    pool: &SqlitePool,
    dimension_name: &str,
    previous_value: &str,
    new_value: &str,
    change_type: &str,
    trigger_workout_id: Option<i64>,
    context_json: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO progression_history
            (dimension_name, previous_value, new_value, change_type, trigger_workout_id, context_snapshot_json)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(dimension_name)
    .bind(previous_value)
    .bind(new_value)
    .bind(change_type)
    .bind(trigger_workout_id)
    .bind(context_json)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to log progression: {}", e))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Progression Actions
// ---------------------------------------------------------------------------

/// Apply a progression to a dimension
pub async fn apply_progression(
    pool: &SqlitePool,
    dimension_name: &str,
    trigger_workout_id: Option<i64>,
) -> Result<String, String> {
    let mut dim = load_dimension(pool, dimension_name).await?;

    let next_val = dim
        .next_value()
        .ok_or_else(|| format!("No next value available for {}", dimension_name))?;

    let prev_val = dim.current_value.clone();
    dim.current_value = next_val.clone();
    dim.last_change_at = Some(Utc::now());

    // Update status if we've reached ceiling
    if dim.is_at_ceiling() {
        dim.status = LifecycleStatus::AtCeiling;
        dim.last_ceiling_touch_at = Some(Utc::now());
    }

    save_dimension(pool, &dim).await?;
    log_progression(
        pool,
        dimension_name,
        &prev_val,
        &next_val,
        "progress",
        trigger_workout_id,
        None,
    )
    .await?;

    Ok(next_val)
}

/// Record a ceiling touch (maintenance workout)
pub async fn record_ceiling_touch(pool: &SqlitePool, dimension_name: &str) -> Result<(), String> {
    let mut dim = load_dimension(pool, dimension_name).await?;

    if dim.status != LifecycleStatus::AtCeiling {
        return Err(format!("{} is not at ceiling", dimension_name));
    }

    dim.last_ceiling_touch_at = Some(Utc::now());
    save_dimension(pool, &dim).await?;

    log_progression(
        pool,
        dimension_name,
        &dim.current_value,
        &dim.current_value,
        "ceiling_touch",
        None,
        None,
    )
    .await?;

    Ok(())
}

/// Apply regression to a dimension
pub async fn apply_regression(pool: &SqlitePool, dimension_name: &str) -> Result<String, String> {
    let mut dim = load_dimension(pool, dimension_name).await?;

    let prev_val = dim
        .prev_value()
        .ok_or_else(|| format!("No previous value available for {}", dimension_name))?;

    let old_val = dim.current_value.clone();
    dim.current_value = prev_val.clone();
    dim.last_change_at = Some(Utc::now());
    dim.status = LifecycleStatus::Building; // Back to building

    save_dimension(pool, &dim).await?;
    log_progression(
        pool,
        dimension_name,
        &old_val,
        &prev_val,
        "regress",
        None,
        None,
    )
    .await?;

    Ok(prev_val)
}

/// Update ceiling for a dimension
pub async fn update_ceiling(
    pool: &SqlitePool,
    dimension_name: &str,
    new_ceiling: &str,
) -> Result<(), String> {
    let mut dim = load_dimension(pool, dimension_name).await?;

    let old_ceiling = dim.ceiling_value.clone();
    dim.ceiling_value = new_ceiling.to_string();

    // Re-evaluate status
    if dim.is_at_ceiling() {
        dim.status = LifecycleStatus::AtCeiling;
    } else {
        dim.status = LifecycleStatus::Building;
    }

    save_dimension(pool, &dim).await?;
    log_progression(
        pool,
        dimension_name,
        &old_ceiling,
        new_ceiling,
        "ceiling_update",
        None,
        None,
    )
    .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
/// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sequence_dimension(current: &str, ceiling: &str) -> ProgressionDimension {
        ProgressionDimension {
            id: 1,
            name: "run_interval".to_string(),
            current_value: current.to_string(),
            ceiling_value: ceiling.to_string(),
            step_config: StepConfig::Sequence {
                sequence: vec![
                    "4:1".to_string(),
                    "5:1".to_string(),
                    "6:1".to_string(),
                    "8:1".to_string(),
                    "10:1".to_string(),
                    "continuous_20".to_string(),
                    "continuous_30".to_string(),
                    "continuous_45".to_string(),
                ],
            },
            status: LifecycleStatus::Building,
            last_change_at: Some(Utc::now() - Duration::days(10)),
            last_ceiling_touch_at: None,
            maintenance_cadence_days: 7,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_increment_dimension(current: i32, ceiling: i32) -> ProgressionDimension {
        ProgressionDimension {
            id: 2,
            name: "long_run".to_string(),
            current_value: current.to_string(),
            ceiling_value: ceiling.to_string(),
            step_config: StepConfig::Increment {
                increment: 5,
                unit: "min".to_string(),
            },
            status: LifecycleStatus::Building,
            last_change_at: Some(Utc::now() - Duration::days(10)),
            last_ceiling_touch_at: None,
            maintenance_cadence_days: 14,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_regulated_dimension() -> ProgressionDimension {
        ProgressionDimension {
            id: 3,
            name: "z2_ride".to_string(),
            current_value: "45".to_string(),
            ceiling_value: "60".to_string(),
            step_config: StepConfig::Regulated {
                options: vec![45, 60],
                unit: "min".to_string(),
            },
            status: LifecycleStatus::AtCeiling,
            last_change_at: None,
            last_ceiling_touch_at: None,
            maintenance_cadence_days: 10,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_sequence_progression() {
        let dim = make_sequence_dimension("4:1", "continuous_45");
        assert_eq!(dim.next_value(), Some("5:1".to_string()));
        assert!(!dim.is_at_ceiling());
    }

    #[test]
    fn test_sequence_at_ceiling() {
        let dim = make_sequence_dimension("continuous_45", "continuous_45");
        assert!(dim.is_at_ceiling());
        assert_eq!(dim.next_value(), None);
    }

    #[test]
    fn test_increment_progression() {
        let dim = make_increment_dimension(30, 90);
        assert_eq!(dim.next_value(), Some("35".to_string()));
        assert!(!dim.is_at_ceiling());
    }

    #[test]
    fn test_increment_at_ceiling() {
        let dim = make_increment_dimension(90, 90);
        assert!(dim.is_at_ceiling());
        assert_eq!(dim.next_value(), None);
    }

    #[test]
    fn test_regulated_no_progression() {
        let dim = make_regulated_dimension();
        assert_eq!(dim.dimension_type(), DimensionType::Regulated);
        assert_eq!(dim.next_value(), None);
    }

    #[test]
    fn test_regulated_tsb_duration() {
        let dim = make_regulated_dimension();

        // Fresh (TSB >= 0): longest duration
        assert_eq!(dim.get_regulated_duration(Some(5.0)), Some(60));

        // Moderate fatigue (TSB -10 to 0): shorter
        assert_eq!(dim.get_regulated_duration(Some(-5.0)), Some(45));

        // High fatigue (TSB < -10): recovery
        assert_eq!(dim.get_regulated_duration(Some(-15.0)), Some(40));
    }

    #[test]
    fn test_maintenance_due() {
        let mut dim = make_sequence_dimension("continuous_45", "continuous_45");
        dim.status = LifecycleStatus::AtCeiling;
        dim.last_ceiling_touch_at = Some(Utc::now() - Duration::days(10));

        // Maintenance cadence is 7 days, 10 days since touch = due
        assert!(dim.maintenance_due());
    }

    #[test]
    fn test_should_regress() {
        let mut dim = make_sequence_dimension("continuous_45", "continuous_45");
        dim.status = LifecycleStatus::AtCeiling;
        dim.last_ceiling_touch_at = Some(Utc::now() - Duration::days(25));

        // 25 days since touch > 21 day threshold
        assert!(dim.should_regress());
    }

    #[test]
    fn test_prev_value_sequence() {
        let dim = make_sequence_dimension("6:1", "continuous_45");
        assert_eq!(dim.prev_value(), Some("5:1".to_string()));
    }

    #[test]
    fn test_prev_value_at_start() {
        let dim = make_sequence_dimension("4:1", "continuous_45");
        assert_eq!(dim.prev_value(), None);
    }

    #[test]
    fn test_step_config_json_roundtrip() {
        let config = StepConfig::Sequence {
            sequence: vec!["4:1".to_string(), "5:1".to_string()],
        };
        let json = config.to_json();
        let parsed = StepConfig::from_json(&json).unwrap();

        match parsed {
            StepConfig::Sequence { sequence } => {
                assert_eq!(sequence, vec!["4:1", "5:1"]);
            }
            _ => panic!("Wrong type"),
        }
    }

    /// ---------------------------------------------------------------------------
    /// Phase 7: Database Operations Tests
    /// ---------------------------------------------------------------------------

    #[tokio::test]
    async fn test_load_and_save_dimension_roundtrip() {
        // Arrange: Setup test DB with progression dimensions
        let pool = crate::test_utils::setup_test_db().await;
        crate::test_utils::seed_test_progression_dimensions(&pool).await;

        // Act: Load a dimension
        let mut dim = load_dimension(&pool, "run_interval")
            .await
            .expect("Should load run_interval");

        // Verify initial state
        assert_eq!(dim.name, "run_interval");
        assert_eq!(dim.current_value, "4:1");
        assert_eq!(dim.ceiling_value, "continuous_45");
        assert_eq!(dim.status, LifecycleStatus::Building);

        // Modify the dimension
        dim.current_value = "5:1".to_string();
        dim.status = LifecycleStatus::AtCeiling;
        dim.last_ceiling_touch_at = Some(Utc::now());

        // Act: Save it back
        save_dimension(&pool, &dim)
            .await
            .expect("Should save dimension");

        // Act: Reload to verify persistence
        let reloaded = load_dimension(&pool, "run_interval")
            .await
            .expect("Should reload dimension");

        // Assert: Changes persisted
        assert_eq!(reloaded.current_value, "5:1");
        assert_eq!(reloaded.status, LifecycleStatus::AtCeiling);
        assert!(reloaded.last_ceiling_touch_at.is_some());

        crate::test_utils::teardown_test_db(pool).await;
    }

    #[tokio::test]
    async fn test_load_all_dimensions() {
        // Arrange
        let pool = crate::test_utils::setup_test_db().await;
        crate::test_utils::seed_test_progression_dimensions(&pool).await;

        // Act
        let dimensions = load_all_dimensions(&pool)
            .await
            .expect("Should load all dimensions");

        // Assert: Should have 3 seeded dimensions
        assert_eq!(dimensions.len(), 3);

        // Verify each dimension
        let run_interval = dimensions.iter().find(|d| d.name == "run_interval").unwrap();
        assert_eq!(run_interval.current_value, "4:1");

        let long_run = dimensions.iter().find(|d| d.name == "long_run").unwrap();
        assert_eq!(long_run.current_value, "30");

        let z2_ride = dimensions.iter().find(|d| d.name == "z2_ride").unwrap();
        assert_eq!(z2_ride.current_value, "45");
        assert_eq!(z2_ride.status, LifecycleStatus::AtCeiling);

        crate::test_utils::teardown_test_db(pool).await;
    }

    #[tokio::test]
    async fn test_load_dimension_not_found() {
        // Arrange
        let pool = crate::test_utils::setup_test_db().await;

        // Act: Try to load non-existent dimension
        let result = load_dimension(&pool, "nonexistent").await;

        // Assert: Should fail with helpful error
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));

        crate::test_utils::teardown_test_db(pool).await;
    }

    #[tokio::test]
    async fn test_record_ceiling_touch_updates_timestamp() {
        // Arrange
        let pool = crate::test_utils::setup_test_db().await;
        crate::test_utils::seed_test_progression_dimensions(&pool).await;

        // Use z2_ride which is seeded as at_ceiling
        let before = load_dimension(&pool, "z2_ride")
            .await
            .expect("Should load");
        assert_eq!(before.status, LifecycleStatus::AtCeiling);
        let before_touch = before.last_ceiling_touch_at;

        // Act: Record ceiling touch
        record_ceiling_touch(&pool, "z2_ride")
            .await
            .expect("Should record touch");

        // Assert: Timestamp updated
        let after = load_dimension(&pool, "z2_ride")
            .await
            .expect("Should reload");
        assert!(after.last_ceiling_touch_at.is_some());

        // If there was no prior touch, should be set now
        // If there was a prior touch, new one should be more recent
        if let Some(before_ts) = before_touch {
            assert!(after.last_ceiling_touch_at.unwrap() > before_ts);
        }

        crate::test_utils::teardown_test_db(pool).await;
    }

    #[tokio::test]
    async fn test_apply_regression_steps_back() {
        // Arrange
        let pool = crate::test_utils::setup_test_db().await;
        crate::test_utils::seed_test_progression_dimensions(&pool).await;

        // Manually advance run_interval to "5:1" first
        let mut dim = load_dimension(&pool, "run_interval")
            .await
            .expect("Should load");
        dim.current_value = "5:1".to_string();
        save_dimension(&pool, &dim).await.expect("Should save");

        // Act: Apply regression
        let new_value = apply_regression(&pool, "run_interval")
            .await
            .expect("Should apply regression");

        // Assert: Should step back to previous value
        assert_eq!(new_value, "4:1", "Should regress from 5:1 to 4:1");

        // Verify in database
        let reloaded = load_dimension(&pool, "run_interval")
            .await
            .expect("Should reload");
        assert_eq!(reloaded.current_value, "4:1");
        // After regression, status should be Building (back to building toward ceiling)
        assert_eq!(reloaded.status, LifecycleStatus::Building);

        crate::test_utils::teardown_test_db(pool).await;
    }

    #[tokio::test]
    async fn test_apply_regression_at_minimum() {
        // Arrange: Dimension already at minimum value
        let pool = crate::test_utils::setup_test_db().await;
        crate::test_utils::seed_test_progression_dimensions(&pool).await;

        // run_interval starts at "4:1" which is the minimum (no previous value)
        // Act: Try to apply regression
        let result = apply_regression(&pool, "run_interval").await;

        // Assert: Should return error since there's no previous value
        assert!(result.is_err(), "Should fail when no previous value exists");
        let err = result.unwrap_err();
        assert!(
            err.contains("No previous value"),
            "Error should explain no previous value: {}",
            err
        );

        crate::test_utils::teardown_test_db(pool).await;
    }
}
