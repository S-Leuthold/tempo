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

### Deterministic Analysis Layer

Rather than dumping raw data to the LLM, we compute metrics and detect patterns first. Claude interprets pre-computed insights, not raw numbers.

#### Tier 1: Per-Workout Metrics (computed on sync)

| Metric | Formula | Modality |
|--------|---------|----------|
| **Pace** | duration / distance → min/km | Run |
| **Power** | average_watts | Ride |
| **kJ** | watts × duration_sec / 1000 | Ride |
| **rTSS** | (duration_min × (avg_hr/lthr)²) / 60 × 100 | Both |
| **Time in Zone** | Classify by HR zone | Both |
| **Efficiency** | pace/avg_hr (run) or watts/avg_hr (ride) | Both |
| **Cardiac Cost** | avg_hr × duration_min | Both |

#### Tier 2: Rolling Context (computed on demand)

| Metric | Window |
|--------|--------|
| **Weekly volume** by modality | 7 days |
| **Week-over-week delta** | vs prior |
| **ATL (Acute Training Load)** | 7-day rTSS sum |
| **CTL (Chronic Training Load)** | 42-day rTSS avg |
| **TSB (Form)** | CTL - ATL |
| **Intensity distribution** | 7-day zone % |
| **Longest session** by modality | 28 days |
| **Consistency** by modality | 28 days |

#### Tier 3: Signals/Flags (boolean alerts)

| Flag | Condition |
|------|-----------|
| `volume_spike` | >1.2× chronic + intensity up |
| `volume_drop` | <0.7× chronic |
| `hr_drift` | +4bpm at same output, 2 sessions |
| `efficiency_gain` | +3% at same HR |
| `long_run_gap` | No >10km run in 3 weeks |
| `musculoskeletal_strain` | Cadence down + pace down + HR same |
| `aerobic_regression` | Efficiency down 2 weeks + Z2% down |
| `high_fatigue` | TSB < -20 |
| `peak_form` | TSB between +5 and +15 |

#### Context Package for Claude

```json
{
  "workout": {
    "type": "Run",
    "duration_min": 44,
    "distance_km": 6.0,
    "pace_min_km": 7.3,
    "avg_hr": 139,
    "efficiency": 0.053,
    "rTSS": 45,
    "zone": "Z2"
  },
  "context": {
    "weekly_volume_hrs": 5.2,
    "week_over_week_change_pct": 12,
    "atl": 280,
    "ctl": 245,
    "tsb": -35,
    "intensity_distribution": {"Z1": 20, "Z2": 60, "Z3": 15, "Z4": 5},
    "workout_number_this_week": 3,
    "consistency_pct": 83
  },
  "flags": ["volume_spike", "high_fatigue"],
  "user": {
    "max_hr": 190,
    "lthr": 170,
    "training_days_per_week": 6
  }
}
```

#### User Configuration Required

- **max_hr**: Maximum heart rate (can estimate as 220 - age)
- **lthr**: Lactate threshold heart rate (~93% of max_hr if unknown)
- **training_days_per_week**: Expected workout frequency (default: 6)

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
- [x] Tauri app scaffold with menubar
- [x] SQLite database setup
- [x] Strava OAuth flow
- [x] Fetch and store workouts
- [ ] Deterministic analysis layer (Tier 1 metrics)
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
| Frontend | React | Familiar ecosystem, plays nicely with Tauri |
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
