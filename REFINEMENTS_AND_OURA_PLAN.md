# V4 Refinements + Oura Integration Plan

## Goal

1. **Tighten V4 guardrails** - Prevent over-interpretation, add backend thresholds, improve narrative coherence
2. **Integrate Oura** - Sleep hours, HRV trends (no proprietary scores)

---

## Phase 1: Backend Thresholds & Constraints

### 1.1 Add Significance Thresholds to Context

**File:** `src-tauri/src/analysis.rs`

Add thresholds to context package so LLM knows when differences are meaningful:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignificanceThresholds {
  pub hr_delta_significant: i64,        // >5 beats
  pub efficiency_delta_significant: f64, // >3%
  pub pace_delta_significant: f64,      // >10 sec/km
  pub power_delta_significant: f64,     // >10W
  pub temperature_delta_significant: f64, // >5°C
}

impl Default for SignificanceThresholds {
  fn default() -> Self {
    Self {
      hr_delta_significant: 5,
      efficiency_delta_significant: 0.03,
      pace_delta_significant: 10.0,  // seconds per km
      power_delta_significant: 10.0,
      temperature_delta_significant: 5.0,
    }
  }
}

// Add to ContextPackage
pub struct ContextPackage {
  // ... existing fields ...
  pub thresholds: SignificanceThresholds,
}
```

### 1.2 Compute Confidence in Rust

**File:** `src-tauri/src/commands/analysis.rs`

Add confidence computation based on signal quality:

```rust
fn compute_prescription_confidence(
  tsb: Option<f64>,
  flags: &[String],
  adherence_pct: f64,
  recent_workouts_count: usize,
) -> String {
  // High confidence: clear signals, good data
  if tsb.is_some() && flags.len() <= 1 && adherence_pct > 0.8 && recent_workouts_count >= 5 {
    return "high".to_string();
  }

  // Low confidence: mixed signals or sparse data
  if tsb.is_none() || flags.len() >= 3 || recent_workouts_count < 3 {
    return "low".to_string();
  }

  // Medium: everything else
  "medium".to_string()
}
```

Add `confidence` field to context, require LLM to use it:
```rust
pub struct PrescriptionContext {
  pub confidence: String,  // pre-computed
  pub confidence_reason: String, // "Clear signals" or "Mixed fatigue indicators"
}
```

### 1.3 Add TSB Trend Arrow

**File:** `src-tauri/src/analysis.rs`

Actually implement TSB trend (currently returns "unknown"):

```rust
impl FatigueContext {
  pub fn from_training_context_with_history(
    ctx: &TrainingContext,
    workouts: &[WorkoutSummary],
  ) -> Self {
    // Compute TSB 3 days ago and 7 days ago
    // Compare to current TSB
    // Return "improving" if TSB increasing (less negative)
    // "declining" if TSB decreasing (more negative)
    // "stable" if within ±2 points
  }
}
```

Add to context:
```json
"fatigue": {
  "tsb": -12,
  "tsb_band": "moderate_fatigue",
  "tsb_trend": "improving",  // ↗️
  "tsb_3d_ago": -15,
  "tsb_7d_ago": -18
}
```

### 1.4 Flag Priority Sorting in Rust

**File:** `src-tauri/src/analysis.rs`

```rust
impl TrainingFlags {
  pub fn to_prioritized_list(&self) -> Vec<(String, u8)> {
    let mut flags = Vec::new();

    if self.high_fatigue {
      flags.push(("high_fatigue".to_string(), 1));
    }
    if self.volume_spike {
      flags.push(("volume_spike".to_string(), 2));
    }
    if self.intensity_heavy {
      flags.push(("intensity_heavy".to_string(), 3));
    }
    if self.long_run_gap {
      flags.push(("long_run_gap".to_string(), 4));
    }
    if self.long_ride_gap {
      flags.push(("long_ride_gap".to_string(), 4));
    }
    // ... etc

    flags.sort_by_key(|(_, priority)| *priority);
    flags
  }
}
```

Pass sorted flags to LLM with priority numbers.

---

## Phase 2: Prompt Refinements

### 2.1 Update V4 Prompt with Noise Thresholds

**File:** `src-tauri/src/prompts/coach_system_v4.txt`

Add to each card section:

**Performance Card:**
```
SIGNIFICANCE RULES:
- HR delta: Only comment if >5 beats difference
- Efficiency: Only comment if >3% change
- Pace: Only comment if >10 sec/km difference
- Power: Only comment if >10W difference
- If delta is below threshold, note trend is "stable" rather than inventing meaning

