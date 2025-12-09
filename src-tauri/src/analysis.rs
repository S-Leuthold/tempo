//! Deterministic analysis layer for workout metrics
//!
//! This module computes training metrics from raw workout data.
//! Claude interprets these pre-computed insights rather than doing math itself.

use serde::{Deserialize, Serialize};

/// ---------------------------------------------------------------------------
/// User Settings (needed for metric calculations)
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
  pub max_hr: Option<i64>,
  pub lthr: Option<i64>,
  pub ftp: Option<i64>,
  pub training_days_per_week: i64,
}

impl Default for UserSettings {
  fn default() -> Self {
    Self {
      max_hr: None,
      lthr: None,
      ftp: None,
      training_days_per_week: 6,
    }
  }
}

impl UserSettings {
  /// Get LTHR, falling back to 93% of max_hr if not set
  pub fn effective_lthr(&self) -> Option<i64> {
    self.lthr.or_else(|| self.max_hr.map(|m| (m as f64 * 0.93) as i64))
  }
}

/// ---------------------------------------------------------------------------
/// HR Zones
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HrZone {
  Z1, // Recovery: < 60% max
  Z2, // Aerobic: 60-70% max
  Z3, // Tempo: 70-80% max
  Z4, // Threshold: 80-90% max
  Z5, // VO2max: > 90% max
}

impl HrZone {
  pub fn from_hr(hr: i64, max_hr: i64) -> Self {
    let pct = (hr as f64 / max_hr as f64) * 100.0;
    match pct {
      p if p < 60.0 => HrZone::Z1,
      p if p < 70.0 => HrZone::Z2,
      p if p < 80.0 => HrZone::Z3,
      p if p < 90.0 => HrZone::Z4,
      _ => HrZone::Z5,
    }
  }

  pub fn as_str(&self) -> &'static str {
    match self {
      HrZone::Z1 => "Z1",
      HrZone::Z2 => "Z2",
      HrZone::Z3 => "Z3",
      HrZone::Z4 => "Z4",
      HrZone::Z5 => "Z5",
    }
  }
}

/// ---------------------------------------------------------------------------
/// Tier 1: Per-Workout Computed Metrics
/// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutMetrics {
  /// Running pace in min/km (None for non-run activities)
  pub pace_min_per_km: Option<f64>,

  /// Cycling speed in km/h (fallback if no power)
  pub speed_kmh: Option<f64>,

  /// Cycling work in kilojoules
  pub kj: Option<f64>,

  /// Relative Training Stress Score (HR-based)
  pub rtss: Option<f64>,

  /// Efficiency: pace/hr (run) or watts/hr (ride)
  pub efficiency: Option<f64>,

  /// Cardiac cost: avg_hr * duration_min
  pub cardiac_cost: Option<f64>,

  /// HR zone based on average HR
  pub hr_zone: Option<HrZone>,
}

impl WorkoutMetrics {
  /// Compute all Tier 1 metrics from raw workout data
  pub fn compute(
    activity_type: &str,
    duration_seconds: Option<i64>,
    distance_meters: Option<f64>,
    average_hr: Option<i64>,
    average_watts: Option<f64>,
    settings: &UserSettings,
  ) -> Self {
    let duration_min = duration_seconds.map(|s| s as f64 / 60.0);
    let duration_hr = duration_seconds.map(|s| s as f64 / 3600.0);
    let distance_km = distance_meters.map(|m| m / 1000.0);

    // Pace (running only)
    let pace_min_per_km = if activity_type.to_lowercase() == "run" {
      match (duration_min, distance_km) {
        (Some(dur), Some(dist)) if dist > 0.0 => Some(dur / dist),
        _ => None,
      }
    } else {
      None
    };

    // Speed (cycling, fallback metric)
    let speed_kmh = if activity_type.to_lowercase() == "ride" {
      match (distance_km, duration_hr) {
        (Some(dist), Some(dur)) if dur > 0.0 => Some(dist / dur),
        _ => None,
      }
    } else {
      None
    };

    // kJ (cycling with power)
    let kj = if activity_type.to_lowercase() == "ride" {
      match (average_watts, duration_seconds) {
        (Some(watts), Some(secs)) => Some(watts * secs as f64 / 1000.0),
        _ => None,
      }
    } else {
      None
    };

    // rTSS (HR-based training stress)
    // Formula: (duration_min * (avg_hr / lthr)^2) / 60 * 100
    let rtss = match (duration_min, average_hr, settings.effective_lthr()) {
      (Some(dur), Some(hr), Some(lthr)) if lthr > 0 => {
        let intensity = hr as f64 / lthr as f64;
        Some((dur * intensity.powi(2)) / 60.0 * 100.0)
      }
      _ => None,
    };

    // Efficiency
    let efficiency = match (activity_type.to_lowercase().as_str(), average_hr) {
      ("run", Some(hr)) if hr > 0 => {
        // For running: lower pace/hr is better (faster at lower HR)
        pace_min_per_km.map(|pace| pace / hr as f64)
      }
      ("ride", Some(hr)) if hr > 0 => {
        // For cycling: higher watts/hr is better
        average_watts.map(|watts| watts / hr as f64)
      }
      _ => None,
    };

    // Cardiac cost
    let cardiac_cost = match (average_hr, duration_min) {
      (Some(hr), Some(dur)) => Some(hr as f64 * dur),
      _ => None,
    };

    // HR Zone
    let hr_zone = match (average_hr, settings.max_hr) {
      (Some(hr), Some(max)) => Some(HrZone::from_hr(hr, max)),
      _ => None,
    };

    Self {
      pace_min_per_km,
      speed_kmh,
      kj,
      rtss,
      efficiency,
      cardiac_cost,
      hr_zone,
    }
  }
}

