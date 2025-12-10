-- Training Plan V2: 12-Week Structured Plan with Per-Week Benchmarks
-- This replaces the simpler training_plan with explicit per-week targets

-- ---------------------------------------------------------------------------
-- Drop old tables (we're starting fresh with the new schema)
-- ---------------------------------------------------------------------------
DROP TABLE IF EXISTS progression_history;
DROP TABLE IF EXISTS progression_state;
DROP TABLE IF EXISTS training_plan;

-- ---------------------------------------------------------------------------
-- Training Phases (enum-like)
-- ---------------------------------------------------------------------------
-- FOUNDATION (weeks 1-4): Build aerobic base, establish habits
-- EXPANSION (weeks 5-8): Extend durations, progress intervals
-- CONSOLIDATION (weeks 9-12): Add quality, maintain volume

-- ---------------------------------------------------------------------------
-- Training Plan: Per-Week Benchmarks
-- Each row represents one week of the 12-week plan
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS training_plan_weeks (
    id INTEGER PRIMARY KEY,
    week_number INTEGER NOT NULL UNIQUE,  -- 1-12

    -- Phase
    phase TEXT NOT NULL CHECK (phase IN ('foundation', 'expansion', 'consolidation')),

    -- Run Interval Targets
    run_interval_target TEXT NOT NULL,  -- e.g., "4:1", "5:1", "continuous_30"

    -- Run Duration Targets (minutes)
    long_run_target_min INTEGER NOT NULL,
    midweek_run_target_min INTEGER,  -- optional structured run

    -- Cycling Targets (minutes per session)
    z2_ride_target_min INTEGER NOT NULL,

    -- Quality Work (only in consolidation phase)
    quality_run_allowed BOOLEAN DEFAULT FALSE,
    tempo_ride_allowed BOOLEAN DEFAULT FALSE,

    -- Progression Gates (what's allowed to progress this week)
    allow_interval_progression BOOLEAN DEFAULT TRUE,
    allow_long_run_progression BOOLEAN DEFAULT TRUE,
    allow_ride_duration_progression BOOLEAN DEFAULT TRUE,

    -- Weekly structure hint (JSON)
    weekly_structure_json TEXT,

    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Progression State: Multi-Channel Tracking
-- Tracks current progress in each dimension separately
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS progression_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- Singleton

    -- Run Interval Progression
    run_interval_current TEXT NOT NULL DEFAULT '4:1',
    run_interval_last_change DATETIME,

    -- Long Run Progression
    long_run_current_min INTEGER NOT NULL DEFAULT 30,
    long_run_last_change DATETIME,

    -- Z2 Ride Progression
    z2_ride_current_min INTEGER NOT NULL DEFAULT 45,
    z2_ride_last_change DATETIME,

    -- Continuous Run Progression (post-intervals)
    continuous_run_current_min INTEGER,  -- NULL until intervals complete
    continuous_run_last_change DATETIME,

    -- Quality Run Level (consolidation phase)
    quality_run_level TEXT DEFAULT 'none' CHECK (quality_run_level IN ('none', 'z3_10min', 'z3_15min', 'z3_20min')),
    quality_run_last_change DATETIME,

    -- Tracking
    current_week INTEGER NOT NULL DEFAULT 1,
    last_workout_date DATETIME,
    consecutive_rest_days INTEGER DEFAULT 0,

    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Progression History: Audit Log
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS progression_history (
    id INTEGER PRIMARY KEY,

    -- What changed
    dimension TEXT NOT NULL CHECK (dimension IN ('run_interval', 'long_run', 'z2_ride', 'continuous_run', 'quality_run')),
    previous_value TEXT NOT NULL,
    new_value TEXT NOT NULL,

    -- Why it changed
    trigger_type TEXT NOT NULL CHECK (trigger_type IN ('criteria_met', 'week_advance', 'manual', 'regression')),
    trigger_workout_id INTEGER REFERENCES workouts(id),

    -- Criteria snapshot at time of change (JSON)
    criteria_snapshot_json TEXT,

    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Insert 12-Week Kilimanjaro Prep Plan
-- ---------------------------------------------------------------------------

-- FOUNDATION Phase (Weeks 1-4)
INSERT INTO training_plan_weeks (week_number, phase, run_interval_target, long_run_target_min, midweek_run_target_min, z2_ride_target_min, quality_run_allowed, tempo_ride_allowed, allow_interval_progression, allow_long_run_progression, allow_ride_duration_progression) VALUES
(1, 'foundation', '4:1', 30, NULL, 45, FALSE, FALSE, TRUE, FALSE, FALSE),
(2, 'foundation', '4:1', 35, NULL, 50, FALSE, FALSE, TRUE, TRUE, TRUE),
(3, 'foundation', '5:1', 40, NULL, 55, FALSE, FALSE, TRUE, TRUE, TRUE),
(4, 'foundation', '5:1', 45, 25, 60, FALSE, FALSE, TRUE, TRUE, TRUE);

-- EXPANSION Phase (Weeks 5-8)
INSERT INTO training_plan_weeks (week_number, phase, run_interval_target, long_run_target_min, midweek_run_target_min, z2_ride_target_min, quality_run_allowed, tempo_ride_allowed, allow_interval_progression, allow_long_run_progression, allow_ride_duration_progression) VALUES
(5, 'expansion', '6:1', 50, 30, 65, FALSE, FALSE, TRUE, TRUE, TRUE),
(6, 'expansion', '8:1', 55, 30, 70, FALSE, FALSE, TRUE, TRUE, TRUE),
(7, 'expansion', '10:1', 60, 35, 75, FALSE, FALSE, TRUE, TRUE, TRUE),
(8, 'expansion', 'continuous_20', 65, 35, 80, FALSE, FALSE, TRUE, TRUE, TRUE);

-- CONSOLIDATION Phase (Weeks 9-12)
INSERT INTO training_plan_weeks (week_number, phase, run_interval_target, long_run_target_min, midweek_run_target_min, z2_ride_target_min, quality_run_allowed, tempo_ride_allowed, allow_interval_progression, allow_long_run_progression, allow_ride_duration_progression) VALUES
(9, 'consolidation', 'continuous_25', 70, 40, 85, TRUE, FALSE, TRUE, TRUE, TRUE),
(10, 'consolidation', 'continuous_30', 75, 40, 90, TRUE, TRUE, TRUE, TRUE, TRUE),
(11, 'consolidation', 'continuous_35', 80, 45, 90, TRUE, TRUE, FALSE, TRUE, FALSE),
(12, 'consolidation', 'continuous_40', 90, 45, 90, TRUE, TRUE, FALSE, FALSE, FALSE);

-- Initialize progression state
INSERT INTO progression_state (id, run_interval_current, long_run_current_min, z2_ride_current_min, current_week)
VALUES (1, '4:1', 30, 45, 1);

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_plan_weeks_week ON training_plan_weeks(week_number);
CREATE INDEX IF NOT EXISTS idx_progression_history_dim ON progression_history(dimension);
CREATE INDEX IF NOT EXISTS idx_progression_history_date ON progression_history(created_at);
