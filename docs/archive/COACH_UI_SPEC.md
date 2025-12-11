# Coach Analysis UI Spec - Card-Based Layout

## Problem Statement

Current coach analysis presents all information in a single paragraph blob, making it hard to:
- Scan for actionable information
- Distinguish "what happened" from "what to do next"
- See progression status at a glance
- Understand priority of different insights

Users want a Strava Athlete Intelligence-style presentation: scannable, categorized, visually distinct sections.

## Proposed Solution

Break the analysis into **4-5 distinct visual cards**, each answering one specific question:

### 1. Performance Card (Always Present)
**Answers:** "How did this workout compare to recent similar sessions?"

**Content:**
- Trend insight with explicit date comparison
- One judgment line about what this means
- Optional efficiency note if meaningful

**Example:**
```
📊 Performance
Same pace as Monday (7:20) but HR dropped 12 beats - nice aerobic development.
```

**Data source:** `trend_insight` + `performance_interpretation.execution_quality`

---

### 2. Training Status Card (Always Present)
**Answers:** "Where am I in the plan right now?"

**Content:**
- Current fatigue state (TSB with band label + trend arrow)
- Top 2 priority flags only (rest go to Watch This)
- Adherence summary

**Example:**
```
🏃 Training Status
🟠 TSB: -12 (moderate fatigue) ⬆ improving
⚠️ Volume spike active
📊 4/6 sessions this week
```

**Visual indicators:**
- 🟢 TSB > 0 (fresh)
- 🟡 TSB -10 to 0 (slightly fatigued)
- 🟠 TSB -20 to -10 (moderate fatigue)
- 🔴 TSB < -20 (high fatigue)

**Data source:** `fatigue`, `flags_and_priorities` (top 2), `progression_summary.adherence`

**Rust additions needed:**
- TSB trend over last 3-7 days (↗️ improving, ➡️ stable, ↘️ declining)

---

### 3. Progression Status Card (Conditional - only if building)
**Answers:** "What's happening with my long run / intervals / etc?"

**Content:**
- Dimension-by-dimension status
- Current → ceiling
- Engine decision with brief explanation

**Example:**
```
🎯 Progression
• long_run: Hold at 30 min (volume unstable)
• run_interval: Hold at 4:1 (intensity too high)
• z2_ride: Regulated - 40 min based on fatigue
```

**Data source:** `decision_logic`

**Show when:** Any dimension has `engine_decision` other than `at_ceiling` or `regulated`

---

### 4. Tomorrow Card (Always Present)
**Answers:** "What should I do tomorrow?"

**Content:**
- Activity type, duration, intensity
- One-line rationale (must be one of: load management, aerobic development, progression readiness)
- Optional confidence indicator

**Example:**
```
📅 Tomorrow
Easy 40-min Z2 run
Goal: Load management - keeping volume moving without adding intensity.
Confidence: High
```

**Data source:** `tomorrow_prescription`

**LLM requirement:** Rationale must explicitly state the coaching goal (load management / aerobic development / progression readiness)

---

### 5. Watch This Card (Conditional - only if actionable flags)
**Answers:** "What should I be careful about?"

**Content:**
- Prioritized list of active flags (sorted by Rust, not LLM)
- Each with concrete action and threshold

**Priority order (Rust-enforced):**
1. `high_fatigue` (TSB < -20)
2. `volume_spike`
3. `intensity_heavy`
4. `long_run_gap` / `long_ride_gap`
5. Other informational flags

**Example:**
```
⚠️ Watch This
• All runs Z2 until intensity drops below 40% (currently 52%)
• No progressions until TSB > -10
```

**Data source:** `flags_and_priorities` (excluding top 2 shown in Training Status)

**Show when:** `flags_and_priorities.length > 2` (first 2 go to Training Status card)

---

## Visual Design Principles

### Layout
- Each card is a distinct visual box with subtle border/shadow
- Cards stack vertically with consistent spacing
- Mobile-first sizing (menubar app is small screen)

### Typography
- Card headers: Bold, slightly larger, icon prefix
- Content: Clean sans-serif, comfortable line-height
- Numbers/metrics: Monospace or tabular figures

### Color Coding
- 🟢 Green accents: Good trends, on-track status
- 🟡 Yellow accents: Cautions, holds, moderate fatigue
- 🔴 Red accents: High fatigue, critical flags, regressions
- ⚪ Neutral: At ceiling, regulated dimensions

### Icons
- 📊 Performance
- 🏃 Training Status
- 🎯 Progression
- 📅 Tomorrow
- ⚠️ Watch This

---

## Data Structure Changes Needed

### Current V3 Output (stays mostly the same)
```typescript
interface WorkoutAnalysisV3 {
  trend_insight: TrendInsight;
  performance_interpretation: PerformanceInterpretation;
  decision_logic: Record<string, DimensionDecision>;
  tomorrow_prescription: TomorrowPrescription;
  flags_and_priorities: FlagWithAction[];
}
```

