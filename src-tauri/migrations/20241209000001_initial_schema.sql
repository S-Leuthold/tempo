-- Initial schema for trainer-log
-- Workout data from Strava
CREATE TABLE IF NOT EXISTS workouts (
  id INTEGER PRIMARY KEY,
  strava_id TEXT UNIQUE NOT NULL,
  activity_type TEXT NOT NULL,
  started_at DATETIME NOT NULL,
  duration_seconds INTEGER,
  distance_meters REAL,
  elevation_gain_meters REAL,
  average_heartrate INTEGER,
  max_heartrate INTEGER,
  average_watts REAL,
  suffer_score INTEGER,
  raw_json TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Recovery data from Oura
CREATE TABLE IF NOT EXISTS recovery (
  id INTEGER PRIMARY KEY,
  date DATE UNIQUE NOT NULL,
  hrv_average INTEGER,
  hrv_balance REAL,
  resting_hr INTEGER,
  sleep_score INTEGER,
  sleep_duration_seconds INTEGER,
  readiness_score INTEGER,
  raw_json TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- LLM-generated analysis (one per workout)
CREATE TABLE IF NOT EXISTS workout_analysis (
  id INTEGER PRIMARY KEY,
  workout_id INTEGER REFERENCES workouts(id),
  summary TEXT,
  tomorrow_recommendation TEXT,
  risk_flags_json TEXT,
  kilimanjaro_notes TEXT,
  model_version TEXT,
  prompt_hash TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(workout_id, prompt_hash)
);

-- Weekly summaries
CREATE TABLE IF NOT EXISTS weekly_summary (
  id INTEGER PRIMARY KEY,
  week_start DATE UNIQUE NOT NULL,
  total_duration_seconds INTEGER,
  run_duration_seconds INTEGER,
  ride_duration_seconds INTEGER,
  avg_hrv INTEGER,
  training_load_trend TEXT,
  llm_summary TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Sync state tracking
CREATE TABLE IF NOT EXISTS sync_state (
  id INTEGER PRIMARY KEY,
  source TEXT UNIQUE NOT NULL,
  last_sync_at DATETIME,
  last_activity_at DATETIME,
  access_token TEXT,
  refresh_token TEXT,
  token_expires_at DATETIME
);

-- User goals
CREATE TABLE IF NOT EXISTS goals (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  target_date DATE,
  description TEXT,
  active BOOLEAN DEFAULT TRUE
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_workouts_started_at ON workouts(started_at);
CREATE INDEX IF NOT EXISTS idx_workouts_activity_type ON workouts(activity_type);
CREATE INDEX IF NOT EXISTS idx_recovery_date ON recovery(date);
CREATE INDEX IF NOT EXISTS idx_workout_analysis_workout_id ON workout_analysis(workout_id);
