# Coach V4 - Single Call, Structured Card Output

## Core Design

**One LLM call** with a structured JSON output where each top-level field maps to a specific card.

The prompt guides the model to **think about each card independently** even though it's one response.

---

## V4 JSON Output Structure

```typescript
interface CoachAnalysisV4 {
  // Card 1: Pace/Power Performance
  performance: {
    metric_name: "pace" | "power" | "duration";
    comparison_date: string;  // "2025-12-09"
    comparison_value: string; // "142W" or "7:20/km"
    today_value: string;      // "132W" or "7:22/km"
    delta: string;            // "-10W" or "+2 sec/km"
    insight: string;          // 1-2 sentences, Strava-like
  };

  // Card 2: HR & Efficiency
  hr_efficiency: {
    avg_hr: number;
    hr_zone: string;          // "Z2"
    hr_pct_max: number;       // 60
    hr_assessment: string;    // "Firmly Z2 as intended"
    efficiency_trend?: string; // Optional, only if meaningful
  };

  // Card 3: Training Status
  training_status: {
    tsb_assessment: string;   // "Moderate fatigue, improving slowly"
    top_flags: string[];      // Max 2, triaged by importance
    adherence_note: string;   // "4/6 sessions this week"
    progression_state: string; // "All progressions on hold" or "Building long run"
  };

  // Card 4: Tomorrow
  tomorrow: {
    activity_type: string;    // "run"
    duration_min: number;     // 40
    duration_label: string;   // "SHORT"
    intensity: string;        // "Z2"
    goal: string;             // "load_management" | "aerobic_development" | "progression_readiness"
    rationale: string;        // One sentence why
    confidence: "high" | "medium" | "low";
  };

  // Card 5: Eyes On (optional)
  eyes_on?: {
    priorities: Array<{
      flag: string;
      current_value?: string; // "Currently 52% Z3+"
      threshold: string;      // "Drop below 40%"
      action: string;         // "All runs Z2 only"
    }>;
  };
}
```

---

## Prompt Structure

The system prompt will have **5 distinct sections**, one per card:

```
You are analyzing a workout. Your output will be displayed as 5 separate cards.
Write each card as if it stands alone, but you may reference insights across cards.

⸻ CARD 1: PERFORMANCE (pace/power trend) ⸻

Compare today's [pace OR power] to recent_same_type workouts.
- Pick the most recent similar workout
- Show the delta
- Note if this is execution vs prescription (for structured rides)
- 1-2 sentences, Strava voice

Output to: performance.*

⸻ CARD 2: HR & EFFICIENCY ⸻

Assess cardiovascular response.
- Is HR appropriate for the zone?
- Is efficiency trending up/down/stable?
- Skip efficiency if not meaningful (runs, or sparse data)
- 1-2 sentences

Output to: hr_efficiency.*

⸻ CARD 3: TRAINING STATUS ⸻

Summarize current plan state.
- TSB with band and trend
- Top 2 priority flags (triage: high_fatigue > volume_spike > intensity_heavy > gaps)
- Session count vs expected
- Progression state summary
- 3-4 short bullets

Output to: training_status.*

⸻ CARD 4: TOMORROW ⸻

Prescribe tomorrow's session.
- Respect schedule.tomorrow_expected_type
- Pick duration from allowed_durations (name the bucket)
- State the goal (load_management / aerobic_development / progression_readiness)
- Set confidence based on signal clarity
- 2-3 lines

Output to: tomorrow.*

⸻ CARD 5: EYES ON ⸻

List remaining actionable flags (flags 3+).
- Each flag gets: current value, threshold, action
- Sort by priority
- Skip if <3 flags total

Output to: eyes_on.*
```

---

## Context Data Strategy

We send **everything** in one context package (same as V3), but the prompt guides the model to:
- Use `workout.*` + `recent_same_type` for Performance card
- Use `workout.{avg_hr, zone, efficiency}` for HR card
- Use `fatigue.*` + `progression_summary.*` for Status card
- Use `schedule.*` + `allowed_durations` for Tomorrow card
- Use `flags` for Eyes On card

The model learns to **extract relevant subsets** for each card.

---

## Implementation

### 1. Update Rust Types

```rust
// llm.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutAnalysisV4 {
  pub performance: PerformanceCard,
  pub hr_efficiency: HrEfficiencyCard,
  pub training_status: TrainingStatusCard,
  pub tomorrow: TomorrowCard,
  pub eyes_on: Option<EyesOnCard>,
}

// Each card type matches JSON spec above
```

### 2. Update Prompt

Create `coach_system_v4.txt` with the 5-section structure above.

### 3. Frontend Uses Native Structure

```tsx
<CoachCards analysis={analysisV4} />

// Inside component:
<PerformanceCard data={analysis.performance} />
<HrEfficiencyCard data={analysis.hr_efficiency} />
<TrainingStatusCard data={analysis.training_status} />
<TomorrowCard data={analysis.tomorrow} />
{analysis.eyes_on && <EyesOnCard data={analysis.eyes_on} />}
```

No parsing, no string splitting - direct mapping.

---

## Benefits

1. **One call** - Fast, cacheable, simpler error handling
2. **Structured JSON** - Type-safe, no parsing hacks
3. **Card-focused prompting** - Model thinks about each card's job
4. **Future-proof** - Easy to add Oura card later:
   ```typescript
   oura_insights?: {
     hrv_trend: string;
     sleep_quality: string;
     readiness_score: number;
   }
   ```

---

## Prompt Example for Performance Card Section

```
⸻ CARD 1: PERFORMANCE ⸻

Analyze pace/power trend by comparing to recent_same_type.

Required fields:
- metric_name: "pace" | "power" | "duration"
- comparison_date: ISO date of the workout you're comparing to
- comparison_value: The value from that workout (e.g., "142W")
- today_value: Today's value (e.g., "132W")
- delta: The change (e.g., "-10W")
- insight: 1-2 Strava-style sentences

For structured rides:
- Power differences = prescription changes, not athlete choice
- Example: "TrainerRoad prescribed 10W lighter than yesterday. You executed right on target."

For unstructured workouts:
- You may discuss pacing decisions
- Anchor to fatigue.tsb_band
```

This gives the model a clear schema + examples for just this one card.

---

## Next Steps

Want me to:
1. Implement the V4 JSON structure in Rust
2. Write the V4 prompt with 5-section card guidance
3. Update the frontend to use the structured card data
4. Test it end-to-end

This is the architecture that will let you add Oura, progression insights, streak detection, etc. later without rewriting everything.