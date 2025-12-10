-- Training Plan and Progression State
-- Stores the training plan and tracks progression through it

-- ---------------------------------------------------------------------------
-- Training Plan: The baseline plan to work against
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS training_plan (
    id INTEGER PRIMARY KEY,

    -- Plan metadata
    name TEXT NOT NULL DEFAULT 'Kilimanjaro Prep',
    phase TEXT NOT NULL DEFAULT 'base',  -- 'base', 'build', 'peak', 'taper'
    phase_week INTEGER NOT NULL DEFAULT 1,
    phase_total_weeks INTEGER NOT NULL DEFAULT 8,

    -- Run targets
    run_days_per_week INTEGER NOT NULL DEFAULT 3,
    run_interval_target TEXT NOT NULL DEFAULT 'continuous',  -- e.g., '5:1', 'continuous'
    long_run_target_min INTEGER NOT NULL DEFAULT 90,

    -- Ride targets
    ride_days_per_week INTEGER NOT NULL DEFAULT 3,
    long_ride_target_min INTEGER NOT NULL DEFAULT 120,

    -- Weekly structure (JSON)
    -- e.g., {"mon": "ride_easy", "tue": "run_intervals", ...}
    weekly_structure_json TEXT,

    -- Goals
    primary_goal TEXT DEFAULT 'Kilimanjaro hike',
    primary_goal_date TEXT,  -- ISO date
    secondary_goal TEXT DEFAULT 'Marathon',

    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Progression State: Current progress toward plan targets
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS progression_state (
    id INTEGER PRIMARY KEY,

    -- Run interval progression
    run_interval_current TEXT NOT NULL DEFAULT '4:1',  -- Current ratio
    run_interval_last_change DATETIME,

    -- Long run progression
    long_run_current_min INTEGER NOT NULL DEFAULT 45,
    long_run_last_change DATETIME,

    -- Long ride progression
    long_ride_current_min INTEGER NOT NULL DEFAULT 60,
    long_ride_last_change DATETIME,

    -- Tracking
    last_workout_date DATETIME,
    consecutive_rest_days INTEGER DEFAULT 0,

    -- LLM-proposed adjustments (Rust validates before applying)
    pending_adjustment_json TEXT,  -- Proposed changes from LLM

    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Progression History: Log of all progression changes
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS progression_history (
    id INTEGER PRIMARY KEY,

    progression_type TEXT NOT NULL,  -- 'run_interval', 'long_run', 'long_ride'
    previous_value TEXT NOT NULL,
    new_value TEXT NOT NULL,

    -- What triggered the change
    trigger_type TEXT NOT NULL,  -- 'criteria_met', 'llm_adjustment', 'manual'
    trigger_workout_id INTEGER REFERENCES workouts(id),

    -- Criteria snapshot at time of change
    criteria_snapshot_json TEXT,

    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Insert defaults if not exists
-- ---------------------------------------------------------------------------
INSERT OR IGNORE INTO training_plan (id, name, phase, weekly_structure_json)
VALUES (1, 'Kilimanjaro Prep', 'base', '{
  "mon": {"type": "ride", "intensity": "easy", "duration_min": 45},
  "tue": {"type": "run", "intensity": "intervals", "duration_min": 35},
  "wed": {"type": "ride", "intensity": "tempo", "duration_min": 50},
  "thu": {"type": "run", "intensity": "easy", "duration_min": 35},
  "fri": {"type": "ride", "intensity": "recovery", "duration_min": 40},
  "sat": {"type": "run", "intensity": "long", "duration_min": 45}
}');

INSERT OR IGNORE INTO progression_state (id, run_interval_current, long_run_current_min, long_ride_current_min)
VALUES (1, '4:1', 45, 60);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_progression_history_type ON progression_history(progression_type);
CREATE INDEX IF NOT EXISTS idx_progression_history_date ON progression_history(created_at);
