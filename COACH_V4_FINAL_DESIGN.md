# Coach V4 - Final Card Design

## Design Principles

1. **Assume all rides are structured** - Don't discuss "adherence to TrainerRoad target" as a focus
2. **Weather-aware** - Pull conditions from Strava, use as context for performance variance
3. **Oura sleep data, not proprietary metrics** - Use sleep hours/stages, skip readiness score
4. **Informative, not chiding** - Present patterns and thresholds without judgment
5. **Keep plan simple** - Tomorrow card is straightforward prescription

---

## Card 1: Performance (Pace/Power)

**Question:** "Is my fitness progressing?"

**Data:**
- `workout.{pace_min_km OR avg_watts, duration_min, distance_km}`
- `recent_same_type` (last 5, focus on last 3 for trend)
- `weather.temperature` (from Strava metadata)
- `workout.elevation_gain` (if significant)

**Focus:**
- **Multi-workout trend** (not just vs yesterday)
  - "Last 3 Z2 rides: 142W → 138W → 135W → 132W"
- **Pace-at-HR** (running) or **Power-at-HR** (cycling)
  - "Same pace but HR dropped 12 beats over 3 sessions"
- **Weather context when relevant**
  - "10°F colder than last week's ride - power naturally lower"
- **Avoid discussing "target adherence"** - assume rides are prescribed

**Example:**
```
Last 3 Z2 rides averaged 140W; today's 132W continues the downward trend.
Temperature dropped 12°F since Monday - power variance expected.
HR down from 126 → 114 BPM suggests good aerobic adaptation despite lighter loads.
```

---

## Card 2: HR & Efficiency

**Question:** "How is my cardiovascular system responding?"

**Data:**
- `workout.{avg_hr, zone, efficiency}`
- `recent_same_type.{avg_hr, efficiency}` (trend)
- `user.{max_hr, lthr}`
- `fatigue.tsb_band`
- **Oura (future):**
  - `oura.hrv_avg_last_7_days`
  - `oura.resting_hr_trend`
  - `oura.sleep_hours_last_night`

**Focus:**
- **Zone appropriateness**
  - "Averaged 114 BPM (60% max) - firmly Z2"
- **HR decoupling/coupling**
  - "HR dropping at same power = aerobic adaptation"
  - "HR creeping up at same pace = early fatigue signal"
- **Efficiency trend** (only if meaningful - skip for sparse data)
- **Within-workout drift** (if detectable)
  - "HR held steady throughout vs last week's 8-beat climb"
- **Oura context (when available):**
  - "HRV dropped 15ms overnight - explains elevated HR"
  - "6hrs sleep vs 7.5hr average - body still recovering"

**Example:**
```
HR averaged 114 BPM (60% max) - firmly Z2 throughout.
Efficiency holding steady at 1.15 W/bpm.
[With Oura] HRV down 12ms from weekly average - explains why HR felt slightly elevated.
```

---

## Card 3: Training Status

**Question:** "Where am I in the plan?"

**Data:**
- `fatigue.{tsb, tsb_band, tsb_trend}`
- `flags` (all - triage to top 2 for this card)
- `progression_summary.{adherence, dimensions}`
- `training_context.{workouts_this_week, consistency_pct}`
- **Oura (future):**
  - `oura.sleep_avg_last_7_days`
  - `oura.hrv_trend` (7-day direction)
  - `oura.sleep_debt` (cumulative)

**Focus:**
- **Fatigue with trajectory**
  - "TSB: -12 (moderate) ↗️ improving from -18 on Monday"
- **Top 2 flags with current values**
  - "Volume spike: 7.2hrs vs 6hr plan (20% over)"
  - "Intensity: 52% Z3+ vs 30% target"
- **Adherence + streaks**
  - "5/6 sessions (83%) - 5 days in a row"
- **Progression state**
  - "All progressions on hold" OR "Building long run toward 35 min"
- **Derived insights from combinations**
  - "Volume spike + intensity heavy = classic overload pattern"
- **Oura sleep context (when available):**
  - "Sleep averaged 6.8hrs vs 8hr need - explains slow TSB recovery"