### Mapping to Cards
1. **Performance Card**
   - `trend_insight.interpretation`
   - `performance_interpretation.efficiency_note` (if present)

2. **Training Status Card**
   - Computed from `fatigue.tsb` + `fatigue.tsb_band`
   - `flags_and_priorities` (just flag names, not actions)
   - `progression_summary.adherence.adherence_pct`

3. **Progression Card**
   - `decision_logic` entries
   - Filter to dimensions where `engine_decision` is actionable

4. **Tomorrow Card**
   - `tomorrow_prescription.*`

5. **Watch This Card**
   - `flags_and_priorities[*].action` (full actions)

---

## Frontend Component Structure

```tsx
<div className="coach-analysis">
  {/* Always show */}
  <PerformanceCard data={analysis.trend_insight} />
  <TrainingStatusCard
    fatigue={context.fatigue}
    flags={analysis.flags_and_priorities}
    adherence={context.progression_summary.adherence}
  />

  {/* Conditional */}
  {hasActiveProgression && (
    <ProgressionCard decisions={analysis.decision_logic} />
  )}

  {/* Always show */}
  <TomorrowCard prescription={analysis.tomorrow_prescription} />

  {/* Conditional */}
  {analysis.flags_and_priorities.length > 0 && (
    <WatchThisCard flags={analysis.flags_and_priorities} />
  )}
</div>
```

---

## Additional Features to Consider

### 1. Streaks & Milestones (Rust-computed)
- "5 days in a row" badge
- "Longest run this month" celebration
- "3 weeks at ceiling" acknowledgment

### 2. Week Progress Bar
```
This Week: ████░░ 4/6 sessions
```

### 3. Fatigue Trend Sparkline
```
TSB: -12 ↗️ (improving)
Last 7 days: ▁▂▃▄▅▄▃
```

### 4. Quick Actions (Buttons)
- "Skip tomorrow" → adjusts schedule
- "I'm sick" → triggers recovery week
- "Extend this" → requests progression

---

## Implementation Plan

### Phase 1: Restructure LLM Output (No frontend changes yet)
- Keep V3 JSON structure
- Update prompt to think in card categories
- Validate output still maps to legacy format for DB

### Phase 2: Update Frontend Components
- Create card components (PerformanceCard, StatusCard, etc.)
- Update App.tsx to use card layout
- Add conditional rendering logic

### Phase 3: Add Rust-Computed Enhancements
- Streak detection
- Milestone detection
- Week progress calculation
- Add these to context package

### Phase 4: Polish
- Icons
- Color coding
- Animations (card slide-in, etc.)
- Responsive layout

---

## Key Improvements from Feedback

### Prompt-to-Card Alignment
- Update LLM prompt to explicitly state: "Your output will be displayed card-by-card. Write each field as if it will stand alone."
- No cross-references between sections ("as mentioned above")

### Missing Data Fallbacks
- If `recent_same_type.length < 2`: Compare to `recent_all` instead with note "insufficient same-type data"
- If `efficiency == null`: Skip efficiency_note entirely
- If week is incomplete: Show session count but don't judge adherence yet

### Rust-Side Enhancements
1. **TSB trend arrow** - Compute 7-day TSB trend (↗️ improving, ➡️ stable, ↘️ declining)
2. **Flag priority sorting** - Hard-coded order in Rust, not LLM-determined
3. **Hold reason clarity** - Distinguish "hold_due_to_fatigue" vs "hold_due_to_adherence" in engine decision

### Additional Context for LLM
- Week progress: "Session 4 of 6 this week"
- Rolling 7-day intensity distribution
- Days since last progression (already have this)

## Open Questions

1. **Card order priority?**
   - Current proposal: Performance → Status → Progression → Tomorrow → Flags
   - Alternative: Tomorrow first (most actionable)?
   - **Feedback suggests:** Keep current order (past → present → future)

2. **Collapsible cards?**
   - Should "Progression Status" collapse by default if all dimensions are on hold?
   - **Recommendation:** Always show - reinforces "hold is intentional"

3. **Celebration moments?**
   - When to show explicit "🎉 Nice work!" badges?
   - Suggestions: PR detection, streaks (5+ days), clean execution during fatigue
   - **Feedback:** Integrate into Performance card, not separate badge

4. **Confidence indicator?**
   - Show confidence level for tomorrow's prescription?
   - **Feedback:** Yes - helps calibrate strict vs flexible recommendations

---

## Success Criteria

Output is successful if:
- ✅ User can scan and find "what to do tomorrow" in <2 seconds
- ✅ Flags/warnings are visually distinct from neutral info
- ✅ Progression status is clear without reading paragraphs
- ✅ Tone feels conversational, not clinical
- ✅ Good execution is celebrated, not just catalogued
- ✅ The card feels useful even when "nothing is happening" (hold week)