Example of GOOD noise handling:
"Pace within 2 sec/km of recent average - stable performance."

Example of BAD noise handling:
"3-second improvement suggests emerging aerobic adaptation."
```

**HR/Efficiency Card:**
```
Do NOT comment on efficiency unless:
- You have 3+ similar workouts
- Change is >3% (use thresholds.efficiency_delta_significant)
- Trend is consistent (not one outlier)

If efficiency data is sparse or noisy, skip efficiency_trend field entirely.
```

**Tomorrow Card:**
```
MANDATORY RATIONALE STRUCTURE:
"[DURATION_LABEL] because [TSB_STATE] + [TOP_FLAG if present]"

Examples:
- "SHORT because TSB -12 (moderate fatigue) + volume spike active"
- "STANDARD because TSB -5 (slightly fatigued) and signals are clear"

Do NOT use the pre-computed confidence - that comes from Rust.
Just output the rationale.
```

### 2.2 Add "Why This Matters" to Eyes On

**Eyes On Card section:**
```
For each flag in priorities array, include:
- flag: Name
- current_value: Current state
- threshold: Target value
- action: What to do
- why_it_matters: One sentence consequence if ignored

Example:
{
  "flag": "intensity_heavy",
  "current_value": "52% Z3+",
  "threshold": "<40%",
  "action": "All runs Z2 only for next 5 sessions",
  "why_it_matters": "High intensity raises recovery demand and typically precedes injury if sustained 7+ days"
}
```

Update Rust type:
```rust
pub struct FlagPriority {
  pub flag: String,
  pub current_value: Option<String>,
  pub threshold: String,
  pub action: String,
  pub why_it_matters: String,  // NEW
}
```

---

## Phase 3: Oura Integration

### 3.1 Oura Data Model

**File:** `src-tauri/src/oura.rs` (create new)

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OuraContext {
  // Sleep data (last night)
  pub sleep_duration_hours: Option<f64>,
  pub deep_sleep_hours: Option<f64>,
  pub rem_sleep_hours: Option<f64>,
  pub sleep_efficiency_pct: Option<f64>,

  // 7-day trends
  pub sleep_avg_7d: Option<f64>,
  pub sleep_debt_hours: Option<f64>,  // cumulative shortfall vs 8hr target

  // HRV (raw values, not scores)
  pub hrv_last_night: Option<f64>,     // milliseconds
  pub hrv_avg_7d: Option<f64>,
  pub hrv_trend_direction: Option<String>, // "declining", "stable", "improving"
  pub hrv_declining_days: Option<u8>,  // consecutive days down

  // Resting HR
  pub resting_hr: Option<i64>,
  pub resting_hr_avg_7d: Option<i64>,
  pub resting_hr_trend: Option<String>, // "up", "stable", "down"
}

impl Default for OuraContext {
  fn default() -> Self {
    Self {
      sleep_duration_hours: None,
      deep_sleep_hours: None,
      rem_sleep_hours: None,
      sleep_efficiency_pct: None,
      sleep_avg_7d: None,
      sleep_debt_hours: None,
      hrv_last_night: None,
      hrv_avg_7d: None,
      hrv_trend_direction: None,
      hrv_declining_days: None,
      resting_hr: None,
      resting_hr_avg_7d: None,
      resting_hr_trend: None,
    }
  }
}
```

### 3.2 Add Oura to Context Package

**File:** `src-tauri/src/analysis.rs`

