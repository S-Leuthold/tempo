-- Analysis layer: user settings and computed workout metrics

-- User settings for analysis calculations
CREATE TABLE IF NOT EXISTS user_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),  -- singleton table
  max_hr INTEGER,                          -- maximum heart rate
  lthr INTEGER,                            -- lactate threshold heart rate
  ftp INTEGER,                             -- functional threshold power (cycling)
  training_days_per_week INTEGER DEFAULT 6,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Insert default row (user can update later)
INSERT OR IGNORE INTO user_settings (id) VALUES (1);

-- Add computed metrics columns to workouts
ALTER TABLE workouts ADD COLUMN pace_min_per_km REAL;        -- running pace
ALTER TABLE workouts ADD COLUMN speed_kmh REAL;              -- cycling speed (backup if no power)
ALTER TABLE workouts ADD COLUMN kj REAL;                     -- cycling work in kilojoules
ALTER TABLE workouts ADD COLUMN rtss REAL;                   -- relative training stress score
ALTER TABLE workouts ADD COLUMN efficiency REAL;             -- pace/hr or watts/hr
ALTER TABLE workouts ADD COLUMN cardiac_cost REAL;           -- hr * duration
ALTER TABLE workouts ADD COLUMN hr_zone TEXT;                -- Z1-Z5
ALTER TABLE workouts ADD COLUMN metrics_computed_at DATETIME;

-- Index for finding workouts needing metric computation
CREATE INDEX IF NOT EXISTS idx_workouts_metrics_computed ON workouts(metrics_computed_at);
