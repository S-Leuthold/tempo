# trainer-log

A macOS menubar app for automated training analysis and coaching feedback.

## Overview

**Problem:** Manual workflow of uploading Strava data to ChatGPT loses context over time and requires daily effort.

**Solution:** An "ambient coach" that syncs training data automatically, analyzes workouts with an LLM, and surfaces insights via a low-friction menubar interface.

## User Context

- **Training schedule:** 6x/week (cycling MWF, running T/Th/Sat)
- **Goals:** Kilimanjaro hike (~8 months), possible marathon
- **Data sources:** Strava (workouts), Oura (recovery/sleep/HRV)

## Architecture

### Stack Decision: All-Tauri with Optional Python Subprocess

Based on consensus analysis, we're avoiding a multi-process architecture (Tauri + Python + launchd) in favor of:

**Primary approach:** Tauri handles everything
- Rust backend for API calls, SQLite, LLM integration
- Web frontend (React/Svelte) for UI and charts
- In-process timer for background sync (no launchd for v1)

**Fallback:** If Rust HTTP/OAuth proves painful, Tauri spawns Python scripts as subprocesses for specific tasks (sync, LLM calls) rather than running them as separate launchd jobs.

```
┌─────────────────────────────────────────────────────────┐
│                 Tauri Menubar App                       │
│  ┌─────────────────┐    ┌────────────────────────────┐ │
│  │   Rust Backend  │    │    Web Frontend (JS/TS)    │ │
│  │                 │    │                            │ │
│  │  - Strava API   │◄──►│  - Daily briefing view     │ │
│  │  - Oura API     │    │  - Weekly trends/charts    │ │
│  │  - LLM calls    │    │  - Settings                │ │
│  │  - SQLite       │    │                            │ │
│  │  - Sync timer   │    │  (Chart.js / Recharts)     │ │
│  └─────────────────┘    └────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │   SQLite Database     │
              │   ~/trainer-log.db    │
              └───────────────────────┘
```

### Alternative: Hybrid with Python Subprocess

If OAuth/API complexity warrants it:

```
┌─────────────────────────────────────────────────────────┐
│                 Tauri Menubar App                       │
│  ┌─────────────────┐    ┌────────────────────────────┐ │
│  │   Rust Backend  │    │    Web Frontend (JS/TS)    │ │
│  │                 │    │                            │ │
│  │  - SQLite read  │◄──►│  - Views & charts          │ │
│  │  - Timer/sched  │    │                            │ │
│  │  - Spawn Python │    │                            │ │
│  └────────┬────────┘    └────────────────────────────┘ │
└───────────┼─────────────────────────────────────────────┘
            │ subprocess
            ▼
┌───────────────────────┐
│   Python Scripts      │
│                       │
│  - sync_strava.py     │
│  - sync_oura.py       │
│  - analyze.py (LLM)   │
│                       │
│  (reuse echo patterns)│
└───────────────────────┘
```

## Data Model

### Core Tables

