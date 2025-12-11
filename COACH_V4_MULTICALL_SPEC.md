# Coach V4 - Multi-Call Card System

## Core Insight

**Problem:** Asking one LLM call to generate "everything" creates generic, unfocused output.

**Solution:** Make 5 **sequential** LLM calls - one per card - where each call:
- Has a hyper-focused prompt for that metric category
- Receives the **insights from previous cards** as context
- Builds on the narrative without repeating information

This creates a **contextualized conversation** instead of isolated analysis:
1. "Let me look at your pace trend..." → Performance insight
2. "Given that pace pattern, how's your HR responding?" → HR insight
3. "Okay, so given those trends, where does this fit in the plan?" → Status
4. "Based on all that, what should tomorrow be?" → Prescription
5. "What should you watch out for?" → Flags

Each card is focused BUT aware of what came before.

---

## Card 1: Pace/Power Performance

**Question:** "How is my pace/power trending?"

**Context needed:**
- `workout.{pace_min_km OR avg_watts, avg_hr, duration_min}`
- `recent_same_type` (last 5)
- `workout.structure` (is this structured?)

**Prompt focus:**
- Compare pace/power to recent similar workouts
- Note if HR is coupling/decoupling from pace/power
- 1-2 sentences max

**Example output:**
```
Power dropped 10W vs yesterday (142W → 132W) with HR down 12 beats.
That's a lighter prescription, not a fitness drop.
```

**Token budget:** 150 tokens

---

## Card 2: HR & Efficiency

**Question:** "How is my cardiovascular response?"

**Context needed:**
- `workout.{avg_hr, zone, efficiency}`
- `recent_same_type.{avg_hr, efficiency}` (trend)
- `user.{max_hr, lthr}` (% calculations)
- `fatigue.tsb_band`

**Prompt focus:**
- HR zone appropriateness for the session intent
- Efficiency trend (if meaningful)
- Is HR creeping up or staying controlled?
- 1-2 sentences max

**Example output:**
```
HR averaged 114 BPM (60% max) - firmly Z2 as intended.
Efficiency holding steady around 1.15 W/BPM.
```

**Token budget:** 150 tokens

---

## Card 3: Training Status

**Question:** "Where am I in the plan right now?"

**Context needed:**
- `fatigue.{tsb, tsb_band, tsb_trend}`
- `progression_summary.adherence`
- `flags` (all of them, LLM triages to top 2)
- `progression_summary.dimensions[*].engine_decision` (summary only)

**Prompt focus:**
- Current fatigue state with trend arrow
- Top 2 priority flags
- Weekly adherence
- Overall progression state (all hold? one building? at ceiling?)
- 2-3 bullet points max

**Example output:**
```
🟠 TSB: -12 (moderate fatigue) ⬆ improving slowly
⚠️ Volume spike active
📊 4/6 sessions this week
🔄 All progressions on hold until volume stabilizes
```

**Token budget:** 200 tokens

---

## Card 4: Tomorrow

**Question:** "What should I do tomorrow?"

**Context needed:**
- `schedule.{tomorrow_is, tomorrow_expected_type}`
- `allowed_durations` (for that modality)
- `fatigue.{tsb_band, tsb_trend}`
- `flags` (only actionable ones: high_fatigue, volume_spike)
- `progression_summary.dimensions` (to know if any progression is pending)

**Prompt focus:**
- Activity type (from schedule)
- Duration (from allowed_durations, named bucket)
- Intensity prescription
- One-line goal statement (load management / aerobic development / progression readiness)
- Confidence level
- 2-3 lines max

**Example output:**
```
Easy 40-min Z2 run (SHORT duration)
Goal: Load management - keeping volume moving without adding intensity
Confidence: High
```

**Token budget:** 150 tokens

---

## Card 5: Eyes On (Watch This)

**Question:** "What should I be monitoring?"

**Context needed:**
- `flags` (all)
- `progression_summary.dimensions[*].engine_decision` (for context)
- `fatigue.tsb`
- `progression_summary.adherence`

**Prompt focus:**
- Flags 3+ (first 2 went to Training Status)
- Each flag gets ONE concrete action with threshold
- Sorted by priority (high_fatigue → volume → intensity → gaps)
- Imperative sentences only

**Example output:**
```
• Drop intensity to under 40% Z3+ (currently 52%)
• Complete Saturday's 30-min run; do not extend
• All sessions Z2 until TSB > -10
```

**Token budget:** 200 tokens

---

## Implementation Architecture

### Backend Changes (Rust)

