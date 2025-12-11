-- Oura Ring integration tables
-- Stores OAuth tokens and daily sleep/HRV/resting HR data

-- OAuth tokens for Oura API
CREATE TABLE IF NOT EXISTS oura_auth (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  access_token TEXT NOT NULL,
  refresh_token TEXT NOT NULL,
  expires_at TIMESTAMP NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily sleep data from Oura
CREATE TABLE IF NOT EXISTS oura_sleep (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date DATE NOT NULL UNIQUE,
  total_sleep_seconds INTEGER,
  deep_sleep_seconds INTEGER,
  rem_sleep_seconds INTEGER,
  light_sleep_seconds INTEGER,
  efficiency_pct INTEGER,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily HRV data from Oura (raw milliseconds, not scores)
CREATE TABLE IF NOT EXISTS oura_hrv (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date DATE NOT NULL UNIQUE,
  average_hrv_ms REAL NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily resting heart rate from Oura
CREATE TABLE IF NOT EXISTS oura_resting_hr (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date DATE NOT NULL UNIQUE,
  resting_hr INTEGER NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for efficient date queries
CREATE INDEX IF NOT EXISTS idx_oura_sleep_date ON oura_sleep(date DESC);
CREATE INDEX IF NOT EXISTS idx_oura_hrv_date ON oura_hrv(date DESC);
CREATE INDEX IF NOT EXISTS idx_oura_resting_hr_date ON oura_resting_hr(date DESC);