```sql
-- Workout data from Strava
CREATE TABLE workouts (
    id INTEGER PRIMARY KEY,
    strava_id TEXT UNIQUE NOT NULL,
    activity_type TEXT NOT NULL,  -- 'run', 'ride'
    started_at DATETIME NOT NULL,
    duration_seconds INTEGER,
    distance_meters REAL,
    elevation_gain_meters REAL,
    average_heartrate INTEGER,
    max_heartrate INTEGER,
    average_watts REAL,           -- cycling power
    suffer_score INTEGER,         -- Strava's relative effort
    raw_json TEXT,                -- full Strava response
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Recovery data from Oura
CREATE TABLE recovery (
    id INTEGER PRIMARY KEY,
    date DATE UNIQUE NOT NULL,
    hrv_average INTEGER,
    hrv_balance REAL,             -- trend indicator
    resting_hr INTEGER,
    sleep_score INTEGER,
    sleep_duration_seconds INTEGER,
    readiness_score INTEGER,
    raw_json TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- LLM-generated analysis (one per workout)
CREATE TABLE workout_analysis (
    id INTEGER PRIMARY KEY,
    workout_id INTEGER REFERENCES workouts(id),
    summary TEXT,
    tomorrow_recommendation TEXT,
    risk_flags_json TEXT,         -- ["flag1", "flag2"]
    kilimanjaro_notes TEXT,
    model_version TEXT,           -- track which model/prompt version
    prompt_hash TEXT,             -- for cache invalidation
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(workout_id, prompt_hash)
);

-- Weekly summaries
CREATE TABLE weekly_summary (
    id INTEGER PRIMARY KEY,
    week_start DATE UNIQUE NOT NULL,
    total_duration_seconds INTEGER,
    run_duration_seconds INTEGER,
    ride_duration_seconds INTEGER,
    avg_hrv INTEGER,
    training_load_trend TEXT,     -- 'increasing', 'stable', 'decreasing'
    llm_summary TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Sync state tracking
CREATE TABLE sync_state (
    id INTEGER PRIMARY KEY,
    source TEXT UNIQUE NOT NULL,  -- 'strava', 'oura'
    last_sync_at DATETIME,
    last_activity_at DATETIME,
    access_token TEXT,
    refresh_token TEXT,
    token_expires_at DATETIME
);

-- User goals
CREATE TABLE goals (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    target_date DATE,
    description TEXT,
    active BOOLEAN DEFAULT TRUE
);
```

### Pre-computed Aggregates

Rather than dumping raw data to the LLM, we compute summaries:

```sql
-- View for LLM context (last 14 days)
CREATE VIEW recent_training_summary AS
SELECT
    date(started_at) as day,
    activity_type,
    SUM(duration_seconds) / 60.0 as total_minutes,
    SUM(distance_meters) / 1000.0 as total_km,
    AVG(average_heartrate) as avg_hr,
    AVG(suffer_score) as avg_effort
FROM workouts
WHERE started_at > datetime('now', '-14 days')
GROUP BY date(started_at), activity_type;
```

## LLM Integration

### System Prompt

```
You are a conservative endurance coach specializing in long-term training
for mountain expeditions and marathons. You have access to the athlete's
recent training history and daily recovery metrics.

Your priorities:
1. Consistency over intensity - sustainable progress
2. Injury prevention - flag overtraining early
3. Goal alignment - keep Kilimanjaro and marathon prep on track

Be direct and specific. Avoid generic advice.
```

### Daily Analysis Prompt Structure

**Input context (pre-aggregated by app):**
- Today's workout details
- Last 7-14 days training summary (duration, type split, effort trend)
- Recent Oura metrics (3-7 day HRV trend, sleep quality, readiness)
- Active goals with dates

**Output format (JSON):**
```json
{
  "summary": "45min easy run at 145bpm avg. Good aerobic session, HR stayed in zone 2 throughout.",
  "tomorrow_recommendation": "Rest day or very easy 30min spin. HRV down 8% from baseline suggests accumulated fatigue.",
  "risk_flags": [
    "HRV trending down for 3 consecutive days",
    "Sleep duration below 7hrs last 2 nights"
  ],
  "kilimanjaro_notes": "Base building on track. Consider adding a longer hike (2-3hrs) this weekend to build time-on-feet."
}
```

### Weekly Analysis

Run once per week, looking at 7-28 day trends:
- Training load distribution (run vs ride, zone time if available)
- Recovery trend analysis
- Goal progress assessment
- Recommendations for coming week

## UI Design

### Menubar States

**Icon states:**
- Green: Ready to train (good recovery, on track)
- Yellow: Caution (elevated fatigue, consider backing off)
- Red: Rest recommended (poor recovery metrics)

### Click → Dropdown/Popover

