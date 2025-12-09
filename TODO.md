# Trainer Log - Future Features

## Goal Tracking System
**Priority:** Medium
**Status:** Deferred

### Summary
Add a flexible goal tracking system that lets users define training goals with target dates and key metrics.

### Design
```rust
Goal {
  id,
  name: String,              // "Kilimanjaro", "Marathon", custom
  goal_type: String,         // "endurance", "distance", "event"
  target_date: Option<Date>,
  key_metrics: Vec<String>,  // ["elevation_gain", "longest_session", ...]
  thresholds: HashMap,       // { "weekly_elevation": 500, "longest_hike": 4hrs }
}
```

### Features
- Goal CRUD in UI
- Deterministic layer computes goal progress based on `key_metrics`
- Flags gaps based on `thresholds`
- Claude contextualizes progress toward goals

### Why deferred
Focus first on core analysis layer. Goals add complexity but aren't required for v1 workout analysis.