```rust
pub struct ContextPackage {
  pub workout: WorkoutContext,
  pub recent_same_type: Vec<RecentWorkoutSummary>,
  pub recent_all: Vec<RecentWorkoutSummary>,
  pub fatigue: FatigueContext,
  pub schedule: ScheduleContext,
  pub allowed_durations: AllowedDurations,
  pub flags: Vec<String>,
  pub user: UserContext,
  pub thresholds: SignificanceThresholds,  // NEW
  #[serde(skip_serializing_if = "Option::is_none")]
  pub oura: Option<OuraContext>,  // NEW
  #[serde(skip_serializing_if = "Option::is_none")]
  pub progression_summary: Option<ProgressionSummary>,
}
```

### 3.3 Oura OAuth Integration

**File:** `src-tauri/src/oura.rs`

```rust
const OURA_CLIENT_ID: &str = "...";  // From Oura developer portal
const OURA_AUTH_URL: &str = "https://cloud.ouraring.com/oauth/authorize";
const OURA_TOKEN_URL: &str = "https://api.ouraring.com/oauth/token";

#[tauri::command]
pub async fn oura_start_auth() -> Result<String, String> {
  // Generate OAuth URL
  // Store state in database
  // Return URL for user to authorize
}

#[tauri::command]
pub async fn oura_complete_auth(code: String) -> Result<(), String> {
  // Exchange code for tokens
  // Store access_token and refresh_token
}

#[tauri::command]
pub async fn oura_sync_data(
  state: State<'_, Arc<AppState>>,
) -> Result<OuraSyncResult, String> {
  // Fetch sleep data (last 7 days)
  // Fetch HRV data (last 7 days)
  // Fetch resting HR data
  // Store in oura_sleep and oura_hrv tables
}
```

### 3.4 Oura Database Schema

**Migration:** `20241211000001_add_oura_tables.sql`

```sql
-- Oura authentication
CREATE TABLE oura_auth (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  access_token TEXT,
  refresh_token TEXT,
  expires_at TIMESTAMP,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Sleep data
CREATE TABLE oura_sleep (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date DATE NOT NULL UNIQUE,
  duration_hours REAL,
  deep_sleep_hours REAL,
  rem_sleep_hours REAL,
  light_sleep_hours REAL,
  awake_time_hours REAL,
  efficiency_pct REAL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- HRV data
CREATE TABLE oura_hrv (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date DATE NOT NULL UNIQUE,
  hrv_ms REAL NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Resting HR data
CREATE TABLE oura_resting_hr (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date DATE NOT NULL UNIQUE,
  resting_hr INTEGER NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### 3.5 Oura Data Fetch Functions

```rust
async fn fetch_oura_context(db: &DbPool) -> Result<OuraContext, String> {
  // Get last night's sleep
  let last_night = sqlx::query_as::<_, (f64, f64, f64)>(
    "SELECT duration_hours, deep_sleep_hours, rem_sleep_hours
     FROM oura_sleep
     ORDER BY date DESC LIMIT 1"
  ).fetch_optional(db).await?;

  // Get 7-day sleep average
  let sleep_avg = sqlx::query_as::<_, (f64,)>(
    "SELECT AVG(duration_hours)
     FROM oura_sleep
     WHERE date >= date('now', '-7 days')"
  ).fetch_one(db).await?;

  // Get HRV trend
  let hrv_data = sqlx::query_as::<_, (f64, f64)>(
    "SELECT hrv_ms,
            (SELECT AVG(hrv_ms) FROM oura_hrv WHERE date >= date('now', '-7 days')) as avg_7d
     FROM oura_hrv
     ORDER BY date DESC LIMIT 1"
  ).fetch_optional(db).await?;

  // Compute HRV trend direction
  let hrv_trend = compute_hrv_trend(db).await?;

  // Build OuraContext
  Ok(OuraContext {
    sleep_duration_hours: last_night.map(|(d, _, _)| d),
    deep_sleep_hours: last_night.map(|(_, deep, _)| deep),
    rem_sleep_hours: last_night.map(|(_, _, rem)| rem),
    sleep_avg_7d: Some(sleep_avg.0),
    sleep_debt_hours: compute_sleep_debt(&sleep_avg.0),
    hrv_last_night: hrv_data.map(|(hrv, _)| hrv),
    hrv_avg_7d: hrv_data.map(|(_, avg)| avg),
    hrv_trend_direction: hrv_trend,
    // ... etc
  })
}

