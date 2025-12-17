# Trainer Log Handoff Document

**Last Updated:** 2025-12-17

## Project Status: v0.4 - Recovery Integration + Comprehensive Testing Complete

The recovery monitoring system is fully integrated with deterministic signal computation, detailed UI, and LLM coaching integration. Comprehensive test suite expanded to 102 tests (~58% coverage). All Oura API endpoint issues resolved. System is production-ready and validated end-to-end.

## What's Working

### Core Infrastructure (v0.1-v0.2)
- Tauri menubar app scaffold
- SQLite database with migrations
- Strava OAuth flow (authorization + token refresh)
- Workout sync and storage
- Deterministic analysis layer (Tier 1 & 2 metrics: ATL, CTL, TSB)
- LLM integration with Claude API for workout analysis

### Progression Engine (v0.2)
- **Ceiling-based progressions** replacing the old week-based 12-week plan
- **Three dimensions tracked:**
  - `run_interval`: Sequence-based (4:1 → 5:1 → ... → continuous_45)
  - `long_run`: Increment-based (+5 min toward 90 min ceiling)
  - `z2_ride`: **Regulated** by TSB (45-60 min based on fatigue, NOT progressive)
- **Lifecycle states:** `building` → `at_ceiling` → `regressing`
- **Maintenance tracking:** `last_ceiling_touch_at` with cadence thresholds
- **Regression detection:** 21+ days without ceiling touch triggers regression
- **Tauri commands exposed:** `get_progression_dimensions`, `progress_dimension`, `regress_dimension`, `touch_ceiling`, `set_dimension_ceiling`

### Coach Analysis V4 (Current)
- **5-card structured output**: Performance, HR/Efficiency, Training Status, Tomorrow, Eyes On
- **Multi-workout trend analysis**: Compares across last 3+ sessions, not just yesterday
- **Rich comparison context**: LLM receives `recent_same_type` (last 5) + `recent_all` (last 7)
- **Schedule awareness**: Knows what day it is, what tomorrow should be
- **Structured workout detection**: All rides marked as TrainerRoad
- **Fatigue-aware prescriptions**: `allowed_durations` computed from TSB bands
- **Strava-like voice**: Conversational, confident, occasionally playful
- **Card-based UI**: All 5 cards rendering with proper TypeScript types

### Recovery Integration (v0.3 - Complete)
- **Oura OAuth**: Connect/disconnect, automatic token refresh
- **Data sync**: Sleep periods endpoint with actual measurements (NOT scores)
- **Recovery signals**: Deterministic computation of sleep/HRV/RHR axes
- **Band determination**: Green/Yellow/Orange/Red based on sleep debt, HRV delta, RHR delta
- **Recovery constraints**: Intensity cap, duration bias, progression gate
- **Recovery tab UI**: Band badge, 3 signal cards with baseline deltas, constraints, flags, charts
- **LLM integration**: Recovery signals in ContextPackage, Claude uses in Tomorrow recommendations
- **Tauri commands**: get_recovery_signals, get_oura_history
- **Tested**: 12 recovery-specific tests, end-to-end validation with real Oura data

### Test Coverage (v0.4 - Complete)
- **102 total tests** (28 → 102, +264% increase)
- **~58% code coverage** across all core modules
- **Strong assertions**: All tests verify actual behavior (DB changes, response values, error types)
- **Modules tested**: analysis (20), progression (17), LLM (13), commands (22), OAuth (10), recovery (12), utils (4), db (2)
- **Test quality**: No meaningless assertions, all tests would catch real regressions
- **CI**: All tests passing in ~2 minutes with clippy -D warnings

### V3 Prompt Guardrails
- Bans physiology speculation ("neuromuscular fatigue", "cardiovascular adaptation")
- Enforces explicit date-based comparisons
- Requires duration choices from `allowed_durations` buckets
- Hard brevity limits (3 sentences max for analysis)
- Flags must have concrete actions with thresholds

