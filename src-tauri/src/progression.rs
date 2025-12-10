//! Progression Engine V2
//!
//! Deterministic progression tracking with per-dimension criteria.
//! The LLM explains; Rust decides.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::analysis::{TrainingContext, TrainingFlags};

/// ---------------------------------------------------------------------------
/// Training Phase
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Foundation,   // Weeks 1-4: Build base, establish habits
    Expansion,    // Weeks 5-8: Extend durations, progress intervals
    Consolidation, // Weeks 9-12: Add quality, maintain volume
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Phase::Foundation => write!(f, "foundation"),
            Phase::Expansion => write!(f, "expansion"),
            Phase::Consolidation => write!(f, "consolidation"),
        }
    }
}

/// ---------------------------------------------------------------------------
/// Week Benchmark: What you should achieve by this week
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekBenchmark {
    pub week_number: i32,
    pub phase: Phase,

    // Run targets
    pub run_interval_target: String,      // "4:1", "5:1", "continuous_30"
    pub long_run_target_min: i32,
    pub midweek_run_target_min: Option<i32>,

    // Cycling targets
    pub z2_ride_target_min: i32,

    // Quality work gates
    pub quality_run_allowed: bool,
    pub tempo_ride_allowed: bool,

    // Progression gates
    pub allow_interval_progression: bool,
    pub allow_long_run_progression: bool,
    pub allow_ride_duration_progression: bool,
}

impl Default for WeekBenchmark {
    fn default() -> Self {
        Self {
            week_number: 1,
            phase: Phase::Foundation,
            run_interval_target: "4:1".to_string(),
            long_run_target_min: 30,
            midweek_run_target_min: None,
            z2_ride_target_min: 45,
            quality_run_allowed: false,
            tempo_ride_allowed: false,
            allow_interval_progression: true,
            allow_long_run_progression: false,
            allow_ride_duration_progression: false,
        }
    }
}