fn compute_sleep_debt(avg_7d: &f64) -> Option<f64> {
  let target = 8.0;
  let debt = (target - avg_7d) * 7.0;
  if debt > 0.0 { Some(debt) } else { None }
}
```

### 3.6 Update analyze_workout to Include Oura

```rust
// In analyze_workout command
let oura_context = fetch_oura_context(&state.db).await.ok();  // Optional

context_package.oura = oura_context;
```

---

## Phase 2: Prompt Updates for Thresholds & Oura

### 2.1 Add Thresholds Section to V4 Prompt

```
⸻ SIGNIFICANCE THRESHOLDS ⸻

The context includes `thresholds` object. Use these to determine if deltas are meaningful:

- HR delta: Only comment if abs(delta) > thresholds.hr_delta_significant (5 beats)
- Efficiency: Only comment if abs(delta) > thresholds.efficiency_delta_significant (3%)
- Pace: Only comment if delta > thresholds.pace_delta_significant (10 sec/km)
- Power: Only comment if delta > thresholds.power_delta_significant (10W)

If delta is below threshold:
- Note trend is "stable" or "holding steady"
- Do NOT invent meaning ("suggests emerging adaptation")

Example:
- HR 136 vs 139 last session (3-beat delta) → "HR stable around 136-139 range"
- NOT: "HR dropped 3 beats, showing improved efficiency"
```

### 2.2 Add Oura Usage Rules

```
⸻ OURA DATA USAGE ⸻

If `oura` field is present, you may reference sleep and HRV data. Rules:

**Sleep Context:**
- Use sleep_duration_hours to explain performance variance
  - "Despite 6hrs sleep (below 8hr target), pace held steady"
- Use sleep_debt_hours if >3hrs accumulated
  - "Sleep debt: 4hrs accumulated this week - explains slow TSB recovery"
- Only mention if sleep < 7hrs OR sleep_debt > 3hrs

**HRV Context:**
- Use hrv_trend_direction for recovery assessment
  - "HRV declining 3 days (−22ms total) - early overtraining signal"
- Use hrv_declining_days if >=3
  - Priority flag in Eyes On card
