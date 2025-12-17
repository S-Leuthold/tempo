use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Recovery {
  pub id: i64,
  pub date: NaiveDate,
  pub hrv_average: Option<i64>,
  pub hrv_balance: Option<f64>,
  pub resting_hr: Option<i64>,
  pub sleep_score: Option<i64>,
  pub sleep_duration_seconds: Option<i64>,
  pub readiness_score: Option<i64>,
  pub raw_json: Option<String>,
  pub created_at: Option<DateTime<Utc>>,
}

/// For inserting new recovery records (without id, created_at)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRecovery {
  pub date: NaiveDate,
  pub hrv_average: Option<i64>,
  pub hrv_balance: Option<f64>,
  pub resting_hr: Option<i64>,
  pub sleep_score: Option<i64>,
  pub sleep_duration_seconds: Option<i64>,
  pub readiness_score: Option<i64>,
  pub raw_json: Option<String>,
}

// ## ---------------------------------------------------------------------------
// ## Recovery Signals Types (Deterministic Capacity Modulators)
// ## ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySignals {
  pub sleep: SignalAxis,
  pub hrv: SignalAxis,
  pub rhr: SignalAxis,
  pub overall_band: RecoveryBand,
  pub constraints: RecoveryConstraints,
  pub flags: Vec<RecoveryFlag>,
  pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalAxis {
  pub current_value: Option<f64>,
  pub avg_7d: Option<f64>,
  pub avg_28d: Option<f64>,
  pub state_vs_28d: Option<f64>,
  pub trend_7d_slope: Option<f64>,
  pub debt: Option<f64>,
  pub consecutive_declining_days: Option<i32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecoveryBand {
  Green,
  Yellow,
  Orange,
  Red,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConstraints {
  pub intensity_cap: IntensityCap,
  pub duration_bias: DurationBias,
  pub progression_gate: bool,
  pub caution_note: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum IntensityCap {
  Normal,
  ModerateOnly,
  EasyOnly,
  RecoveryOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DurationBias {
  Short,
  Standard,
  Long,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecoveryFlag {
  SleepDebtAccumulating,
  HrvBelowBaseline,
  RhrElevated,
  MultiDayDecline,
  DataStale,
}

// ## ---------------------------------------------------------------------------
// ## Helper Data Structures
// ## ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OuraDay {
  pub date: String,
  pub sleep_duration_hours: Option<f64>,
  pub hrv_ms: Option<f64>,
  pub resting_hr_bpm: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OuraBaseline {
  pub sleep_avg_28d: Option<f64>,
  pub hrv_avg_28d: Option<f64>,
  pub rhr_avg_28d: Option<f64>,
}

// ## ---------------------------------------------------------------------------
// ## Math Utilities
// ## ---------------------------------------------------------------------------

/// Compute simple linear regression slope for trend detection
/// Returns slope (change per day) or None if insufficient data
pub fn compute_linear_slope(values: &[(f64, f64)]) -> Option<f64> {
  if values.len() < 3 {
    return None;
  }

  let n = values.len() as f64;
  let sum_x: f64 = values.iter().map(|(x, _)| x).sum();
  let sum_y: f64 = values.iter().map(|(_, y)| y).sum();
  let sum_xy: f64 = values.iter().map(|(x, y)| x * y).sum();
  let sum_x2: f64 = values.iter().map(|(x, _)| x * x).sum();

  let denominator = n * sum_x2 - sum_x * sum_x;
  if denominator.abs() < 1e-10 {
    return None;
  }

  let slope = (n * sum_xy - sum_x * sum_y) / denominator;
  Some(slope)
}

/// Compute standard deviation
#[allow(dead_code)]
pub fn compute_std_dev(values: &[f64]) -> Option<f64> {
  if values.is_empty() {
    return None;
  }

  let mean = values.iter().sum::<f64>() / values.len() as f64;
  let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
  Some(variance.sqrt())
}

// ## ---------------------------------------------------------------------------
// ## Recovery Band Determination
// ## ---------------------------------------------------------------------------

/// Determine recovery band for sleep based on accumulated debt
fn determine_sleep_band(sleep_debt_hours: Option<f64>) -> RecoveryBand {
  match sleep_debt_hours {
    Some(debt) if debt > 4.0 => RecoveryBand::Red,
    Some(debt) if debt > 2.5 => RecoveryBand::Orange,
    Some(debt) if debt > 1.0 => RecoveryBand::Yellow,
    _ => RecoveryBand::Green,
  }
}

/// Determine recovery band for HRV based on state vs 28d baseline
fn determine_hrv_band(state_vs_28d: Option<f64>) -> RecoveryBand {
  match state_vs_28d {
    Some(delta) if delta < -25.0 => RecoveryBand::Red,
    Some(delta) if delta < -15.0 => RecoveryBand::Orange,
    Some(delta) if delta < -5.0 => RecoveryBand::Yellow,
    _ => RecoveryBand::Green,
  }
}

/// Determine recovery band for resting HR based on state vs 28d baseline
fn determine_rhr_band(state_vs_28d: Option<f64>) -> RecoveryBand {
  match state_vs_28d {
    Some(delta) if delta > 8.0 => RecoveryBand::Red,
    Some(delta) if delta > 5.0 => RecoveryBand::Orange,
    Some(delta) if delta > 2.0 => RecoveryBand::Yellow,
    _ => RecoveryBand::Green,
  }
}

/// Determine overall recovery band from individual signal bands
/// Uses conservative approach: take the worst band
/// Exception: HRV-only Orange → Yellow if both sleep AND RHR are Green
/// (HRV is noisier and can swing without real capacity limitation)
pub fn determine_overall_band(
  sleep: &SignalAxis,
  hrv: &SignalAxis,
  rhr: &SignalAxis,
) -> RecoveryBand {
  let sleep_band = determine_sleep_band(sleep.debt);
  let hrv_band = determine_hrv_band(hrv.state_vs_28d);
  let rhr_band = determine_rhr_band(rhr.state_vs_28d);

  // Apply HRV exception: Orange HRV + Green sleep + Green RHR = Yellow overall
  if hrv_band == RecoveryBand::Orange
    && sleep_band == RecoveryBand::Green
    && rhr_band == RecoveryBand::Green
  {
    return RecoveryBand::Yellow;
  }

  // Otherwise take worst (most conservative)
  use std::cmp::max;
  max(max(sleep_band, hrv_band), rhr_band)
}

/// Generate recovery constraints from overall band
pub fn generate_constraints(
  band: RecoveryBand,
  sleep: &SignalAxis,
  hrv: &SignalAxis,
  rhr: &SignalAxis,
) -> RecoveryConstraints {
  match band {
    RecoveryBand::Green => RecoveryConstraints {
      intensity_cap: IntensityCap::Normal,
      duration_bias: DurationBias::Standard,
      progression_gate: false,
      caution_note: None,
    },
    RecoveryBand::Yellow => {
      let note = format_caution_note(sleep, hrv, rhr);
      RecoveryConstraints {
        intensity_cap: IntensityCap::ModerateOnly,
        duration_bias: DurationBias::Standard,
        progression_gate: false,
        caution_note: Some(note),
      }
    }
    RecoveryBand::Orange => {
      let note = format_caution_note(sleep, hrv, rhr);
      RecoveryConstraints {
        intensity_cap: IntensityCap::EasyOnly,
        duration_bias: DurationBias::Short,
        progression_gate: true,
        caution_note: Some(note),
      }
    }
    RecoveryBand::Red => {
      let note = format_caution_note(sleep, hrv, rhr);
      RecoveryConstraints {
        intensity_cap: IntensityCap::RecoveryOnly,
        duration_bias: DurationBias::Short,
        progression_gate: true,
        caution_note: Some(note),
      }
    }
  }
}

/// Format human-readable caution note from signal states
fn format_caution_note(sleep: &SignalAxis, hrv: &SignalAxis, rhr: &SignalAxis) -> String {
  let mut parts = Vec::new();

  if let Some(debt) = sleep.debt {
    if debt > 1.0 {
      parts.push(format!("sleep debt {:.1}h", debt));
    }
  }

  if let Some(delta) = hrv.state_vs_28d {
    if delta < -5.0 {
      parts.push(format!("HRV {}ms vs baseline", delta.round() as i32));
    }
  }

  if let Some(delta) = rhr.state_vs_28d {
    if delta > 2.0 {
      parts.push(format!("RHR +{}bpm vs baseline", delta.round() as i32));
    }
  }

  if parts.is_empty() {
    "Recovery signals suggest caution".to_string()
  } else {
    parts.join(", ")
  }
}

// ## ---------------------------------------------------------------------------
// ## SignalAxis Computation
// ## ---------------------------------------------------------------------------

/// Compute sleep signal axis from history and baseline
pub fn compute_sleep_axis(
  recent: &OuraDay,
  history_7d: &[OuraDay],
  baseline: &OuraBaseline,
  sleep_target: f64,
) -> SignalAxis {
  let current_value = recent.sleep_duration_hours;

  // Compute 7d average
  let avg_7d = if !history_7d.is_empty() {
    let sum: f64 = history_7d
      .iter()
      .filter_map(|d| d.sleep_duration_hours)
      .sum();
    let count = history_7d.iter().filter(|d| d.sleep_duration_hours.is_some()).count();
    if count > 0 {
      Some(sum / count as f64)
    } else {
      None
    }
  } else {
    None
  };

  // Compute sleep debt (cumulative shortfall vs target over 7d)
  let debt = if !history_7d.is_empty() {
    let total_shortfall: f64 = history_7d
      .iter()
      .filter_map(|d| d.sleep_duration_hours.map(|hours| sleep_target - hours))
      .filter(|shortfall| *shortfall > 0.0)
      .sum();
    Some(total_shortfall)
  } else {
    None
  };

  // Compute 7d trend
  let trend_7d_slope = if history_7d.len() >= 3 {
    let points: Vec<(f64, f64)> = history_7d
      .iter()
      .enumerate()
      .filter_map(|(i, d)| d.sleep_duration_hours.map(|hours| (i as f64, hours)))
      .collect();
    compute_linear_slope(&points)
  } else {
    None
  };

  SignalAxis {
    current_value,
    avg_7d,
    avg_28d: baseline.sleep_avg_28d,
    state_vs_28d: None, // Sleep doesn't use state_vs_28d, uses debt instead
    trend_7d_slope,
    debt,
    consecutive_declining_days: None,
  }
}

/// Compute HRV signal axis from history and baseline
pub fn compute_hrv_axis(
  recent: &OuraDay,
  history_7d: &[OuraDay],
  baseline: &OuraBaseline,
) -> SignalAxis {
  let current_value = recent.hrv_ms;

  // Compute 7d average
  let avg_7d = if !history_7d.is_empty() {
    let sum: f64 = history_7d.iter().filter_map(|d| d.hrv_ms).sum();
    let count = history_7d.iter().filter(|d| d.hrv_ms.is_some()).count();
    if count > 0 {
      Some(sum / count as f64)
    } else {
      None
    }
  } else {
    None
  };

  // Compute state vs 28d baseline
  let state_vs_28d = match (current_value, baseline.hrv_avg_28d) {
    (Some(current), Some(baseline_val)) => Some(current - baseline_val),
    _ => None,
  };

  // Compute 7d trend
  let trend_7d_slope = if history_7d.len() >= 3 {
    let points: Vec<(f64, f64)> = history_7d
      .iter()
      .enumerate()
      .filter_map(|(i, d)| d.hrv_ms.map(|hrv| (i as f64, hrv)))
      .collect();
    compute_linear_slope(&points)
  } else {
    None
  };

  // Count consecutive declining days
  let consecutive_declining_days = count_consecutive_declining_days(
    &history_7d
      .iter()
      .filter_map(|d| d.hrv_ms)
      .collect::<Vec<_>>(),
  );

  SignalAxis {
    current_value,
    avg_7d,
    avg_28d: baseline.hrv_avg_28d,
    state_vs_28d,
    trend_7d_slope,
    debt: None,
    consecutive_declining_days,
  }
}

/// Compute resting HR signal axis from history and baseline
pub fn compute_rhr_axis(
  recent: &OuraDay,
  history_7d: &[OuraDay],
  baseline: &OuraBaseline,
) -> SignalAxis {
  let current_value = recent.resting_hr_bpm;

  // Compute 7d average
  let avg_7d = if !history_7d.is_empty() {
    let sum: f64 = history_7d.iter().filter_map(|d| d.resting_hr_bpm).sum();
    let count = history_7d.iter().filter(|d| d.resting_hr_bpm.is_some()).count();
    if count > 0 {
      Some(sum / count as f64)
    } else {
      None
    }
  } else {
    None
  };

  // Compute state vs 28d baseline
  let state_vs_28d = match (current_value, baseline.rhr_avg_28d) {
    (Some(current), Some(baseline_val)) => Some(current - baseline_val),
    _ => None,
  };

  // Compute 7d trend
  let trend_7d_slope = if history_7d.len() >= 3 {
    let points: Vec<(f64, f64)> = history_7d
      .iter()
      .enumerate()
      .filter_map(|(i, d)| d.resting_hr_bpm.map(|rhr| (i as f64, rhr)))
      .collect();
    compute_linear_slope(&points)
  } else {
    None
  };

  // Count consecutive rising days
  let consecutive_declining_days = count_consecutive_rising_days(
    &history_7d
      .iter()
      .filter_map(|d| d.resting_hr_bpm)
      .collect::<Vec<_>>(),
  );

  SignalAxis {
    current_value,
    avg_7d,
    avg_28d: baseline.rhr_avg_28d,
    state_vs_28d,
    trend_7d_slope,
    debt: None,
    consecutive_declining_days,
  }
}

/// Count consecutive days where values are declining
fn count_consecutive_declining_days(values: &[f64]) -> Option<i32> {
  if values.len() < 2 {
    return None;
  }

  let mut consecutive = 0;
  for i in (1..values.len()).rev() {
    if values[i] < values[i - 1] {
      consecutive += 1;
    } else {
      break;
    }
  }

  Some(consecutive)
}

/// Count consecutive days where values are rising
fn count_consecutive_rising_days(values: &[f64]) -> Option<i32> {
  if values.len() < 2 {
    return None;
  }

  let mut consecutive = 0;
  for i in (1..values.len()).rev() {
    if values[i] > values[i - 1] {
      consecutive += 1;
    } else {
      break;
    }
  }

  Some(consecutive)
}

// ## ---------------------------------------------------------------------------
// ## Main Recovery Signals Computation
// ## ---------------------------------------------------------------------------

/// Compute complete recovery signals from Oura data
/// This is the main entry point that orchestrates all signal computation
pub fn compute_recovery_signals(
  recent: &OuraDay,
  history_7d: &[OuraDay],
  baseline: &OuraBaseline,
  sleep_target: f64,
) -> RecoverySignals {
  // Compute individual signal axes
  let sleep = compute_sleep_axis(recent, history_7d, baseline, sleep_target);
  let hrv = compute_hrv_axis(recent, history_7d, baseline);
  let rhr = compute_rhr_axis(recent, history_7d, baseline);

  // Determine overall recovery band
  let overall_band = determine_overall_band(&sleep, &hrv, &rhr);

  // Generate constraints from band
  let constraints = generate_constraints(overall_band, &sleep, &hrv, &rhr);

  // Identify recovery flags
  let mut flags = Vec::new();

  if let Some(debt) = sleep.debt {
    if debt > 2.0 {
      flags.push(RecoveryFlag::SleepDebtAccumulating);
    }
  }

  if let Some(delta) = hrv.state_vs_28d {
    if delta < -10.0 {
      flags.push(RecoveryFlag::HrvBelowBaseline);
    }
  }

  if let Some(delta) = rhr.state_vs_28d {
    if delta > 4.0 {
      flags.push(RecoveryFlag::RhrElevated);
    }
  }

  // Check for multi-day decline (HRV declining 3+ days or RHR rising 3+ days)
  let has_multi_day_decline = hrv.consecutive_declining_days.is_some_and(|days| days >= 3)
    || rhr.consecutive_declining_days.is_some_and(|days| days >= 3);

  if has_multi_day_decline {
    flags.push(RecoveryFlag::MultiDayDecline);
  }

  RecoverySignals {
    sleep,
    hrv,
    rhr,
    overall_band,
    constraints,
    flags,
    last_updated: chrono::Utc::now().to_rfc3339(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_linear_slope_increasing_trend() {
    // Perfect linear increase: y = x
    let data = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)];
    let slope = compute_linear_slope(&data).unwrap();
    assert!((slope - 1.0).abs() < 0.001);
  }

  #[test]
  fn test_linear_slope_decreasing_trend() {
    // Perfect linear decrease: y = 10 - x
    let data = vec![(0.0, 10.0), (1.0, 9.0), (2.0, 8.0), (3.0, 7.0)];
    let slope = compute_linear_slope(&data).unwrap();
    assert!((slope + 1.0).abs() < 0.001);
  }

  #[test]
  fn test_linear_slope_flat() {
    // Flat line
    let data = vec![(0.0, 5.0), (1.0, 5.0), (2.0, 5.0), (3.0, 5.0)];
    let slope = compute_linear_slope(&data).unwrap();
    assert!(slope.abs() < 0.001);
  }

  #[test]
  fn test_linear_slope_insufficient_data() {
    let data = vec![(0.0, 1.0), (1.0, 2.0)];
    assert!(compute_linear_slope(&data).is_none());
  }

  #[test]
  fn test_linear_slope_noisy_data() {
    // HRV-like noisy data with slight downward trend
    let data = vec![
      (0.0, 60.0),
      (1.0, 55.0),
      (2.0, 58.0),
      (3.0, 52.0),
      (4.0, 54.0),
      (5.0, 50.0),
      (6.0, 48.0),
    ];
    let slope = compute_linear_slope(&data).unwrap();
    // Should detect downward trend
    assert!(slope < 0.0);
    // Slope should be around -2 per day
    assert!(slope > -3.0 && slope < -1.0);
  }

  #[test]
  fn test_std_dev_calculation() {
    let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
    let std_dev = compute_std_dev(&values).unwrap();
    // Expected std dev is 2.0
    assert!((std_dev - 2.0).abs() < 0.01);
  }

  #[test]
  fn test_std_dev_empty() {
    let values: Vec<f64> = vec![];
    assert!(compute_std_dev(&values).is_none());
  }

  // ## Recovery Band Tests -----------------------------------------------------

  #[test]
  fn test_sleep_band_thresholds() {
    // Green: < 1.0h debt
    assert_eq!(determine_sleep_band(Some(0.5)), RecoveryBand::Green);

    // Yellow: 1.0 - 2.5h debt
    assert_eq!(determine_sleep_band(Some(1.5)), RecoveryBand::Yellow);

    // Orange: 2.5 - 4.0h debt
    assert_eq!(determine_sleep_band(Some(3.0)), RecoveryBand::Orange);

    // Red: > 4.0h debt
    assert_eq!(determine_sleep_band(Some(5.0)), RecoveryBand::Red);

    // None = Green
    assert_eq!(determine_sleep_band(None), RecoveryBand::Green);
  }

  #[test]
  fn test_hrv_band_thresholds() {
    // Green: >= -5ms vs baseline
    assert_eq!(determine_hrv_band(Some(0.0)), RecoveryBand::Green);
    assert_eq!(determine_hrv_band(Some(-4.0)), RecoveryBand::Green);

    // Yellow: -5 to -15ms
    assert_eq!(determine_hrv_band(Some(-10.0)), RecoveryBand::Yellow);

    // Orange: -15 to -25ms
    assert_eq!(determine_hrv_band(Some(-20.0)), RecoveryBand::Orange);

    // Red: < -25ms
    assert_eq!(determine_hrv_band(Some(-30.0)), RecoveryBand::Red);
  }

  #[test]
  fn test_rhr_band_thresholds() {
    // Green: <= +2 bpm vs baseline
    assert_eq!(determine_rhr_band(Some(0.0)), RecoveryBand::Green);
    assert_eq!(determine_rhr_band(Some(2.0)), RecoveryBand::Green);

    // Yellow: +2 to +5 bpm
    assert_eq!(determine_rhr_band(Some(4.0)), RecoveryBand::Yellow);

    // Orange: +5 to +8 bpm
    assert_eq!(determine_rhr_band(Some(7.0)), RecoveryBand::Orange);

    // Red: > +8 bpm
    assert_eq!(determine_rhr_band(Some(10.0)), RecoveryBand::Red);
  }

  #[test]
  fn test_overall_band_all_green() {
    let sleep = SignalAxis {
      debt: Some(0.5),
      state_vs_28d: None,
      current_value: Some(7.5),
      avg_7d: Some(7.3),
      avg_28d: Some(7.2),
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let hrv = SignalAxis {
      state_vs_28d: Some(-2.0),
      debt: None,
      current_value: Some(58.0),
      avg_7d: Some(57.0),
      avg_28d: Some(60.0),
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let rhr = SignalAxis {
      state_vs_28d: Some(1.0),
      debt: None,
      current_value: Some(52.0),
      avg_7d: Some(52.0),
      avg_28d: Some(51.0),
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };

    assert_eq!(determine_overall_band(&sleep, &hrv, &rhr), RecoveryBand::Green);
  }

  #[test]
  fn test_overall_band_worst_signal_wins() {
    // Sleep is Red, others Green → Red overall
    let sleep_red = SignalAxis {
      debt: Some(5.0),
      state_vs_28d: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let hrv_green = SignalAxis {
      state_vs_28d: Some(0.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let rhr_green = SignalAxis {
      state_vs_28d: Some(0.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };

    assert_eq!(
      determine_overall_band(&sleep_red, &hrv_green, &rhr_green),
      RecoveryBand::Red
    );
  }

  #[test]
  fn test_overall_band_hrv_exception() {
    // HRV Orange + Sleep Green + RHR Green → Yellow (HRV is noisy)
    let sleep_green = SignalAxis {
      debt: Some(0.5),
      state_vs_28d: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let hrv_orange = SignalAxis {
      state_vs_28d: Some(-20.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let rhr_green = SignalAxis {
      state_vs_28d: Some(1.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };

    // Should return Yellow, not Orange
    assert_eq!(
      determine_overall_band(&sleep_green, &hrv_orange, &rhr_green),
      RecoveryBand::Yellow
    );
  }

  #[test]
  fn test_overall_band_hrv_exception_does_not_apply() {
    // HRV Orange + Sleep Yellow → Orange (exception doesn't apply)
    let sleep_yellow = SignalAxis {
      debt: Some(1.5),
      state_vs_28d: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let hrv_orange = SignalAxis {
      state_vs_28d: Some(-20.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let rhr_green = SignalAxis {
      state_vs_28d: Some(1.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };

    // Should return Orange (worst signal)
    assert_eq!(
      determine_overall_band(&sleep_yellow, &hrv_orange, &rhr_green),
      RecoveryBand::Orange
    );
  }

  #[test]
  fn test_constraints_green_band() {
    let sleep = SignalAxis {
      debt: Some(0.5),
      state_vs_28d: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let hrv = SignalAxis {
      state_vs_28d: Some(0.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let rhr = SignalAxis {
      state_vs_28d: Some(0.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };

    let constraints = generate_constraints(RecoveryBand::Green, &sleep, &hrv, &rhr);

    assert_eq!(constraints.intensity_cap, IntensityCap::Normal);
    assert_eq!(constraints.duration_bias, DurationBias::Standard);
    assert_eq!(constraints.progression_gate, false);
    assert!(constraints.caution_note.is_none());
  }

  #[test]
  fn test_constraints_yellow_band() {
    let sleep = SignalAxis {
      debt: Some(1.5),
      state_vs_28d: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let hrv = SignalAxis {
      state_vs_28d: Some(0.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let rhr = SignalAxis {
      state_vs_28d: Some(0.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };

    let constraints = generate_constraints(RecoveryBand::Yellow, &sleep, &hrv, &rhr);

    assert_eq!(constraints.intensity_cap, IntensityCap::ModerateOnly);
    assert_eq!(constraints.duration_bias, DurationBias::Standard);
    assert_eq!(constraints.progression_gate, false);
    assert!(constraints.caution_note.is_some());
    assert!(constraints.caution_note.unwrap().contains("sleep debt 1.5h"));
  }

  #[test]
  fn test_constraints_orange_band() {
    let sleep = SignalAxis {
      debt: Some(3.0),
      state_vs_28d: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let hrv = SignalAxis {
      state_vs_28d: Some(-20.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let rhr = SignalAxis {
      state_vs_28d: Some(0.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };

    let constraints = generate_constraints(RecoveryBand::Orange, &sleep, &hrv, &rhr);

    assert_eq!(constraints.intensity_cap, IntensityCap::EasyOnly);
    assert_eq!(constraints.duration_bias, DurationBias::Short);
    assert_eq!(constraints.progression_gate, true);
    assert!(constraints.caution_note.is_some());
  }

  #[test]
  fn test_constraints_red_band() {
    let sleep = SignalAxis {
      debt: Some(5.0),
      state_vs_28d: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let hrv = SignalAxis {
      state_vs_28d: Some(0.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let rhr = SignalAxis {
      state_vs_28d: Some(0.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };

    let constraints = generate_constraints(RecoveryBand::Red, &sleep, &hrv, &rhr);

    assert_eq!(constraints.intensity_cap, IntensityCap::RecoveryOnly);
    assert_eq!(constraints.duration_bias, DurationBias::Short);
    assert_eq!(constraints.progression_gate, true);
    assert!(constraints.caution_note.is_some());
  }

  #[test]
  fn test_caution_note_multiple_signals() {
    let sleep = SignalAxis {
      debt: Some(2.0),
      state_vs_28d: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let hrv = SignalAxis {
      state_vs_28d: Some(-12.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };
    let rhr = SignalAxis {
      state_vs_28d: Some(6.0),
      debt: None,
      current_value: None,
      avg_7d: None,
      avg_28d: None,
      trend_7d_slope: None,
      consecutive_declining_days: None,
    };

    let note = format_caution_note(&sleep, &hrv, &rhr);

    // Should contain all three signals
    assert!(note.contains("sleep debt 2.0h"));
    assert!(note.contains("HRV -12ms"));
    assert!(note.contains("RHR +6bpm"));
  }

  // ## SignalAxis Computation Tests --------------------------------------------

  #[test]
  fn test_compute_sleep_axis_basic() {
    let recent = OuraDay {
      date: "2025-12-12".to_string(),
      sleep_duration_hours: Some(6.5),
      hrv_ms: None,
      resting_hr_bpm: None,
    };

    let history = vec![
      OuraDay { date: "2025-12-05".to_string(), sleep_duration_hours: Some(7.0), hrv_ms: None, resting_hr_bpm: None },
      OuraDay { date: "2025-12-06".to_string(), sleep_duration_hours: Some(6.8), hrv_ms: None, resting_hr_bpm: None },
      OuraDay { date: "2025-12-07".to_string(), sleep_duration_hours: Some(7.2), hrv_ms: None, resting_hr_bpm: None },
      OuraDay { date: "2025-12-08".to_string(), sleep_duration_hours: Some(6.5), hrv_ms: None, resting_hr_bpm: None },
      OuraDay { date: "2025-12-09".to_string(), sleep_duration_hours: Some(6.9), hrv_ms: None, resting_hr_bpm: None },
      OuraDay { date: "2025-12-10".to_string(), sleep_duration_hours: Some(7.1), hrv_ms: None, resting_hr_bpm: None },
      OuraDay { date: "2025-12-11".to_string(), sleep_duration_hours: Some(6.5), hrv_ms: None, resting_hr_bpm: None },
    ];

    let baseline = OuraBaseline {
      sleep_avg_28d: Some(7.0),
      hrv_avg_28d: None,
      rhr_avg_28d: None,
    };

    let axis = compute_sleep_axis(&recent, &history, &baseline, 7.0);

    assert_eq!(axis.current_value, Some(6.5));
    assert!(axis.avg_7d.is_some());
    assert_eq!(axis.avg_28d, Some(7.0));

    // Sleep debt: target 7.0h
    // Shortfalls: 0.0, 0.2, 0.0, 0.5, 0.1, 0.0, 0.5 = 1.3h
    assert!(axis.debt.is_some());
    let debt = axis.debt.unwrap();
    assert!(debt > 1.0 && debt < 1.5);
  }

  #[test]
  fn test_compute_hrv_axis_with_baseline_delta() {
    let recent = OuraDay {
      date: "2025-12-12".to_string(),
      sleep_duration_hours: None,
      hrv_ms: Some(45.0),
      resting_hr_bpm: None,
    };

    let history = vec![
      OuraDay { date: "2025-12-05".to_string(), sleep_duration_hours: None, hrv_ms: Some(60.0), resting_hr_bpm: None },
      OuraDay { date: "2025-12-06".to_string(), sleep_duration_hours: None, hrv_ms: Some(55.0), resting_hr_bpm: None },
      OuraDay { date: "2025-12-07".to_string(), sleep_duration_hours: None, hrv_ms: Some(52.0), resting_hr_bpm: None },
      OuraDay { date: "2025-12-08".to_string(), sleep_duration_hours: None, hrv_ms: Some(50.0), resting_hr_bpm: None },
      OuraDay { date: "2025-12-09".to_string(), sleep_duration_hours: None, hrv_ms: Some(48.0), resting_hr_bpm: None },
      OuraDay { date: "2025-12-10".to_string(), sleep_duration_hours: None, hrv_ms: Some(47.0), resting_hr_bpm: None },
      OuraDay { date: "2025-12-11".to_string(), sleep_duration_hours: None, hrv_ms: Some(45.0), resting_hr_bpm: None },
    ];

    let baseline = OuraBaseline {
      sleep_avg_28d: None,
      hrv_avg_28d: Some(60.0),
      rhr_avg_28d: None,
    };

    let axis = compute_hrv_axis(&recent, &history, &baseline);

    assert_eq!(axis.current_value, Some(45.0));
    assert!(axis.avg_7d.is_some());
    assert_eq!(axis.avg_28d, Some(60.0));

    // State vs baseline: 45 - 60 = -15ms (Orange territory)
    assert_eq!(axis.state_vs_28d, Some(-15.0));

    // Should detect downward trend
    assert!(axis.trend_7d_slope.is_some());
    assert!(axis.trend_7d_slope.unwrap() < 0.0);

    // All 6 consecutive days declining
    assert_eq!(axis.consecutive_declining_days, Some(6));
  }

  #[test]
  fn test_compute_rhr_axis_with_elevation() {
    let recent = OuraDay {
      date: "2025-12-12".to_string(),
      sleep_duration_hours: None,
      hrv_ms: None,
      resting_hr_bpm: Some(58.0),
    };

    let history = vec![
      OuraDay { date: "2025-12-05".to_string(), sleep_duration_hours: None, hrv_ms: None, resting_hr_bpm: Some(52.0) },
      OuraDay { date: "2025-12-06".to_string(), sleep_duration_hours: None, hrv_ms: None, resting_hr_bpm: Some(53.0) },
      OuraDay { date: "2025-12-07".to_string(), sleep_duration_hours: None, hrv_ms: None, resting_hr_bpm: Some(54.0) },
      OuraDay { date: "2025-12-08".to_string(), sleep_duration_hours: None, hrv_ms: None, resting_hr_bpm: Some(55.0) },
      OuraDay { date: "2025-12-09".to_string(), sleep_duration_hours: None, hrv_ms: None, resting_hr_bpm: Some(56.0) },
      OuraDay { date: "2025-12-10".to_string(), sleep_duration_hours: None, hrv_ms: None, resting_hr_bpm: Some(57.0) },
      OuraDay { date: "2025-12-11".to_string(), sleep_duration_hours: None, hrv_ms: None, resting_hr_bpm: Some(58.0) },
    ];

    let baseline = OuraBaseline {
      sleep_avg_28d: None,
      hrv_avg_28d: None,
      rhr_avg_28d: Some(52.0),
    };

    let axis = compute_rhr_axis(&recent, &history, &baseline);

    assert_eq!(axis.current_value, Some(58.0));

    // State vs baseline: 58 - 52 = +6bpm (Orange territory)
    assert_eq!(axis.state_vs_28d, Some(6.0));

    // Should detect upward trend
    assert!(axis.trend_7d_slope.is_some());
    assert!(axis.trend_7d_slope.unwrap() > 0.0);

    // All 6 consecutive days rising
    assert_eq!(axis.consecutive_declining_days, Some(6));
  }

  #[test]
  fn test_consecutive_declining_days() {
    // All declining
    let values = vec![60.0, 58.0, 55.0, 52.0, 50.0];
    assert_eq!(count_consecutive_declining_days(&values), Some(4));

    // Last 2 declining
    let values = vec![50.0, 55.0, 58.0, 56.0, 54.0];
    assert_eq!(count_consecutive_declining_days(&values), Some(2));

    // No decline
    let values = vec![50.0, 52.0, 54.0, 56.0];
    assert_eq!(count_consecutive_declining_days(&values), Some(0));

    // Insufficient data
    let values = vec![50.0];
    assert_eq!(count_consecutive_declining_days(&values), None);
  }

  #[test]
  fn test_consecutive_rising_days() {
    // All rising
    let values = vec![50.0, 52.0, 54.0, 56.0, 58.0];
    assert_eq!(count_consecutive_rising_days(&values), Some(4));

    // Last 2 rising
    let values = vec![60.0, 55.0, 52.0, 54.0, 56.0];
    assert_eq!(count_consecutive_rising_days(&values), Some(2));

    // No rise
    let values = vec![58.0, 56.0, 54.0, 52.0];
    assert_eq!(count_consecutive_rising_days(&values), Some(0));
  }
}