### Tests
All 90 tests passing (+221% increase from 28 base tests):
- **Test infrastructure complete**: `test_utils.rs` provides mock builders and helper functions
- **Commands layer**: Full coverage of Tauri command wrappers (22 tests)
- **Analysis layer**: Tier 1/2 metrics, flags, volume tracking, helpers (20 tests)
- **LLM parsing**: V4 JSON parsing with comprehensive edge cases (13 tests)
- **Progression layer**: Logic + database persistence (17 tests)
- **Oura helpers**: Sleep/HRV trend analysis + OAuth (10 tests)
- **Strava helpers**: OAuth and token management (5 tests)
- **Database operations**: Load/save/regression roundtrips (6 tests)
- **Coverage**: ~58% (core business logic comprehensively tested)
- Clean compilation with zero warnings

## Key Files

| File | Purpose |
|------|---------|
| `src-tauri/src/progression.rs` | Ceiling-based progression engine (~900 lines) |
| `src-tauri/src/commands/progression.rs` | Tauri command wrappers |
| `src-tauri/src/commands/analysis.rs` | Workout analysis with LLM |
| `src-tauri/src/prompts/coach_system.txt` | Coach prompt (ceiling model) |
| `src-tauri/migrations/20241210000001_ceiling_based_progressions.sql` | DB schema |
| `src-tauri/src/test_utils.rs` | Test infrastructure (mock builders, helpers) |
| `SPEC.md` | Full specification (updated for ceiling model) |

## Architecture Decision: Cycling Is Different

Cycling duration is **regulated by TSB**, not progressive:
- TSB >= 0 (fresh): 60 min Z2
- TSB -10 to 0 (moderate): 45 min Z2
- TSB < -10 (fatigued): 30-40 min recovery or skip

Power drifts naturally; duration stays stable. This protects running and lifting recovery for hybrid athletes.

## Next Steps

### Testing Progress (Complete - Branch: `feature/comprehensive-test-coverage`)
- 90 tests passing (+221% from 28 baseline)
- 8 test phases complete (Phases 1-8)
- Coverage: ~58% (all core business logic)
- 10 commits ready for merge
- **What's tested**: ATL/CTL/TSB, flags, volume, progression engine, V4 parsing, DB ops, OAuth helpers
- **What's not tested**: External HTTP APIs (Strava/Oura/Claude calls) - would need mocking library

### Optional Enhancements
1. **Oura context in V4 prompt** - Add sleep/HRV rules to coach_system_v4.txt (dormant until you wear ring)
2. **Weather integration** - Extract temperature from Strava raw_json, add to performance context
3. **UX polish** - Better empty state messages, error handling

### Archived Design Specs
See `docs/archive/` for superseded specifications:
- `COACH_V4_MULTICALL_SPEC.md` - Original multi-call design (superseded by single-call)
- `COACH_V4_SINGLE_CALL_SPEC.md` - Implementation details (merged into FINAL_DESIGN)
- `REFINEMENTS_AND_OURA_PLAN.md` - Planning document (implementation complete)
- `COACH_UI_SPEC.md` - UI patterns (implemented in CoachCards.tsx)

### Future Enhancements
- Oura tab (separate detailed sleep/recovery view)
- Streak detection and celebration
- PR detection
- Weekly summary view

## How to Run

```bash
# Development
cd /Users/samleuthold/Desktop/workshop/projects/trainer-log
npm run tauri dev

# Tests
cd src-tauri
cargo test

# Build
cargo build
```

## Environment

Requires `.env` file in `src-tauri/` with:
```
ANTHROPIC_API_KEY=your_key_here
```

Strava OAuth credentials are configured in Strava developer portal and stored in `sync_state` table after auth.

## Philosophy Reminders

From the coach prompt and progression engine:
- **Rust is source of truth** - LLM explains, never decides
- **No compensatory volume** - Missing days = hold or regress, never cram
- **Ceilings prevent runaway progression** - "Enough for your goal" not "max possible"
- **At-ceiling is success** - Maintenance mode, not stagnation
- **Regression is periodization** - Normal, not failure

## Database

SQLite at `~/Library/Application Support/com.trainer-log.dev/trainer-log.db`

Key tables:
- `workouts` - Strava activity data
- `workout_analysis` - LLM-generated analysis
- `progression_dimensions` - Ceiling-based progression state
- `progression_history` - Audit log of all progressions
- `user_settings` - max_hr, lthr, training_days_per_week

## Recent Commits

- `refactor: replace week-based plan with ceiling-based progression engine` - The big pivot from calendar-driven to criteria-driven progressions