**Example:**
```
🟠 TSB: -12 (moderate) ↗️ improving
⚠️ Volume spike: 7.2hrs (20% above plan)
⚠️ Intensity: 52% Z3+ (target: <30%)
📊 5/6 sessions - 5-day streak
🔄 All progressions on hold until load normalizes
[With Oura] Sleep: 6.8hr avg vs 8hr need - body needs more recovery time
```

---

## Card 4: Tomorrow

**Question:** "What's the prescription?"

**Data:**
- `schedule.{tomorrow_expected_type, tomorrow_is}`
- `allowed_durations.{z2_ride OR run_duration_options}`
- `fatigue.tsb_band`
- `progression_summary.dimensions` (any ready to progress?)
- **Oura (future):**
  - `oura.readiness_band` (high/medium/low - derived from sleep + HRV)

**Focus:**
- **Activity from schedule** (no choice)
- **Duration from allowed_durations** with bucket name
- **Intensity from TSB + flags**
- **Goal statement** (load_management / aerobic_development / progression_readiness)
- **Confidence** (high when signals align, medium when mixed)
- **NO Oura proprietary scores** - just use sleep quality as context

**Example:**
```
Easy 40-min Z2 run (SHORT duration)
Goal: Load management - keep volume moving without adding stress
Rationale: TSB -12 + volume spike + intensity heavy = need easy day
[With Oura] Last night: 7.2hrs sleep (good) - body ready for easy volume
Confidence: High
```

---

## Card 5: Eyes On

**Question:** "What patterns need attention?"

**Data:**
- `flags` (all actionable, sorted by priority)
- `progression_summary.dimensions` (for completion context)
- `fatigue.tsb`
- **Oura (future):**
  - `oura.hrv_declining_days` (consecutive days HRV dropped)
  - `oura.sleep_debt_hours` (cumulative shortfall)

**Focus:**
- **Flag name + current state + threshold + action**
- **Time-bound when possible** ("next 3-4 sessions" not "until better")
- **Consequence framing without guilt**
  - "This pattern typically precedes injury if it continues 7+ days"
- **Positive when earned**
  - "Adherence strong - progressions unlock when TSB > -10"
- **Oura early warnings (when available):**
  - "HRV declining 4 days in a row - early overreaching signal"
  - "Sleep debt: 4hrs accumulated this week - explains slow recovery"

**Priority order (Rust-enforced):**
1. `high_fatigue` (TSB < -20)
2. `volume_spike`
3. `intensity_heavy`
4. `long_run_gap` / `long_ride_gap`
5. HRV/sleep warnings (Oura)
6. Informational flags

**Example:**
```
• Intensity: Currently 52% Z3+ → Target: <40% → All runs Z2 for next 5 sessions
• Volume spike: 20% above average → Hold at current durations until TSB > -10
[With Oura] HRV declining 3 days (-22ms total) → Early overtraining signal, back off if continues
• Long run gap: 21 days since 30+ min → Complete Saturday's session, do not skip
```

---

## ✅ Recovery Integration (IMPLEMENTED)

### Recovery Signals Structure

Full recovery monitoring system with deterministic computation:

```rust
pub struct RecoverySignals {
  sleep: SignalAxis,      // Sleep debt, 7d avg, 28d baseline
  hrv: SignalAxis,        // Current vs baseline delta, trend, declining days
  rhr: SignalAxis,        // Current vs baseline delta, trend, rising days
  overall_band: RecoveryBand,  // Green/Yellow/Orange/Red
  constraints: RecoveryConstraints,  // Intensity cap, duration bias, progression gate
  flags: Vec<RecoveryFlag>,  // SleepDebtAccumulating, HrvBelowBaseline, etc.
}
```