- Do NOT invent HRV insights if data is None
- Do NOT reference Oura "readiness score" (we don't use it)

**Resting HR:**
- Use resting_hr_trend if "up" for multiple days
  - "Resting HR up 5 beats from weekly average - body still stressed"

If Oura data is None/absent:
- Do NOT mention it
- Do NOT say "Oura data unavailable"
- Just proceed without sleep/HRV context
```

### 2.3 Refine Each Card Section

**Performance Card:**
```
- If abs(delta) < thresholds.pace_delta_significant: say "stable"
- Multi-workout trend: show last 3 values explicitly
- Format: "Last 3 runs: 7:20 → 7:18 → 7:22 (stable)"
```

**HR/Efficiency Card:**
```
- Zone correctness FIRST
- HR trend only if >5 beat delta
- Efficiency only if >3% change AND 3+ workouts
- Oura HRV context (if present): "HRV down 15ms overnight - explains elevated HR"
```

**Training Status Card:**
```
Add explicit engine state line:
- "You are in a hold week; progressions paused until [condition]"
- TSB with trend arrow: "TSB: -12 (moderate) ↗️ improving"
- Top 2 flags with current values
```

**Tomorrow Card:**
```
MUST include explicit duration rationale:
"SHORT duration (40 min) because TSB is moderate_fatigue and volume_spike is active"

Use pre-computed confidence from context:
confidence: context.prescription_context.confidence
```

**Eyes On Card:**
```
Each priority MUST include why_it_matters:
{
  "flag": "intensity_heavy",
  "current_value": "52% Z3+",
  "threshold": "<40%",
  "action": "All runs Z2 for next 5 sessions",
  "why_it_matters": "Sustained high intensity increases injury risk and delays recovery"
}
```

---

## Phase 3: Frontend Updates for Oura

### 3.1 Update CoachCards Component

**Eyes On card - add "why it matters":**
```tsx
<div className="priority-item">
  <div className="priority-header">
    <strong className="flag-name">{item.flag}</strong>
    {item.current_value && (
      <span className="current-value">{item.current_value}</span>
    )}
  </div>
  <div className="priority-body">
    <div className="priority-action">{item.action}</div>
    <div className="priority-threshold">Target: {item.threshold}</div>
    <div className="priority-why">{item.why_it_matters}</div>
  </div>
</div>
```

### 3.2 Add Oura Tab (Separate View)

**File:** `src/components/OuraTab.tsx` (new)

Show detailed sleep/recovery metrics:
- Sleep chart (last 7 days)
- HRV trend chart
- Resting HR trend
- Sleep debt indicator
- Last night's sleep breakdown (deep/REM/light)

### 3.3 Oura Connection UI

**In App.tsx:**
```tsx
<div className="card">
  <h2>Oura Connection</h2>
  {ouraStatus?.is_connected ? (
    <div>
      <p>Connected</p>
      <button onClick={syncOuraData}>Sync Data</button>
    </div>
  ) : (
    <button onClick={connectOura}>Connect Oura</button>
  )}
</div>
```

---

## Phase 4: Testing & Validation

### 4.1 Test Edge Cases

Create test scenarios:
1. **Sparse data** - Only 2 recent workouts (verify no crashes)
2. **Missing Oura** - Context with oura: None (verify no hallucinations)
3. **High fatigue** - TSB < -20 (verify Eyes On prioritization)
4. **All signals green** - No flags, good adherence (verify celebratory tone)
5. **Conflicting signals** - Volume spike + good pace (verify medium confidence)

### 4.2 Validate Thresholds

Run analysis on workouts with:
- 2-beat HR delta (should say "stable")
- 15-beat HR delta (should comment)
- 1% efficiency change (should skip)
- 5% efficiency change (should comment)

### 4.3 Oura Integration Test

1. Mock Oura data with:
   - Sleep: 6.5hrs (below target)
   - HRV declining 4 days
   - Resting HR up 8 beats

2. Verify cards mention:
   - Performance: "Despite short sleep..."
   - HR/Efficiency: "HRV down, explains elevated HR"
   - Eyes On: "Sleep debt accumulating" as priority

---

## Implementation Order

1. ✅ **V4 system complete** (done this session)
2. **Add significance thresholds** (30 min)
   - Add SignificanceThresholds struct
   - Pass to context
   - Update prompt
3. **Add TSB trend** (20 min)
   - Implement compute_tsb_trend
   - Add to FatigueContext
4. **Add flag priority sorting** (15 min)
   - to_prioritized_list method
   - Update context
5. **Compute confidence in Rust** (20 min)
   - Add compute_prescription_confidence
   - Pass to context
6. **Update V4 prompt with refinements** (30 min)
   - Noise thresholds
   - Duration rationale structure
   - Why it matters for flags
7. **Oura OAuth** (60 min)
   - Auth flow
   - Token storage
8. **Oura data sync** (60 min)
   - API integration
   - Database schema
   - Data fetch
9. **Oura context integration** (30 min)
   - Add to analyze_workout
   - Update prompt with Oura rules
10. **Oura tab UI** (45 min)
    - Sleep charts
    - HRV visualization
11. **Testing** (60 min)
    - Edge cases
    - Threshold validation
    - Oura integration

**Total estimated time:** 6-7 hours

---

## Success Criteria

**Thresholds & Refinements:**
- ✅ Small deltas (<5 beats, <3% efficiency) labeled as "stable"
- ✅ Confidence computed in Rust, not guessed by LLM
- ✅ TSB shows trend arrow (↗️ ↘️ ➡️)
- ✅ Flags sorted by priority in Rust
- ✅ Eyes On includes "why it matters"

**Oura Integration:**
- ✅ OAuth flow complete
- ✅ Sleep/HRV data synced daily
- ✅ Oura context appears in cards when relevant
- ✅ No hallucinated Oura data when None
- ✅ Separate Oura tab for detailed view

**Quality:**
- ✅ No over-interpretation of noise
- ✅ No contradictions between cards
- ✅ Celebratory tone when earned
- ✅ Informative warnings without guilt