```rust
// New multi-call analysis function
pub async fn analyze_workout_multicall(
  workout_id: i64,
) -> Result<MultiCardAnalysis, AnalysisError> {
  // 1. Fetch workout + context (same as before)
  // 2. Make 5 parallel LLM calls with focused prompts
  // 3. Combine into MultiCardAnalysis struct
  // 4. Store combined result in DB (for caching)
}

#[derive(Serialize)]
pub struct MultiCardAnalysis {
  pub performance: PerformanceCard,
  pub hr_efficiency: HrEfficiencyCard,
  pub training_status: TrainingStatusCard,
  pub tomorrow: TomorrowCard,
  pub eyes_on: Option<EyesOnCard>,
}
```

### Prompt Templates

Create 5 separate prompt files:
- `coach_performance.txt`
- `coach_hr_efficiency.txt`
- `coach_training_status.txt`
- `coach_tomorrow.txt`
- `coach_eyes_on.txt`

Each is hyper-focused on its card's job.

### Parallel vs Sequential?

**Option A: Parallel** (faster, costs more tokens)
- All 5 calls fire simultaneously
- Total time ≈ slowest call (~2-3s)
- Total tokens ≈ 850 (vs current 700 for one big call)

**Option B: Sequential** (slower, slightly cheaper)
- Performance → HR → Status → Tomorrow → Eyes On
- Cards appear one by one
- Total time ≈ 10-12s

**Recommendation:** Parallel. The UX win of instant cards is worth the small token cost increase.

---

## Why This Is Better

### Current System (One Big Call)
```
Input: Everything
Output: Narrative blob that mentions pace, HR, status, tomorrow

LLM thinks: "I need to write a coherent story that covers all these topics"
Result: Generic, transitional language, unfocused
```

### Multi-Call System
```
Call 1: Just pace/power data
Output: Crisp pace comparison

Call 2: Just HR data
Output: Crisp HR assessment

Call 3: Just status data
Output: Crisp plan summary

etc.
```

**LLM thinks:** "I have ONE job: compare today's power to last week's"
**Result:** Focused, no bloat, no transitions

---

## Migration Path

### Phase 1: Build Multi-Call Backend (No Frontend Changes)
1. Add `analyze_workout_v4_multicall()` function
2. Create 5 prompt templates
3. Make parallel calls
4. Map results to legacy `WorkoutAnalysis` format for DB storage
5. Test that it still works with current UI

### Phase 2: Update Frontend to Use Card Data
1. Update `WorkoutAnalysis` interface to include card fields
2. Update `CoachCards` component to use specific card data
3. Add card-specific styling

### Phase 3: Polish
1. Add loading states for each card
2. Add retry logic for failed cards
3. Add caching to avoid re-calling for same workout

---

## Example: Performance Card Prompt

```
You are analyzing the pace/power performance of one workout.

CONTEXT:
- Today: {workout.activity_type} for {workout.duration_min} min
- Pace/Power: {workout.pace_min_km OR workout.avg_watts}
- Recent similar workouts: {recent_same_type}
- Is structured: {workout.structure.is_structured}

TASK:
Compare today's pace/power to at least ONE recent workout. Name the date and show the delta.

If structured (TrainerRoad):
- Treat power differences as prescription changes, NOT athlete decisions
- Focus on execution vs target

RULES:
- 1-2 sentences max
- No physiology speculation
- Concrete numbers only

OUTPUT (plain text, not JSON):
[Your 1-2 sentence comparison]
```

This prompt is laser-focused on ONE job: pace/power comparison. No distractions.

---

## Estimated Token Usage

| Card | Input Tokens | Output Tokens | Total |
|------|--------------|---------------|-------|
| Performance | ~800 | ~100 | ~900 |
| HR/Efficiency | ~800 | ~100 | ~900 |
| Training Status | ~1200 | ~150 | ~1350 |
| Tomorrow | ~900 | ~100 | ~1000 |
| Eyes On | ~600 | ~150 | ~750 |
| **Total** | ~4300 | ~600 | **~4900** |

Current single-call system: ~5200 tokens total

**Cost:** Slightly cheaper + better quality

---

## Open Questions

1. **Caching strategy?**
   - Cache all 5 card results together?
   - Or allow re-running individual cards?

2. **Error handling?**
   - If one card fails, show the others?
   - Or fail the whole analysis?

3. **Progressive loading?**
   - Show cards as they complete?
   - Or wait for all 5?

4. **Card dependencies?**
   - Do any cards need results from others?
   - Or are they truly independent?

---

## Success Criteria

✅ Each card feels purpose-built for its metric category
✅ No generic transitional language
✅ Cards can be read in any order
✅ Total analysis time < 5 seconds
✅ Each card passes the "would a coach say this?" test