/// ---------------------------------------------------------------------------
/// Progression State: Multi-Channel Current Status
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressionState {
    // Run Interval
    pub run_interval_current: String,
    pub run_interval_last_change: Option<DateTime<Utc>>,

    // Long Run
    pub long_run_current_min: i32,
    pub long_run_last_change: Option<DateTime<Utc>>,

    // Z2 Ride
    pub z2_ride_current_min: i32,
    pub z2_ride_last_change: Option<DateTime<Utc>>,

    // Continuous Run (post-intervals)
    pub continuous_run_current_min: Option<i32>,
    pub continuous_run_last_change: Option<DateTime<Utc>>,

    // Quality Run Level
    pub quality_run_level: QualityRunLevel,
    pub quality_run_last_change: Option<DateTime<Utc>>,

    // Tracking
    pub current_week: i32,
    pub last_workout_date: Option<DateTime<Utc>>,
    pub consecutive_rest_days: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRunLevel {
    None,
    Z3_10min,
    Z3_15min,
    Z3_20min,
}

impl Default for QualityRunLevel {
    fn default() -> Self {
        Self::None
    }
}

impl Default for ProgressionState {
    fn default() -> Self {
        Self {
            run_interval_current: "4:1".to_string(),
            run_interval_last_change: None,
            long_run_current_min: 30,
            long_run_last_change: None,
            z2_ride_current_min: 45,
            z2_ride_last_change: None,
            continuous_run_current_min: None,
            continuous_run_last_change: None,
            quality_run_level: QualityRunLevel::None,
            quality_run_last_change: None,
            current_week: 1,
            last_workout_date: None,
            consecutive_rest_days: 0,
        }
    }
}

/// ---------------------------------------------------------------------------
/// Adherence Summary: Weekly Completion Tracking
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdherenceSummary {
    /// Expected workouts per week (from plan structure)
    pub total_expected: u8,
    /// Completed workouts this week
    pub total_completed: u8,
    /// Key sessions expected (long run, one midweek run, one ride)
    pub key_expected: u8,
    /// Key sessions completed
    pub key_completed: u8,
    /// Adherence percentage (total_completed / total_expected)
    pub adherence_pct: f32,
    /// True if all key sessions completed
    pub key_adherence_good: bool,
    /// True if adherence_pct >= 0.75 AND key_adherence_good
    pub week_stable: bool,
    /// Number of missed workouts this week
    pub missed_workouts: u8,
    /// Consecutive weeks with < 70% adherence
    pub consecutive_low_adherence_weeks: u8,
}

impl AdherenceSummary {
    /// Compute adherence summary from workout counts
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

    /// Returns true if the week is unstable (< 70% adherence)
    pub fn is_unstable(&self) -> bool {
        self.adherence_pct < 0.7
    }

    /// Returns true if regression might be warranted (2+ consecutive low weeks)
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

/// ---------------------------------------------------------------------------
/// Dimension: The three progression tracks
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    RunInterval,
    LongRun,
    Z2Ride,
}

/// ---------------------------------------------------------------------------
/// Criteria: Per-Dimension Requirements for Progression
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionCriteria {
    pub dimension: Dimension,

    // Shared criteria
    pub days_since_change: i32,
    pub min_days_required: i32,  // Usually 7
    pub volume_stable: bool,
    pub fatigue_low: bool,       // TSB > threshold

    // Dimension-specific
    pub hr_stability: bool,      // For run_interval: very sensitive
    pub pain_free: bool,         // For run_interval: hard gate

    // Result
    pub criteria_met: bool,
}

impl DimensionCriteria {
    /// Compute criteria for run interval progression
    pub fn for_run_interval(
        state: &ProgressionState,
        context: &TrainingContext,
        flags: &TrainingFlags,
    ) -> Self {
        let days_since_change = state
            .run_interval_last_change
            .map(|d| (Utc::now() - d).num_days() as i32)
            .unwrap_or(30);

        let volume_stable = !flags.volume_spike && !flags.volume_drop;
        let fatigue_low = context.tsb.map_or(true, |tsb| tsb > -15.0);
        let hr_stability = !flags.intensity_heavy;
        let pain_free = true; // TODO: manual input

        let criteria_met = days_since_change >= 7
            && volume_stable
            && fatigue_low
            && hr_stability
            && pain_free;

        Self {
            dimension: Dimension::RunInterval,
            days_since_change,
            min_days_required: 7,
            volume_stable,
            fatigue_low,
            hr_stability,
            pain_free,
            criteria_met,
        }
    }

    /// Compute criteria for long run progression
    pub fn for_long_run(
        state: &ProgressionState,
        context: &TrainingContext,
        flags: &TrainingFlags,
    ) -> Self {
        let days_since_change = state
            .long_run_last_change
            .map(|d| (Utc::now() - d).num_days() as i32)
            .unwrap_or(30);

        let volume_stable = !flags.volume_spike && !flags.volume_drop;
        let fatigue_low = context.tsb.map_or(true, |tsb| tsb > -15.0);
        // Long run is more tolerant of minor HR issues
        let hr_stability = true;
        let pain_free = true;

        let criteria_met = days_since_change >= 7 && volume_stable && fatigue_low;

        Self {
            dimension: Dimension::LongRun,
            days_since_change,
            min_days_required: 7,
            volume_stable,
            fatigue_low,
            hr_stability,
            pain_free,
            criteria_met,
        }
    }

    /// Compute criteria for Z2 ride duration progression
    pub fn for_z2_ride(
        state: &ProgressionState,
        context: &TrainingContext,
        flags: &TrainingFlags,
    ) -> Self {
        let days_since_change = state
            .z2_ride_last_change
            .map(|d| (Utc::now() - d).num_days() as i32)
            .unwrap_or(30);

        let volume_stable = !flags.volume_spike;
        // Cycling is primarily bound by fatigue, more tolerant threshold
        let fatigue_low = context.tsb.map_or(true, |tsb| tsb > -10.0);
        let hr_stability = true;
        let pain_free = true;

        let criteria_met = days_since_change >= 7 && volume_stable && fatigue_low;

        Self {
            dimension: Dimension::Z2Ride,
            days_since_change,
            min_days_required: 7,
            volume_stable,
            fatigue_low,
            hr_stability,
            pain_free,
            criteria_met,
        }
    }
}

/// ---------------------------------------------------------------------------
/// Assessment: READY | HOLD | REGRESS
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Assessment {
    Ready,
    Hold,
    Regress,
}

/// ---------------------------------------------------------------------------
/// Engine Decision: What Rust actually allows
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineDecision {
    ProgressAllowed,           // All criteria met, plan allows, no conflicts
    HoldForNow,                // Criteria met but blocked by plan or overlap rule
    HoldDueToUnstableWeek,     // Week had < 70% adherence
    HoldDueToMissedKeySession, // Key session (e.g., long run) missed
    Hold,                      // Criteria not met
    Regress,                   // Significant regression needed
}

/// ---------------------------------------------------------------------------
/// Dimension Status: Complete status for one progression track
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionStatus {
    pub dimension: Dimension,
    pub current: String,          // Current value as string (e.g., "4:1", "45")
    pub target_for_week: String,  // Target for this week per plan
    pub assessment: Assessment,   // Raw criteria-based assessment
    pub engine_decision: EngineDecision,  // Final decision after plan + overlap rules
    pub reason: String,           // Human-readable explanation
    pub next_value: Option<String>, // What it would progress to
}

/// ---------------------------------------------------------------------------
/// Plan Status: ahead / on_track / behind
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanAlignment {
    Ahead,
    OnTrack,
    Behind,
}

/// ---------------------------------------------------------------------------
/// Progression Summary: The stable interface sent to the LLM
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressionSummary {
    pub current_week: i32,
    pub phase: Phase,

    // Per-dimension status
    pub run_interval: DimensionStatus,
    pub long_run: DimensionStatus,
    pub z2_ride: DimensionStatus,

    // Overall plan alignment
    pub plan_alignment: PlanAlignment,

    // Which dimension was most recently progressed (for overlap rule)
    pub last_progression_dimension: Option<Dimension>,
    pub days_since_any_progression: i32,

    // Adherence (missing a day ≠ moral failure; it's a load signal)
    pub adherence: AdherenceSummary,
}

impl ProgressionSummary {
    /// Compute the full progression summary
    pub fn compute(
        state: &ProgressionState,
        benchmark: &WeekBenchmark,
        context: &TrainingContext,
        flags: &TrainingFlags,
        adherence: AdherenceSummary,
    ) -> Self {
        // Compute criteria for each dimension
        let interval_criteria = DimensionCriteria::for_run_interval(state, context, flags);
        let long_run_criteria = DimensionCriteria::for_long_run(state, context, flags);
        let z2_ride_criteria = DimensionCriteria::for_z2_ride(state, context, flags);

        // Find most recent progression for overlap rule
        let last_changes = [
            (Dimension::RunInterval, state.run_interval_last_change),
            (Dimension::LongRun, state.long_run_last_change),
            (Dimension::Z2Ride, state.z2_ride_last_change),
        ];
        let most_recent = last_changes
            .iter()
            .filter_map(|(dim, date)| date.map(|d| (*dim, d)))
            .max_by_key(|(_, d)| *d);

        let last_progression_dimension = most_recent.map(|(dim, _)| dim);
        let days_since_any_progression = most_recent
            .map(|(_, d)| (Utc::now() - d).num_days() as i32)
            .unwrap_or(30);

        // Build dimension statuses with overlap rule and adherence enforcement
        let run_interval = Self::build_interval_status(
            state,
            benchmark,
            &interval_criteria,
            last_progression_dimension,
            days_since_any_progression,
            &adherence,
        );
        let long_run = Self::build_long_run_status(
            state,
            benchmark,
            &long_run_criteria,
            last_progression_dimension,
            days_since_any_progression,
            &adherence,
        );
        let z2_ride = Self::build_z2_ride_status(
            state,
            benchmark,
            &z2_ride_criteria,
            last_progression_dimension,
            days_since_any_progression,
            &adherence,
        );

        // Overall plan alignment
        let plan_alignment = Self::compute_alignment(state, benchmark);

        Self {
            current_week: state.current_week,
            phase: benchmark.phase,
            run_interval,
            long_run,
            z2_ride,
            plan_alignment,
            last_progression_dimension,
            days_since_any_progression,
            adherence,
        }
    }

    fn build_interval_status(
        state: &ProgressionState,
        benchmark: &WeekBenchmark,
        criteria: &DimensionCriteria,
        last_dim: Option<Dimension>,
        days_since_any: i32,
        adherence: &AdherenceSummary,
    ) -> DimensionStatus {
        let current = state.run_interval_current.clone();
        let target = benchmark.run_interval_target.clone();
        let next = Self::next_interval(&current);

        // Check if at or beyond target
        let at_target = Self::interval_gte(&current, &target);

        // Raw assessment (before adherence gates)
        let assessment = if state.consecutive_rest_days > 3 && current != "4:1" {
            Assessment::Regress
        } else if adherence.should_consider_regression() && current != "4:1" {
            Assessment::Regress
        } else if at_target {
            Assessment::Hold
        } else if criteria.criteria_met {
            Assessment::Ready
        } else {
            Assessment::Hold
        };

        // Apply plan gate
        let plan_allows = benchmark.allow_interval_progression;

        // Apply overlap rule: if another dimension progressed in last 7 days, hold
        let overlap_blocked = last_dim.map_or(false, |dim| {
            dim != Dimension::RunInterval && days_since_any < 7
        });

        // Final engine decision (adherence gates applied here)
        let (engine_decision, reason) = if assessment == Assessment::Regress {
            if adherence.should_consider_regression() {
                (EngineDecision::Regress, format!("{} consecutive low-adherence weeks", adherence.consecutive_low_adherence_weeks))
            } else {
                (EngineDecision::Regress, format!("{} consecutive rest days", state.consecutive_rest_days))
            }
        } else if at_target {
            (EngineDecision::Hold, "Already at target for this week".to_string())
        } else if adherence.is_unstable() {
            (EngineDecision::HoldDueToUnstableWeek, format!("Week had {}% adherence (need 70%)", (adherence.adherence_pct * 100.0) as i32))
        } else if !plan_allows {
            (EngineDecision::HoldForNow, "Plan doesn't allow interval progression this week".to_string())
        } else if overlap_blocked {
            (EngineDecision::HoldForNow, format!("Another dimension progressed {} days ago (need 7)", days_since_any))
        } else if !criteria.criteria_met {
            let mut reasons = Vec::new();
            if criteria.days_since_change < 7 {
                reasons.push(format!("{} days since last change (need 7)", criteria.days_since_change));
            }
            if !criteria.volume_stable { reasons.push("volume unstable".to_string()); }
            if !criteria.fatigue_low { reasons.push("fatigue high".to_string()); }
            if !criteria.hr_stability { reasons.push("HR unstable".to_string()); }
            (EngineDecision::Hold, reasons.join(", "))
        } else if !adherence.week_stable {
            // Criteria met but week not stable (key session missed or < 75%)
            (EngineDecision::HoldForNow, "Week not stable (missed sessions or low adherence)".to_string())
        } else {
            (EngineDecision::ProgressAllowed, "All criteria met".to_string())
        };

        DimensionStatus {
            dimension: Dimension::RunInterval,
            current,
            target_for_week: target,
            assessment,
            engine_decision,
            reason,
            next_value: Some(next),
        }
    }

    fn build_long_run_status(
        state: &ProgressionState,
        benchmark: &WeekBenchmark,
        criteria: &DimensionCriteria,
        last_dim: Option<Dimension>,
        days_since_any: i32,
        adherence: &AdherenceSummary,
    ) -> DimensionStatus {
        let current = state.long_run_current_min;
        let target = benchmark.long_run_target_min;
        let next = current + 5;

        let at_target = current >= target;

        // Raw assessment (before adherence gates)
        let assessment = if state.consecutive_rest_days > 5 && current > 30 {
            Assessment::Regress
        } else if adherence.should_consider_regression() && current > 30 {
            Assessment::Regress
        } else if at_target {
            Assessment::Hold
        } else if criteria.criteria_met {
            Assessment::Ready
        } else {
            Assessment::Hold
        };

        let plan_allows = benchmark.allow_long_run_progression;
        let overlap_blocked = last_dim.map_or(false, |dim| {
            dim != Dimension::LongRun && days_since_any < 7
        });

        // Final engine decision (adherence gates applied here)
        // Long run is especially sensitive to key session adherence
        let (engine_decision, reason) = if assessment == Assessment::Regress {
            if adherence.should_consider_regression() {
                (EngineDecision::Regress, format!("{} consecutive low-adherence weeks", adherence.consecutive_low_adherence_weeks))
            } else {
                (EngineDecision::Regress, format!("{} consecutive rest days", state.consecutive_rest_days))
            }
        } else if at_target {
            (EngineDecision::Hold, "Already at target for this week".to_string())
        } else if !adherence.key_adherence_good {
            // Long run missed? Can't progress long run dimension
            (EngineDecision::HoldDueToMissedKeySession, "Key session missed this week (long run requires completion)".to_string())
        } else if adherence.is_unstable() {
            (EngineDecision::HoldDueToUnstableWeek, format!("Week had {}% adherence (need 70%)", (adherence.adherence_pct * 100.0) as i32))
        } else if !plan_allows {
            (EngineDecision::HoldForNow, "Plan doesn't allow long run progression this week".to_string())
        } else if overlap_blocked {
            (EngineDecision::HoldForNow, format!("Another dimension progressed {} days ago (need 7)", days_since_any))
        } else if !criteria.criteria_met {
            let mut reasons = Vec::new();
            if criteria.days_since_change < 7 {
                reasons.push(format!("{} days since last change", criteria.days_since_change));
            }
            if !criteria.volume_stable { reasons.push("volume unstable".to_string()); }
            if !criteria.fatigue_low { reasons.push("fatigue high".to_string()); }
            (EngineDecision::Hold, reasons.join(", "))
        } else if !adherence.week_stable {
            (EngineDecision::HoldForNow, "Week not stable (missed sessions or low adherence)".to_string())
        } else {
            (EngineDecision::ProgressAllowed, "All criteria met".to_string())
        };

        DimensionStatus {
            dimension: Dimension::LongRun,
            current: format!("{}min", current),
            target_for_week: format!("{}min", target),
            assessment,
            engine_decision,
            reason,
            next_value: Some(format!("{}min", next)),
        }
    }

    fn build_z2_ride_status(
        state: &ProgressionState,
        benchmark: &WeekBenchmark,
        criteria: &DimensionCriteria,
        last_dim: Option<Dimension>,
        days_since_any: i32,
        adherence: &AdherenceSummary,
    ) -> DimensionStatus {
        let current = state.z2_ride_current_min;
        let target = benchmark.z2_ride_target_min;
        let next = current + 10;

        let at_target = current >= target;

        let assessment = if at_target {
            Assessment::Hold
        } else if criteria.criteria_met {
            Assessment::Ready
        } else {
            Assessment::Hold
        };

        let plan_allows = benchmark.allow_ride_duration_progression;
        let overlap_blocked = last_dim.map_or(false, |dim| {
            dim != Dimension::Z2Ride && days_since_any < 7
        });

        // Final engine decision (adherence gates applied here)
        // Cycling is somewhat more tolerant but still respects unstable weeks
        let (engine_decision, reason) = if at_target {
            (EngineDecision::Hold, "Already at target for this week".to_string())
        } else if adherence.is_unstable() {
            (EngineDecision::HoldDueToUnstableWeek, format!("Week had {}% adherence (need 70%)", (adherence.adherence_pct * 100.0) as i32))
        } else if !plan_allows {
            (EngineDecision::HoldForNow, "Plan doesn't allow ride progression this week".to_string())
        } else if overlap_blocked {
            (EngineDecision::HoldForNow, format!("Another dimension progressed {} days ago (need 7)", days_since_any))
        } else if !criteria.criteria_met {
            let mut reasons = Vec::new();
            if criteria.days_since_change < 7 {
                reasons.push(format!("{} days since last change", criteria.days_since_change));
            }
            if !criteria.volume_stable { reasons.push("volume spike".to_string()); }
            if !criteria.fatigue_low { reasons.push("fatigue high".to_string()); }
            (EngineDecision::Hold, reasons.join(", "))
        } else if !adherence.week_stable {
            (EngineDecision::HoldForNow, "Week not stable (missed sessions or low adherence)".to_string())
        } else {
            (EngineDecision::ProgressAllowed, "All criteria met".to_string())
        };

        DimensionStatus {
            dimension: Dimension::Z2Ride,
            current: format!("{}min", current),
            target_for_week: format!("{}min", target),
            assessment,
            engine_decision,
            reason,
            next_value: Some(format!("{}min", next)),
        }
    }

    fn compute_alignment(state: &ProgressionState, benchmark: &WeekBenchmark) -> PlanAlignment {
        let mut behind_count = 0;
        let mut ahead_count = 0;

        // Run interval
        if !Self::interval_gte(&state.run_interval_current, &benchmark.run_interval_target) {
            behind_count += 1;
        } else if Self::interval_gt(&state.run_interval_current, &benchmark.run_interval_target) {
            ahead_count += 1;
        }

        // Long run
        if state.long_run_current_min < benchmark.long_run_target_min {
            behind_count += 1;
        } else if state.long_run_current_min > benchmark.long_run_target_min {
            ahead_count += 1;
        }

        // Z2 ride
        if state.z2_ride_current_min < benchmark.z2_ride_target_min {
            behind_count += 1;
        } else if state.z2_ride_current_min > benchmark.z2_ride_target_min {
            ahead_count += 1;
        }

        if behind_count >= 2 {
            PlanAlignment::Behind
        } else if ahead_count >= 2 {
            PlanAlignment::Ahead
        } else {
            PlanAlignment::OnTrack
        }
    }

    // Interval progression: 4:1 -> 5:1 -> 6:1 -> 8:1 -> 10:1 -> continuous_20 -> continuous_25 -> ...
    fn next_interval(current: &str) -> String {
        match current {
            "4:1" => "5:1".to_string(),
            "5:1" => "6:1".to_string(),
            "6:1" => "8:1".to_string(),
            "8:1" => "10:1".to_string(),
            "10:1" => "continuous_20".to_string(),
            "continuous_20" => "continuous_25".to_string(),
            "continuous_25" => "continuous_30".to_string(),
            "continuous_30" => "continuous_35".to_string(),
            "continuous_35" => "continuous_40".to_string(),
            _ => "continuous_45".to_string(),
        }
    }

    fn interval_to_rank(interval: &str) -> i32 {
        match interval {
            "4:1" => 1,
            "5:1" => 2,
            "6:1" => 3,
            "8:1" => 4,
            "10:1" => 5,
            "continuous_20" => 6,
            "continuous_25" => 7,
            "continuous_30" => 8,
            "continuous_35" => 9,
            "continuous_40" => 10,
            _ => 11,
        }
    }

    fn interval_gte(a: &str, b: &str) -> bool {
        Self::interval_to_rank(a) >= Self::interval_to_rank(b)
    }

    fn interval_gt(a: &str, b: &str) -> bool {
        Self::interval_to_rank(a) > Self::interval_to_rank(b)
    }
}

/// ---------------------------------------------------------------------------
/// Tests
/// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{IntensityDistribution, LongestSession, WeeklyVolume};

    fn default_context() -> TrainingContext {
        TrainingContext {
            atl: Some(100.0),
            ctl: Some(80.0),
            tsb: Some(-5.0), // Moderate, not fatigued
            weekly_volume: WeeklyVolume::default(),
            week_over_week_delta_pct: Some(5.0),
            intensity_distribution: IntensityDistribution::default(),
            longest_session: LongestSession::default(),
            consistency_pct: Some(85.0),
            workouts_this_week: 4,
        }
    }

    fn default_flags() -> TrainingFlags {
        TrainingFlags::default()
    }

    fn stable_adherence() -> AdherenceSummary {
        AdherenceSummary::default() // 100% adherence, all key sessions done
    }

    fn unstable_adherence() -> AdherenceSummary {
        AdherenceSummary::compute(6, 3, 3, 2, 0) // 50% adherence, missed key session
    }

    #[test]
    fn test_interval_progression_order() {
        assert!(ProgressionSummary::interval_gte("5:1", "4:1"));
        assert!(ProgressionSummary::interval_gte("continuous_20", "10:1"));
        assert!(!ProgressionSummary::interval_gte("4:1", "5:1"));
    }

    #[test]
    fn test_criteria_met_allows_progression() {
        let mut state = ProgressionState::default();
        state.run_interval_last_change = Some(Utc::now() - Duration::days(10));

        // Set target higher than current to allow progression
        let benchmark = WeekBenchmark {
            run_interval_target: "5:1".to_string(), // Current is 4:1, so not at target
            ..WeekBenchmark::default()
        };
        let context = default_context();
        let flags = default_flags();
        let adherence = stable_adherence();

        let summary = ProgressionSummary::compute(&state, &benchmark, &context, &flags, adherence);

        // Should be allowed since criteria met and no overlap
        assert_eq!(summary.run_interval.engine_decision, EngineDecision::ProgressAllowed);
    }

    #[test]
    fn test_overlap_rule_blocks_second_progression() {
        let mut state = ProgressionState::default();
        // Long run just progressed 3 days ago
        state.long_run_last_change = Some(Utc::now() - Duration::days(3));
        // Interval criteria would otherwise be met
        state.run_interval_last_change = Some(Utc::now() - Duration::days(10));

        let benchmark = WeekBenchmark {
            run_interval_target: "5:1".to_string(), // Target higher than current
            allow_long_run_progression: true,
            ..WeekBenchmark::default()
        };
        let context = default_context();
        let flags = default_flags();
        let adherence = stable_adherence();

        let summary = ProgressionSummary::compute(&state, &benchmark, &context, &flags, adherence);

        // Interval should be blocked by overlap rule
        assert_eq!(summary.run_interval.engine_decision, EngineDecision::HoldForNow);
        assert!(summary.run_interval.reason.contains("days ago"));
    }

    #[test]
    fn test_plan_gate_blocks_progression() {
        let mut state = ProgressionState::default();
        state.long_run_last_change = Some(Utc::now() - Duration::days(10));

        // Set long run target higher, but don't allow progression
        let benchmark = WeekBenchmark {
            long_run_target_min: 45, // Current is 30, so not at target
            allow_long_run_progression: false,
            ..WeekBenchmark::default()
        };
        let context = default_context();
        let flags = default_flags();
        let adherence = stable_adherence();

        let summary = ProgressionSummary::compute(&state, &benchmark, &context, &flags, adherence);

        assert_eq!(summary.long_run.engine_decision, EngineDecision::HoldForNow);
        assert!(summary.long_run.reason.contains("Plan doesn't allow"));
    }

    #[test]
    fn test_unstable_week_blocks_progression() {
        let mut state = ProgressionState::default();
        state.run_interval_last_change = Some(Utc::now() - Duration::days(10));

        // Set target higher than current
        let benchmark = WeekBenchmark {
            run_interval_target: "5:1".to_string(),
            ..WeekBenchmark::default()
        };
        let context = default_context();
        let flags = default_flags();
        let adherence = unstable_adherence(); // 50% adherence

        let summary = ProgressionSummary::compute(&state, &benchmark, &context, &flags, adherence);

        // Should be blocked due to unstable week
        assert_eq!(summary.run_interval.engine_decision, EngineDecision::HoldDueToUnstableWeek);
        assert!(summary.run_interval.reason.contains("adherence"));
    }

    #[test]
    fn test_missed_key_session_blocks_long_run() {
        let mut state = ProgressionState::default();
        state.long_run_last_change = Some(Utc::now() - Duration::days(10));

        let benchmark = WeekBenchmark {
            long_run_target_min: 45, // Current is 30, so not at target
            allow_long_run_progression: true,
            ..WeekBenchmark::default()
        };
        let context = default_context();
        let flags = default_flags();
        // 83% adherence but missed key session (key_completed < key_expected)
        let adherence = AdherenceSummary::compute(6, 5, 3, 2, 0);

        let summary = ProgressionSummary::compute(&state, &benchmark, &context, &flags, adherence);

        // Long run should be blocked due to missed key session
        assert_eq!(summary.long_run.engine_decision, EngineDecision::HoldDueToMissedKeySession);
    }

    #[test]
    fn test_consecutive_low_weeks_triggers_regression() {
        let mut state = ProgressionState::default();
        state.run_interval_current = "5:1".to_string();
        state.run_interval_last_change = Some(Utc::now() - Duration::days(10));

        // Set target to match current (at target), but regression overrides
        let benchmark = WeekBenchmark {
            run_interval_target: "5:1".to_string(),
            ..WeekBenchmark::default()
        };
        let context = default_context();
        let flags = default_flags();
        // 2 consecutive low-adherence weeks
        let adherence = AdherenceSummary::compute(6, 3, 3, 1, 2);

        let summary = ProgressionSummary::compute(&state, &benchmark, &context, &flags, adherence);

        // Should recommend regression
        assert_eq!(summary.run_interval.engine_decision, EngineDecision::Regress);
        assert!(summary.run_interval.reason.contains("low-adherence weeks"));
    }
}
