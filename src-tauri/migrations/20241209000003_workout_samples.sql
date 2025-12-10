-- Add samples_json column for 10-second downsampled time series data
-- Format: {"hr": [140,142,...], "watts": [180,185,...], "pace": [5.2,5.1,...]}
-- ~2KB per workout (90 points for 15min, 540 for 90min)

ALTER TABLE workouts ADD COLUMN samples_json TEXT;

-- Track when samples were fetched (null = not yet fetched)
ALTER TABLE workouts ADD COLUMN samples_fetched_at DATETIME;