/// ---------------------------------------------------------------------------
/// Tests
/// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_hr_zones() {
    let max_hr = 190;
    assert_eq!(HrZone::from_hr(100, max_hr), HrZone::Z1); // 53%
    assert_eq!(HrZone::from_hr(120, max_hr), HrZone::Z2); // 63%
    assert_eq!(HrZone::from_hr(140, max_hr), HrZone::Z3); // 74%
    assert_eq!(HrZone::from_hr(165, max_hr), HrZone::Z4); // 87%
    assert_eq!(HrZone::from_hr(180, max_hr), HrZone::Z5); // 95%
  }

  #[test]
  fn test_running_metrics() {
    let settings = UserSettings {
      max_hr: Some(190),
      lthr: Some(170),
      ftp: None,
      training_days_per_week: 6,
    };

    let metrics = WorkoutMetrics::compute(
      "Run",
      Some(2640),     // 44 minutes
      Some(6000.0),   // 6 km
      Some(139),      // avg HR
      None,           // no watts
      &settings,
    );

    // Pace should be ~7.33 min/km
    assert!(metrics.pace_min_per_km.is_some());
    let pace = metrics.pace_min_per_km.unwrap();
    assert!((pace - 7.33).abs() < 0.1);

    // Should have rTSS
    assert!(metrics.rtss.is_some());

    // Should be Z3 (139/190 = 73%)
    assert_eq!(metrics.hr_zone, Some(HrZone::Z3));

    // No cycling metrics
    assert!(metrics.kj.is_none());
    assert!(metrics.speed_kmh.is_none());
  }

  #[test]
  fn test_cycling_metrics() {
    let settings = UserSettings {
      max_hr: Some(190),
      lthr: Some(170),
      ftp: Some(250),
      training_days_per_week: 6,
    };

    let metrics = WorkoutMetrics::compute(
      "Ride",
      Some(2700),     // 45 minutes
      Some(20600.0),  // 20.6 km
      Some(126),      // avg HR
      Some(180.0),    // 180 watts
      &settings,
    );

    // kJ should be 180 * 2700 / 1000 = 486
    assert!(metrics.kj.is_some());
    let kj = metrics.kj.unwrap();
    assert!((kj - 486.0).abs() < 1.0);

    // Speed should be ~27.5 km/h
    assert!(metrics.speed_kmh.is_some());

    // Efficiency: watts/hr = 180/126 ≈ 1.43
    assert!(metrics.efficiency.is_some());
    let eff = metrics.efficiency.unwrap();
    assert!((eff - 1.43).abs() < 0.1);

    // No running metrics
    assert!(metrics.pace_min_per_km.is_none());
  }

  #[test]
  fn test_lthr_fallback() {
    let settings = UserSettings {
      max_hr: Some(190),
      lthr: None, // Not set
      ftp: None,
      training_days_per_week: 6,
    };

    // Should fall back to 93% of max = 177
    assert_eq!(settings.effective_lthr(), Some(176)); // 190 * 0.93 = 176.7 -> 176
  }
}