Added to `ContextPackage`:
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub recovery_signals: Option<RecoverySignals>,
```

### Recovery Band Logic

**Band Thresholds:**
- **Sleep**: Debt-based (>1h = Yellow, >2.5h = Orange, >4h = Red)
- **HRV**: Delta from 28d baseline (-5ms = Yellow, -15ms = Orange, -25ms = Red)
- **RHR**: Elevation from 28d baseline (+2bpm = Yellow, +5bpm = Orange, +8bpm = Red)
- **Overall**: Takes worst signal (conservative approach)
- **Exception**: HRV Orange + Sleep Green + RHR Green = Yellow (HRV is noisier)

**Constraints by Band:**
- **Green**: Normal intensity, Standard duration, gate:false
- **Yellow**: ModerateOnly intensity, Standard duration, gate:false
- **Orange**: EasyOnly intensity, Short duration, gate:true
- **Red**: RecoveryOnly intensity, Short duration, gate:true

### LLM Integration (coach_system_v4.txt)

**Card 2: HR & Efficiency**
- Links elevated HR to recovery metrics when Yellow/Orange/Red
- Example: "HR 10bpm higher - aligns with Orange recovery (HRV -15ms vs baseline)"

**Card 3: Training Status**
- Includes recovery_band and recovery_note fields (optional)
- Recovery explains TSB: "Orange recovery (sleep debt 2.5h) explains why -12 TSB feels harder"
- Recovery flags in priority order (Red/Orange > training flags > Yellow)

**Card 4: Tomorrow**
- Recovery constraints inform intensity/duration recommendations
- EasyOnly cap → recommend Z1-Z2 even if TSB allows harder
- Example: "Orange recovery (sleep debt 3.0h) + TSB -16.8 = short easy session"

**Card 5: Eyes On**
- Recovery flags appear if not in top 2
- Format: flag, current_value, threshold, action, why_it_matters

### UI Implementation

**Recovery Tab:**
- Recovery band badge with color-coded border
- 3 signal cards (Sleep, HRV, RHR) with current/7d/28d/delta display
- Charts for 7-day trends
- Recovery Constraints card showing caps and progression gate
- Recovery Flags card with detailed explanations

**Coach Cards:**
- Training Status card optionally displays recovery band and note
- Color-coded recovery row (🟢🟡🟠🔴)
- Recovery context explains training load patterns

---

## Weather Integration

### Data from Strava

Strava activity payload includes:
```json
{
  "average_temp": 8.0,  // Celsius
  "weather_conditions": "Clear",
  "wind_speed": 12.5
}
```

### Add to ContextPackage

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherContext {
  pub temperature_c: Option<f64>,
  pub conditions: Option<String>,
}

// In WorkoutContext
pub weather: Option<WeatherContext>,
```

### Prompt Guidance

**Performance card only:**
- "If temperature differs >10°C from comparison workout, note as context for pace/power variance"
- "Example: '12°C colder than Monday - power naturally 5-10W lower in cold'"

---

## ✅ V4 Implementation Status: COMPLETE

### Rust Backend
- [x] V4 types in `llm.rs` (WorkoutAnalysisV4, all card types)
- [x] V4 prompt (`coach_system_v4.txt`) with 5 card sections + recovery rules
- [x] Oura integration with RecoverySignals struct (actual measurements, not scores)
- [x] `analyze_workout` uses V4 format with recovery context
- [ ] Weather extraction from Strava sync (future enhancement)

### Frontend
- [x] `CoachCards` component uses V4 structured data
- [x] Type-safe props for each card (TypeScript interfaces)
- [x] Recovery tab with detailed signal display
- [x] Tab navigation component
- [x] Chart.js integration for trend visualization

### Testing & Quality
- [x] 102 tests passing (~58% coverage)
- [x] Recovery signal computation fully tested
- [x] All assertions verify actual behavior
- [x] Zero clippy warnings

### Future Enhancements
- [ ] Weather display on workout list
- [ ] Automatic sync timer (currently manual)
- [ ] Recovery trend analytics dashboard
- [ ] Sleep target in user settings (currently hardcoded 7.0h)

---

## Summary

**V4 + Recovery Architecture (DEPLOYED):**
- ✅ One call, structured JSON, 5 card fields
- ✅ Recovery signals from Oura sleep periods (actual values, not scores)
- ✅ TSB remains primary, recovery is advisory context
- ✅ Each card has clear data requirements
- ✅ Assume structured rides (don't discuss target adherence)
- ✅ Informative tone, not chiding
- ✅ TrainerRoad-style red/yellow/green advisory system

**Production ready!** Clean separation maintained: data layer (Rust) → LLM layer (prompt + JSON) → UI layer (cards).
