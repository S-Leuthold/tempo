-- Ceiling-Based Progressions
-- Replaces week-based training plan with dimension-agnostic progression tracking
-- Each dimension has: current value, ceiling (max), step config, lifecycle status

-- ---------------------------------------------------------------------------
-- Drop old week-based tables
-- ---------------------------------------------------------------------------
DROP TABLE IF EXISTS training_plan_weeks;
DROP TABLE IF EXISTS progression_state;

-- Keep progression_history for audit log (modify to be dimension-agnostic)
DROP TABLE IF EXISTS progression_history;

-- ---------------------------------------------------------------------------
-- Progression Dimensions: Generic table for any progression dimension
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS progression_dimensions (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,            -- 'run_interval', 'long_run', 'z2_ride'

    -- Current state
    current_value TEXT NOT NULL,          -- '5:1' or '40' (stored as text, typed in Rust)
    ceiling_value TEXT NOT NULL,          -- 'continuous_45' or '90'

    -- Progression config (JSON)
    step_config_json TEXT NOT NULL,       -- {"sequence": [...]} or {"increment": 5, "unit": "min"}

    -- Lifecycle
    status TEXT NOT NULL DEFAULT 'building' CHECK (status IN ('building', 'at_ceiling', 'regressing')),
    last_change_at DATETIME,              -- Last time current_value changed
    last_ceiling_touch_at DATETIME,       -- Last time we executed at ceiling level (for maintenance)

    -- Maintenance cadence (days)
    maintenance_cadence_days INTEGER DEFAULT 14,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Progression History: Audit log for all progression changes
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS progression_history (
    id INTEGER PRIMARY KEY,

    -- What changed
    dimension_name TEXT NOT NULL,         -- 'run_interval', 'long_run', 'z2_ride'
    previous_value TEXT NOT NULL,
    new_value TEXT NOT NULL,

    -- Why it changed
    change_type TEXT NOT NULL CHECK (change_type IN ('progress', 'regress', 'ceiling_touch', 'manual', 'ceiling_update')),
    trigger_workout_id INTEGER REFERENCES workouts(id),

    -- Context at time of change (JSON)
    context_snapshot_json TEXT,

    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Seed default dimensions for hybrid fitness / Kilimanjaro prep
-- ---------------------------------------------------------------------------
-- Run intervals: Sequence-based progression toward continuous running
-- Long runs: Increment-based (5 min steps) toward 90 min ceiling
-- Z2 rides: REGULATED (not progressive) - duration selected by TSB
--           For a hybrid athlete (lift + run + ride), cycling duration stays
--           fixed at 45-60 min. Power drifts up naturally; time doesn't escalate.

INSERT INTO progression_dimensions (name, current_value, ceiling_value, step_config_json, maintenance_cadence_days) VALUES
(
    'run_interval',
    '4:1',
    'continuous_45',
    '{"type": "sequence", "sequence": ["4:1", "5:1", "6:1", "8:1", "10:1", "continuous_20", "continuous_30", "continuous_45"]}',
    7
),
(
    'long_run',
    '30',
    '90',
    '{"type": "increment", "increment": 5, "unit": "min"}',
    14
),
(
    'z2_ride',
    '45',
    '60',
    '{"type": "regulated", "options": [45, 60], "unit": "min"}',
    10
);

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_progression_dimensions_name ON progression_dimensions(name);
CREATE INDEX IF NOT EXISTS idx_progression_history_dimension ON progression_history(dimension_name);
CREATE INDEX IF NOT EXISTS idx_progression_history_date ON progression_history(created_at);
