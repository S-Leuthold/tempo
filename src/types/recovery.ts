// Oura recovery data types matching the Rust OuraContext struct

export interface OuraContext {
  // Sleep metrics
  sleep_duration_hours?: number;
  deep_sleep_hours?: number;
  rem_sleep_hours?: number;
  sleep_efficiency_pct?: number;
  sleep_avg_7d?: number;
  sleep_debt_hours?: number;

  // HRV metrics
  hrv_last_night?: number;
  hrv_avg_7d?: number;
  hrv_trend?: string; // "improving" | "stable" | "declining"
  hrv_declining_days?: number;

  // Resting HR metrics
  resting_hr?: number;
  resting_hr_avg_7d?: number;
  resting_hr_trend?: string; // "up" | "stable" | "down"
}

export interface OuraHistoryPoint {
  date: string;
  sleep_hours?: number;
  hrv?: number;
  resting_hr?: number;
}

// OuraDay from backend (matches Rust struct)
export interface OuraDay {
  date: string;
  sleep_duration_hours?: number;
  hrv_ms?: number;
  resting_hr_bpm?: number;
}

// ## ---------------------------------------------------------------------------
// ## Recovery Signals Types (Deterministic Capacity Modulators)
// ## ---------------------------------------------------------------------------

export interface RecoverySignals {
  sleep: SignalAxis;
  hrv: SignalAxis;
  rhr: SignalAxis;
  overall_band: RecoveryBand;
  constraints: RecoveryConstraints;
  flags: RecoveryFlag[];
  last_updated: string;
}

export interface SignalAxis {
  current_value?: number;
  avg_7d?: number;
  avg_28d?: number;
  state_vs_28d?: number;  // Delta from baseline (key metric!)
  trend_7d_slope?: number;
  debt?: number;  // Sleep only - cumulative shortfall
  consecutive_declining_days?: number;  // HRV/RHR only
}

export type RecoveryBand = 'Green' | 'Yellow' | 'Orange' | 'Red';

export interface RecoveryConstraints {
  intensity_cap: IntensityCap;
  duration_bias: DurationBias;
  progression_gate: boolean;
  caution_note?: string;
}

export type IntensityCap = 'Normal' | 'ModerateOnly' | 'EasyOnly' | 'RecoveryOnly';

export type DurationBias = 'Short' | 'Standard' | 'Long';

export type RecoveryFlag =
  | 'SleepDebtAccumulating'
  | 'HrvBelowBaseline'
  | 'RhrElevated'
  | 'MultiDayDecline'
  | 'DataStale';