```
┌────────────────────────────────────────┐
│ Ready to Train                      🟢 │
├────────────────────────────────────────┤
│ TODAY: Thursday                        │
│ Last: 45min easy run ✓                │
│                                        │
│ Tomorrow: Rest or easy spin           │
│ HRV trending down - recovery day      │
│ recommended                           │
├────────────────────────────────────────┤
│ This Week                              │
│ ██████░░░░ 4.2 / 7 hrs                │
│ Run: 2.1h | Ride: 2.1h                │
├────────────────────────────────────────┤
│ ⚠️ HRV down 3 days                     │
│ ⚠️ Sleep avg 6.5h (target: 7.5h)      │
├────────────────────────────────────────┤
│ [View Details]  [Settings]  [Sync Now]│
└────────────────────────────────────────┘
```

### Detail View (separate window)

- Weekly/monthly charts (training load, HRV trend, sleep)
- Full workout history with analysis
- Goal tracking
- Settings (sync frequency, LLM model, notification preferences)

## API Integration

### Strava

- OAuth2 flow with refresh tokens
- Webhook for real-time activity updates (stretch goal)
- Polling fallback: check every 30-60 min while app running
- Rate limit: 100 requests/15min, 1000/day (plenty for personal use)

**Key endpoints:**
- `GET /athlete/activities` - list recent activities
- `GET /activities/{id}` - detailed activity data
- `GET /activities/{id}/streams` - time-series data (HR, power, pace)

### Oura

- OAuth2 or Personal Access Token
- Daily data only (no real-time)
- Poll once per day (morning, after ring sync)

**Key endpoints:**
- `GET /v2/usercollection/daily_readiness`
- `GET /v2/usercollection/daily_sleep`
- `GET /v2/usercollection/heartrate` (for HRV)

## Sync Strategy

### While App Running
- In-process timer checks for new data every 15-30 minutes
- On new activity detected: fetch details → run LLM analysis → update UI

### App Launch
- Check for any missed data since last sync
- Process any unanalyzed workouts

### No launchd for v1
- Sync only happens while app is running
- If this becomes painful, add launchd later

## MVP Scope

### v0.1 - Core Loop
- [ ] Tauri app scaffold with menubar
- [ ] SQLite database setup
- [ ] Strava OAuth flow
- [ ] Fetch and store workouts
- [ ] Basic LLM analysis (one workout)
- [ ] Simple menubar dropdown showing latest analysis

### v0.2 - Full Daily Flow
- [ ] Oura integration
- [ ] Automatic sync on timer
- [ ] Pre-aggregated context for LLM
- [ ] Tomorrow recommendation based on recovery
- [ ] Status icon color based on readiness

### v0.3 - Polish
- [ ] Weekly summary generation
- [ ] Charts in detail view
- [ ] Historical workout browser
- [ ] Settings UI

### Future
- Webhook-based real-time sync
- Training plan generation
- Integration with other data sources (Garmin, Apple Health)
- Local LLM option

## Technical Decisions Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| App framework | Tauri | Native feel, small footprint, web-based charts |
| Backend language | Rust (primary) | Single-language stack, spawns Python if needed |
| Database | SQLite | Local-first, no server, good enough for years of data |
| Frontend | TBD (React/Svelte) | Web tech for easy charting |
| LLM | Anthropic Claude API | Quality, reasonable cost for personal use |
| Background sync | In-process timer | Simpler than launchd, sync while app running |

## Open Questions

1. ~~**Frontend framework:**~~ **Decided: React** - familiar ecosystem, plays nicely with Tauri
2. **Charting library:** Chart.js vs Recharts vs something else?
3. ~~**LLM provider:**~~ **Decided: Claude Sonnet 4.5** - good balance of quality and cost
4. ~~**Notification strategy:**~~ **Decided: Menubar-only** - no macOS notifications

## References

- [Tauri docs](https://tauri.app/v1/guides/)
- [Strava API](https://developers.strava.com/docs/reference/)
- [Oura API](https://cloud.ouraring.com/v2/docs)
- Echo project patterns: `../echo/integrations/`
