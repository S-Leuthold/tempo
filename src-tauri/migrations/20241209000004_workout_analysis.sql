-- LLM-generated workout analysis storage
-- Stores Claude's analysis for each workout

CREATE TABLE IF NOT EXISTS workout_analysis (
    id INTEGER PRIMARY KEY,
    workout_id INTEGER NOT NULL REFERENCES workouts(id) ON DELETE CASCADE,

    -- Analysis content
    summary TEXT NOT NULL,
    tomorrow_recommendation TEXT NOT NULL,
    risk_flags_json TEXT,  -- JSON array of strings
    goal_notes TEXT,       -- Kilimanjaro/marathon specific notes

    -- Tracking
    model_version TEXT NOT NULL,      -- e.g., "claude-sonnet-4-20250514"
    prompt_hash TEXT,                 -- For cache invalidation
    input_tokens INTEGER,
    output_tokens INTEGER,

    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(workout_id)  -- One analysis per workout (can be updated)
);

-- Index for quick lookups
CREATE INDEX IF NOT EXISTS idx_workout_analysis_workout ON workout_analysis(workout_id);
