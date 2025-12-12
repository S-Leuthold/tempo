//! LLM integration for workout analysis
//!
//! This module handles communication with the Claude API for generating
//! training insights and recommendations.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// ---------------------------------------------------------------------------
/// Configuration
/// ---------------------------------------------------------------------------

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_MODEL: &str = "claude-sonnet-4-20250514";
const API_VERSION: &str = "2023-06-01";

/// ---------------------------------------------------------------------------
/// Error Types
/// ---------------------------------------------------------------------------

#[derive(Error, Debug, Serialize)]
pub enum LlmError {
  #[error("API key not configured")]
  MissingApiKey,

  #[error("Request failed: {0}")]
  Request(String),

  #[error("API error: {0}")]
  Api(String),

  #[error("Parse error: {0}")]
  Parse(String),
}

/// ---------------------------------------------------------------------------
/// Claude API Types
/// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ClaudeRequest {
  model: String,
  max_tokens: u32,
  system: String,
  messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize)]
struct ClaudeMessage {
  role: String,
  content: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
  content: Vec<ContentBlock>,
  #[allow(dead_code)]
  model: String,
  #[allow(dead_code)]
  stop_reason: Option<String>,
  usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
  #[serde(rename = "type")]
  content_type: String,
  text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
  pub input_tokens: u32,
  pub output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ClaudeErrorResponse {
  error: ClaudeErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ClaudeErrorDetail {
  message: String,
}

/// ---------------------------------------------------------------------------
/// Workout Analysis Response (from Claude) - V3 Trend-Focused Format
/// ---------------------------------------------------------------------------

/// V3 analysis format with trend insight and structured prescription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutAnalysisV3 {
  /// Trend analysis comparing to recent workouts
  pub trend_insight: TrendInsight,

  /// Performance interpretation for this workout
  pub performance_interpretation: PerformanceInterpretation,

  /// Decision logic for each dimension (keyed by dimension name)
  pub decision_logic: std::collections::HashMap<String, DimensionDecision>,

  /// Structured prescription for tomorrow
  pub tomorrow_prescription: TomorrowPrescription,

  /// Prioritized flags with actions
  #[serde(default)]
  pub flags_and_priorities: Vec<FlagWithAction>,
}

/// Trend insight comparing to recent similar workouts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendInsight {
  pub metric_compared: String,
  pub direction: String,
  pub delta: String,
  pub interpretation: String,
}

/// Performance interpretation for the current workout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceInterpretation {
  pub execution_quality: String,
  #[serde(default)]
  pub efficiency_note: Option<String>,
  pub context_vs_trend: String,
}

/// Decision logic for a single dimension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionDecision {
  pub engine_decision: String,
  pub explanation: String,
  pub action: String,
}

/// Structured prescription for tomorrow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomorrowPrescription {
  pub activity_type: String,
  pub duration_min: i32,
  pub intensity: String,
  pub rationale: String,
}

/// Flag with action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagWithAction {
  pub flag: String,
  pub action: String,
}

/// ---------------------------------------------------------------------------
/// Workout Analysis Response (from Claude) - V4 Multi-Card Format
/// ---------------------------------------------------------------------------

/// V4 analysis format with purpose-built cards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutAnalysisV4 {
  pub performance: PerformanceCard,
  pub hr_efficiency: HrEfficiencyCard,
  pub training_status: TrainingStatusCard,
  pub tomorrow: TomorrowCard,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub eyes_on: Option<EyesOnCard>,
}

/// Card 1: Pace/power performance trends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceCard {
  pub metric_name: String,
  pub comparison_date: String,
  pub comparison_value: String,
  pub today_value: String,
  pub delta: String,
  pub insight: String,
}

/// Card 2: HR and efficiency assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrEfficiencyCard {
  pub avg_hr: i64,
  pub hr_zone: String,
  pub hr_pct_max: i64,
  pub hr_assessment: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub efficiency_trend: Option<String>,
}

/// Card 3: Training status (fatigue, flags, adherence, progression)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingStatusCard {
  pub tsb_value: f64,
  pub tsb_band: String,
  pub tsb_assessment: String,
  pub top_flags: Vec<String>,
  pub adherence_note: String,
  pub progression_state: String,
}

/// Card 4: Tomorrow's prescription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomorrowCard {
  pub activity_type: String,
  pub duration_min: i32,
  pub duration_label: String,
  pub intensity: String,
  pub goal: String,
  pub rationale: String,
  pub confidence: String,
}

/// Card 5: Eyes on (actionable flags)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EyesOnCard {
  pub priorities: Vec<FlagPriority>,
}

/// Flag with priority, current value, threshold, action, and consequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagPriority {
  pub flag: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub current_value: Option<String>,
  pub threshold: String,
  pub action: String,
  pub why_it_matters: String,
}

/// Convert V4 to legacy format for DB storage
impl From<WorkoutAnalysisV4> for WorkoutAnalysis {
  fn from(v4: WorkoutAnalysisV4) -> Self {
    let summary = format!("{} {}",
      v4.performance.insight,
      v4.hr_efficiency.hr_assessment
    );

    let tomorrow = format!(
      "{} for {} min at {} intensity. {}",
      v4.tomorrow.activity_type,
      v4.tomorrow.duration_min,
      v4.tomorrow.intensity,
      v4.tomorrow.rationale
    );

    let risk_flags = v4.eyes_on
      .map(|eyes| eyes.priorities.into_iter()
        .map(|p| format!("{}: {}", p.flag, p.action))
        .collect())
      .unwrap_or_default();

    Self {
      summary,
      tomorrow_recommendation: tomorrow,
      risk_flags,
      goal_notes: None,
    }
  }
}

/// Legacy V2 format (for backward compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutAnalysisV2 {
  pub workout_analysis: WorkoutBreakdown,
  pub progression: Option<ProgressionResponse>,
  pub plan_status: Option<PlanStatusResponse>,
  pub tomorrow: String,
  pub risk_flags: Vec<String>,
  pub goal_notes: Option<String>,
}

/// Deep workout breakdown (V2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutBreakdown {
  pub summary: String,
  pub execution: String,
  pub hr_insights: String,
  pub comparison: Option<String>,
}

/// Progression status from LLM (V2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressionResponse {
  pub run_interval_status: String,
  pub run_interval_note: String,
  pub long_run_status: Option<String>,
  pub long_run_note: Option<String>,
}

/// Plan status from LLM (V2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStatusResponse {
  pub week_on_track: bool,
  pub adjustment_needed: Option<String>,
}

/// Legacy format (backward compatible - stored in DB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutAnalysis {
  /// Brief summary of the workout
  pub summary: String,

  /// What to do tomorrow based on current state
  pub tomorrow_recommendation: String,

  /// Risk flags or concerns identified
  pub risk_flags: Vec<String>,

  /// Notes specific to training goals
  pub goal_notes: Option<String>,
}

/// Convert V3 to legacy format for storage
impl From<WorkoutAnalysisV3> for WorkoutAnalysis {
  fn from(v3: WorkoutAnalysisV3) -> Self {
    // Build summary from trend insight and performance interpretation
    let summary = format!(
      "{}. {}",
      v3.trend_insight.interpretation,
      v3.performance_interpretation.execution_quality
    );

    // Build recommendation from prescription
    let tomorrow = format!(
      "{} for {} min at {} intensity. {}",
      v3.tomorrow_prescription.activity_type,
      v3.tomorrow_prescription.duration_min,
      v3.tomorrow_prescription.intensity,
      v3.tomorrow_prescription.rationale
    );

    // Extract flag names
    let risk_flags: Vec<String> = v3
      .flags_and_priorities
      .into_iter()
      .map(|f| format!("{}: {}", f.flag, f.action))
      .collect();

    Self {
      summary,
      tomorrow_recommendation: tomorrow,
      risk_flags,
      goal_notes: None,
    }
  }
}

impl From<WorkoutAnalysisV2> for WorkoutAnalysis {
  fn from(v2: WorkoutAnalysisV2) -> Self {
    Self {
      summary: v2.workout_analysis.summary,
      tomorrow_recommendation: v2.tomorrow,
      risk_flags: v2.risk_flags,
      goal_notes: v2.goal_notes,
    }
  }
}

/// ---------------------------------------------------------------------------
/// Claude Client
/// ---------------------------------------------------------------------------

pub struct ClaudeClient {
  client: Client,
  api_key: String,
}

impl ClaudeClient {
  /// Create a new Claude client, loading API key from environment
  pub fn from_env() -> Result<Self, LlmError> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| LlmError::MissingApiKey)?;

    Ok(Self {
      client: Client::new(),
      api_key,
    })
  }

  /// Call Claude with a system prompt and user message
  pub async fn complete(
    &self,
    system_prompt: &str,
    user_message: &str,
    max_tokens: u32,
  ) -> Result<(String, Usage), LlmError> {
    let request = ClaudeRequest {
      model: CLAUDE_MODEL.to_string(),
      max_tokens,
      system: system_prompt.to_string(),
      messages: vec![ClaudeMessage {
        role: "user".to_string(),
        content: user_message.to_string(),
      }],
    };

    let response = self
      .client
      .post(CLAUDE_API_URL)
      .header("x-api-key", &self.api_key)
      .header("anthropic-version", API_VERSION)
      .header("content-type", "application/json")
      .json(&request)
      .send()
      .await
      .map_err(|e| LlmError::Request(e.to_string()))?;

    let status = response.status();
    let body = response
      .text()
      .await
      .map_err(|e| LlmError::Request(e.to_string()))?;

    if !status.is_success() {
      // Try to parse error response
      if let Ok(error_resp) = serde_json::from_str::<ClaudeErrorResponse>(&body) {
        return Err(LlmError::Api(error_resp.error.message));
      }
      return Err(LlmError::Api(format!("HTTP {}: {}", status, body)));
    }

    let claude_response: ClaudeResponse =
      serde_json::from_str(&body).map_err(|e| LlmError::Parse(e.to_string()))?;

    // Extract text from the first text content block
    let text = claude_response
      .content
      .iter()
      .find(|c| c.content_type == "text")
      .and_then(|c| c.text.clone())
      .ok_or_else(|| LlmError::Parse("No text content in response".to_string()))?;

    Ok((text, claude_response.usage))
  }

  /// Analyze a workout and return V4 format (for frontend)
  pub async fn analyze_workout_v4_or_fallback(
    &self,
    context_json: &str,
  ) -> Result<(WorkoutAnalysisV4, Usage), LlmError> {
    // Try V4 first (multi-card), fall back to converting V3/V2/legacy to V4 structure
    match self.analyze_workout_v4(context_json).await {
      Ok((v4, usage)) => {
        println!("LLM returned V4 format");
        Ok((v4, usage))
      }
      Err(e) => {
        println!("V4 parse failed: {}, trying V3", e);
        // V3 fallback - would need conversion logic
        // For now, return error to force V4
        Err(e)
      }
    }
  }

  /// Analyze a workout with structured JSON output (returns legacy format for DB storage)
  #[allow(dead_code)]
  pub async fn analyze_workout(
    &self,
    context_json: &str,
  ) -> Result<(WorkoutAnalysis, Usage), LlmError> {
    // Try V4 first (multi-card), fall back to V3, V2, then legacy
    match self.analyze_workout_v4(context_json).await {
      Ok((v4, usage)) => {
        println!("LLM returned V4 format");
        Ok((v4.into(), usage))
      }
      Err(e) => {
        println!("V4 parse failed: {}, trying V3", e);
        match self.analyze_workout_v3(context_json).await {
          Ok((v3, usage)) => {
            println!("LLM returned V3 format");
            Ok((v3.into(), usage))
          }
          Err(e) => {
            println!("V3 parse failed: {}, trying V2", e);
            match self.analyze_workout_v2(context_json).await {
              Ok((v2, usage)) => Ok((v2.into(), usage)),
              Err(_) => self.analyze_workout_legacy(context_json).await,
            }
          }
        }
      }
    }
  }

  /// Analyze a workout with V4 format (multi-card system)
  async fn analyze_workout_v4(
    &self,
    context_json: &str,
  ) -> Result<(WorkoutAnalysisV4, Usage), LlmError> {
    let system_prompt = include_str!("prompts/coach_system_v4.txt");

    let user_message = format!(
      r#"Analyze this workout and provide card-based coaching feedback.

TRAINING CONTEXT:
{}

Respond with valid JSON matching the V4 OUTPUT STRUCTURE."#,
      context_json
    );

    let (response_text, usage) = self.complete(system_prompt, &user_message, 2500).await?;

    let json_str = extract_json(&response_text)?;

    let analysis: WorkoutAnalysisV4 =
      serde_json::from_str(&json_str)
        .map_err(|e| LlmError::Parse(format!("{}: {}", e, json_str)))?;

    Ok((analysis, usage))
  }

  /// Analyze a workout with V3 format (trend-focused with structured prescription)
  #[allow(dead_code)]
  async fn analyze_workout_v3(
    &self,
    context_json: &str,
  ) -> Result<(WorkoutAnalysisV3, Usage), LlmError> {
    let system_prompt = include_str!("prompts/coach_system.txt");

    let user_message = format!(
      r#"Analyze this workout and provide coaching feedback.

TRAINING CONTEXT:
{}

Respond with valid JSON matching the OUTPUT STRUCTURE specified in your instructions."#,
      context_json
    );

    let (response_text, usage) = self.complete(system_prompt, &user_message, 2000).await?;

    // Parse the JSON response
    let json_str = extract_json(&response_text)?;

    let analysis: WorkoutAnalysisV3 =
      serde_json::from_str(&json_str).map_err(|e| LlmError::Parse(format!("{}: {}", e, json_str)))?;

    Ok((analysis, usage))
  }

  /// Analyze a workout with the V2 format (deep analysis)
  #[allow(dead_code)]
  async fn analyze_workout_v2(
    &self,
    context_json: &str,
  ) -> Result<(WorkoutAnalysisV2, Usage), LlmError> {
    let system_prompt = include_str!("prompts/coach_system.txt");

    let user_message = format!(
      r#"Analyze this workout and provide coaching feedback.

TRAINING CONTEXT:
{}

Respond with valid JSON matching the OUTPUT FORMAT specified in your instructions."#,
      context_json
    );

    let (response_text, usage) = self.complete(system_prompt, &user_message, 1500).await?;

    // Parse the JSON response
    let json_str = extract_json(&response_text)?;

    let analysis: WorkoutAnalysisV2 =
      serde_json::from_str(&json_str).map_err(|e| LlmError::Parse(format!("{}: {}", e, json_str)))?;

    Ok((analysis, usage))
  }

  /// Legacy analysis format (simpler, backward compatible)
  #[allow(dead_code)]
  async fn analyze_workout_legacy(
    &self,
    context_json: &str,
  ) -> Result<(WorkoutAnalysis, Usage), LlmError> {
    let system_prompt = include_str!("prompts/coach_system.txt");

    let user_message = format!(
      r#"Analyze this workout and provide coaching feedback.

TRAINING CONTEXT:
{}

Respond with valid JSON in this exact format:
{{
  "summary": "Brief summary of the workout (1-2 sentences)",
  "tomorrow_recommendation": "Specific recommendation for tomorrow's training",
  "risk_flags": ["flag1", "flag2"],
  "goal_notes": "Optional notes about training progress, or null if nothing relevant"
}}

Be direct and specific. Reference the actual numbers provided."#,
      context_json
    );

    let (response_text, usage) = self.complete(system_prompt, &user_message, 1024).await?;

    let json_str = extract_json(&response_text)?;

    let analysis: WorkoutAnalysis =
      serde_json::from_str(&json_str).map_err(|e| LlmError::Parse(format!("{}: {}", e, json_str)))?;

    Ok((analysis, usage))
  }
}

/// Extract JSON from Claude's response (handles markdown code blocks)
fn extract_json(text: &str) -> Result<String, LlmError> {
  // Try direct parse first
  if text.trim().starts_with('{') {
    return Ok(text.trim().to_string());
  }

  // Look for JSON in code blocks
  if let Some(start) = text.find("```json") {
    let start = start + 7;
    if let Some(end) = text[start..].find("```") {
      return Ok(text[start..start + end].trim().to_string());
    }
  }

  // Look for plain code blocks
  if let Some(start) = text.find("```") {
    let start = start + 3;
    // Skip language identifier if present
    let content_start = text[start..]
      .find('\n')
      .map(|i| start + i + 1)
      .unwrap_or(start);
    if let Some(end) = text[content_start..].find("```") {
      return Ok(text[content_start..content_start + end].trim().to_string());
    }
  }

  // Last resort: find first { to last }
  if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
    return Ok(text[start..=end].to_string());
  }

  Err(LlmError::Parse("Could not extract JSON from response".to_string()))
}

/// ---------------------------------------------------------------------------
/// Tests
/// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_extract_json_direct() {
    let input = r#"{"summary": "test", "risk_flags": []}"#;
    let result = extract_json(input).unwrap();
    assert!(result.contains("summary"));
  }

  #[test]
  fn test_extract_json_code_block() {
    let input = r#"Here's the analysis:

```json
{"summary": "Good workout", "risk_flags": []}
```

Hope that helps!"#;
    let result = extract_json(input).unwrap();
    assert!(result.contains("Good workout"));
  }

  #[test]
  fn test_extract_json_fallback() {
    let input = r#"The analysis is {"summary": "test"} as shown."#;
    let result = extract_json(input).unwrap();
    assert!(result.contains("summary"));
  }

  #[test]
  fn test_v4_to_legacy_conversion() {
    let v4 = WorkoutAnalysisV4 {
      performance: PerformanceCard {
        metric_name: "pace".to_string(),
        comparison_date: "2025-12-09".to_string(),
        comparison_value: "7:20/km".to_string(),
        today_value: "7:22/km".to_string(),
        delta: "+2 sec/km".to_string(),
        insight: "Pace holding steady around 7:20/km across last 3 runs.".to_string(),
      },
      hr_efficiency: HrEfficiencyCard {
        avg_hr: 136,
        hr_zone: "Z2".to_string(),
        hr_pct_max: 72,
        hr_assessment: "HR firmly in Z2 throughout".to_string(),
        efficiency_trend: None,
      },
      training_status: TrainingStatusCard {
        tsb_value: -12.0,
        tsb_band: "moderate_fatigue".to_string(),
        tsb_assessment: "Improving from -18".to_string(),
        top_flags: vec!["volume_spike".to_string()],
        adherence_note: "6/6 sessions - perfect week".to_string(),
        progression_state: "All on hold until load stabilizes".to_string(),
      },
      tomorrow: TomorrowCard {
        activity_type: "Ride".to_string(),
        duration_min: 40,
        duration_label: "SHORT".to_string(),
        intensity: "Z2".to_string(),
        goal: "load_management".to_string(),
        rationale: "TSB -12 + volume spike = keep it short and easy".to_string(),
        confidence: "high".to_string(),
      },
      eyes_on: Some(EyesOnCard {
        priorities: vec![
          FlagPriority {
            flag: "long_run_gap".to_string(),
            current_value: Some("21 days since 30+ min".to_string()),
            threshold: "Weekly long run".to_string(),
            action: "Hit Saturday's long session".to_string(),
            why_it_matters: "Extended gaps reduce aerobic durability".to_string(),
          },
        ],
      }),
    };

    let legacy: WorkoutAnalysis = v4.into();

    // Check summary combines performance + HR assessment
    assert!(legacy.summary.contains("Pace holding steady"));
    assert!(legacy.summary.contains("HR firmly in Z2"));

    // Check tomorrow formats activity, duration, intensity, rationale
    assert!(legacy.tomorrow_recommendation.contains("Ride"));
    assert!(legacy.tomorrow_recommendation.contains("40 min"));
    assert!(legacy.tomorrow_recommendation.contains("Z2"));
    assert!(legacy.tomorrow_recommendation.contains("TSB -12"));

    // Check flags are extracted from eyes_on priorities
    assert_eq!(legacy.risk_flags.len(), 1);
    assert!(legacy.risk_flags[0].contains("long_run_gap"));
    assert!(legacy.risk_flags[0].contains("Hit Saturday's long session"));
  }

  /// ---------------------------------------------------------------------------
  /// Phase 6: V4 JSON Parsing Edge Cases
  /// ---------------------------------------------------------------------------

  #[test]
  fn test_v4_parse_complete_valid_json() {
    // Arrange: Complete valid V4 JSON with all cards
    let json = r#"{
      "performance": {
        "metric_name": "pace",
        "comparison_date": "2025-12-10",
        "comparison_value": "7:15/km",
        "today_value": "7:18/km",
        "delta": "+3 sec/km",
        "insight": "Slightly slower than Tuesday"
      },
      "hr_efficiency": {
        "avg_hr": 142,
        "hr_zone": "Z2",
        "hr_pct_max": 75,
        "hr_assessment": "Controlled Z2 effort",
        "efficiency_trend": "Stable"
      },
      "training_status": {
        "tsb_value": -8.0,
        "tsb_band": "slightly_fatigued",
        "tsb_assessment": "Moderate load",
        "top_flags": ["volume_spike"],
        "adherence_note": "5/6 this week",
        "progression_state": "Building"
      },
      "tomorrow": {
        "activity_type": "Ride",
        "duration_min": 45,
        "duration_label": "STANDARD",
        "intensity": "Z2",
        "goal": "recovery",
        "rationale": "Active recovery",
        "confidence": "high"
      },
      "eyes_on": {
        "priorities": [
          {
            "flag": "long_run_gap",
            "current_value": "14 days",
            "threshold": "Every 7 days",
            "action": "Schedule long run Saturday",
            "why_it_matters": "Maintains endurance"
          }
        ]
      }
    }"#;

    // Act: Parse V4 JSON
    let result: Result<WorkoutAnalysisV4, _> = serde_json::from_str(json);

    // Assert: Should parse successfully
    assert!(result.is_ok(), "Valid V4 JSON should parse");
    let v4 = result.unwrap();
    assert_eq!(v4.performance.metric_name, "pace");
    assert_eq!(v4.hr_efficiency.avg_hr, 142);
    assert_eq!(v4.training_status.tsb_value, -8.0);
    assert_eq!(v4.tomorrow.duration_min, 45);
    assert!(v4.eyes_on.is_some());
    assert_eq!(v4.eyes_on.unwrap().priorities.len(), 1);
  }

  #[test]
  fn test_v4_parse_missing_optional_eyes_on() {
    // Arrange: Valid V4 JSON without optional eyes_on card
    let json = r#"{
      "performance": {
        "metric_name": "power",
        "comparison_date": "2025-12-10",
        "comparison_value": "180W",
        "today_value": "185W",
        "delta": "+5W",
        "insight": "Power trending up"
      },
      "hr_efficiency": {
        "avg_hr": 128,
        "hr_zone": "Z2",
        "hr_pct_max": 67,
        "hr_assessment": "Efficient Z2"
      },
      "training_status": {
        "tsb_value": 2.0,
        "tsb_band": "slightly_fatigued",
        "tsb_assessment": "Recovering well",
        "top_flags": [],
        "adherence_note": "Perfect week",
        "progression_state": "On track"
      },
      "tomorrow": {
        "activity_type": "Run",
        "duration_min": 45,
        "duration_label": "STANDARD",
        "intensity": "Z2",
        "goal": "aerobic_maintenance",
        "rationale": "Continue base building",
        "confidence": "high"
      }
    }"#;

    // Act
    let result: Result<WorkoutAnalysisV4, _> = serde_json::from_str(json);

    // Assert: Should parse successfully with eyes_on = None
    assert!(result.is_ok());
    let v4 = result.unwrap();
    assert!(v4.eyes_on.is_none(), "eyes_on should be None when omitted");

    // Conversion to legacy should handle missing eyes_on
    let legacy: WorkoutAnalysis = v4.into();
    assert_eq!(legacy.risk_flags.len(), 0, "Should have empty flags array");
  }

  #[test]
  fn test_v4_parse_missing_optional_efficiency_trend() {
    // Arrange: HR efficiency card without optional efficiency_trend
    let json = r#"{
      "performance": {
        "metric_name": "pace",
        "comparison_date": "2025-12-10",
        "comparison_value": "7:30/km",
        "today_value": "7:32/km",
        "delta": "+2 sec/km",
        "insight": "Consistent pace"
      },
      "hr_efficiency": {
        "avg_hr": 145,
        "hr_zone": "Z3",
        "hr_pct_max": 76,
        "hr_assessment": "Upper Z2/low Z3"
      },
      "training_status": {
        "tsb_value": -5.0,
        "tsb_band": "slightly_fatigued",
        "tsb_assessment": "Normal training load",
        "top_flags": [],
        "adherence_note": "On track",
        "progression_state": "Building"
      },
      "tomorrow": {
        "activity_type": "Rest",
        "duration_min": 0,
        "duration_label": "REST",
        "intensity": "none",
        "goal": "recovery",
        "rationale": "Scheduled rest day",
        "confidence": "high"
      }
    }"#;

    // Act
    let result: Result<WorkoutAnalysisV4, _> = serde_json::from_str(json);

    // Assert
    assert!(result.is_ok());
    let v4 = result.unwrap();
    assert!(
      v4.hr_efficiency.efficiency_trend.is_none(),
      "efficiency_trend should be None when omitted"
    );
  }

  #[test]
  fn test_v4_parse_empty_arrays() {
    // Arrange: V4 with empty top_flags and priorities arrays
    let json = r#"{
      "performance": {
        "metric_name": "distance",
        "comparison_date": "2025-12-10",
        "comparison_value": "10.0km",
        "today_value": "10.2km",
        "delta": "+0.2km",
        "insight": "Slight increase"
      },
      "hr_efficiency": {
        "avg_hr": 140,
        "hr_zone": "Z2",
        "hr_pct_max": 74,
        "hr_assessment": "Good aerobic effort"
      },
      "training_status": {
        "tsb_value": 0.0,
        "tsb_band": "slightly_fatigued",
        "tsb_assessment": "Balanced",
        "top_flags": [],
        "adherence_note": "5/6 this week",
        "progression_state": "Steady"
      },
      "tomorrow": {
        "activity_type": "Ride",
        "duration_min": 60,
        "duration_label": "STANDARD",
        "intensity": "Z2",
        "goal": "aerobic_base",
        "rationale": "Build endurance",
        "confidence": "high"
      },
      "eyes_on": {
        "priorities": []
      }
    }"#;

    // Act
    let result: Result<WorkoutAnalysisV4, _> = serde_json::from_str(json);

    // Assert: Should handle empty arrays gracefully
    assert!(result.is_ok());
    let v4 = result.unwrap();
    assert_eq!(v4.training_status.top_flags.len(), 0);
    assert!(v4.eyes_on.is_some());
    assert_eq!(v4.eyes_on.unwrap().priorities.len(), 0);
  }

  #[test]
  fn test_v4_parse_nested_flag_priority_structure() {
    // Arrange: Test nested FlagPriority with optional current_value
    let json = r#"{
      "performance": {
        "metric_name": "power",
        "comparison_date": "2025-12-10",
        "comparison_value": "200W",
        "today_value": "205W",
        "delta": "+5W",
        "insight": "Power improving"
      },
      "hr_efficiency": {
        "avg_hr": 130,
        "hr_zone": "Z2",
        "hr_pct_max": 68,
        "hr_assessment": "Low Z2"
      },
      "training_status": {
        "tsb_value": -15.0,
        "tsb_band": "moderate_fatigue",
        "tsb_assessment": "Fatigued",
        "top_flags": ["high_fatigue", "volume_spike"],
        "adherence_note": "Overreaching",
        "progression_state": "Hold"
      },
      "tomorrow": {
        "activity_type": "Rest",
        "duration_min": 0,
        "duration_label": "REST",
        "intensity": "none",
        "goal": "recovery",
        "rationale": "Need recovery",
        "confidence": "high"
      },
      "eyes_on": {
        "priorities": [
          {
            "flag": "high_fatigue",
            "current_value": "TSB -15",
            "threshold": "TSB > -10",
            "action": "Rest day tomorrow",
            "why_it_matters": "Prevent overtraining"
          },
          {
            "flag": "intensity_heavy",
            "threshold": "< 40% Z3+",
            "action": "More Z2 this week",
            "why_it_matters": "Aerobic foundation"
          }
        ]
      }
    }"#;

    // Act
    let result: Result<WorkoutAnalysisV4, _> = serde_json::from_str(json);

    // Assert: Should parse nested structure with optional current_value
    assert!(result.is_ok());
    let v4 = result.unwrap();
    let eyes = v4.eyes_on.unwrap();
    assert_eq!(eyes.priorities.len(), 2);

    // First priority has current_value
    assert_eq!(eyes.priorities[0].flag, "high_fatigue");
    assert!(eyes.priorities[0].current_value.is_some());
    assert_eq!(eyes.priorities[0].current_value.as_ref().unwrap(), "TSB -15");

    // Second priority missing current_value (optional)
    assert_eq!(eyes.priorities[1].flag, "intensity_heavy");
    assert!(eyes.priorities[1].current_value.is_none());
  }

  #[test]
  fn test_v4_parse_negative_numbers() {
    // Arrange: Test parsing of negative TSB values and deltas
    let json = r#"{
      "performance": {
        "metric_name": "pace",
        "comparison_date": "2025-12-10",
        "comparison_value": "7:00/km",
        "today_value": "7:10/km",
        "delta": "-10 sec/km",
        "insight": "Slower pace"
      },
      "hr_efficiency": {
        "avg_hr": 155,
        "hr_zone": "Z3",
        "hr_pct_max": 82,
        "hr_assessment": "Z3 effort"
      },
      "training_status": {
        "tsb_value": -25.5,
        "tsb_band": "high_fatigue",
        "tsb_assessment": "Deeply fatigued",
        "top_flags": ["high_fatigue"],
        "adherence_note": "Overloaded",
        "progression_state": "Regressing"
      },
      "tomorrow": {
        "activity_type": "Rest",
        "duration_min": 0,
        "duration_label": "REST",
        "intensity": "none",
        "goal": "recovery",
        "rationale": "Critical recovery needed",
        "confidence": "very_high"
      }
    }"#;

    // Act
    let result: Result<WorkoutAnalysisV4, _> = serde_json::from_str(json);

    // Assert: Negative numbers should parse correctly
    assert!(result.is_ok());
    let v4 = result.unwrap();
    assert_eq!(v4.training_status.tsb_value, -25.5);
    assert_eq!(v4.tomorrow.duration_min, 0); // Zero is valid
  }

  #[test]
  fn test_v4_parse_with_unicode_and_special_chars() {
    // Arrange: JSON with unicode, quotes, and special characters in strings
    let json = r#"{
      "performance": {
        "metric_name": "distance",
        "comparison_date": "2025-12-10",
        "comparison_value": "10.5km",
        "today_value": "10.7km",
        "delta": "+200m",
        "insight": "Distance up—nice \"push\" today! 💪"
      },
      "hr_efficiency": {
        "avg_hr": 138,
        "hr_zone": "Z2",
        "hr_pct_max": 73,
        "hr_assessment": "HR stayed in Z2 → good control",
        "efficiency_trend": "Improving (3% better)"
      },
      "training_status": {
        "tsb_value": -6.0,
        "tsb_band": "slightly_fatigued",
        "tsb_assessment": "Normal load",
        "top_flags": [],
        "adherence_note": "Perfect week (6/6)",
        "progression_state": "On track"
      },
      "tomorrow": {
        "activity_type": "Rest",
        "duration_min": 0,
        "duration_label": "REST",
        "intensity": "none",
        "goal": "recovery",
        "rationale": "Scheduled rest → let it work",
        "confidence": "high"
      }
    }"#;

    // Act
    let result: Result<WorkoutAnalysisV4, _> = serde_json::from_str(json);

    // Assert: Should handle unicode and special chars
    assert!(result.is_ok());
    let v4 = result.unwrap();
    assert!(v4.performance.insight.contains("push"));
    assert!(v4.hr_efficiency.hr_assessment.contains("→"));
  }

  #[test]
  fn test_extract_json_with_trailing_comma() {
    // Arrange: JSON with trailing comma (common LLM mistake) - should fail gracefully
    let input = r#"{
      "summary": "Good workout",
      "risk_flags": [],
    }"#;

    // Act: extract_json will extract it, but serde will fail
    let extracted = extract_json(input);
    assert!(extracted.is_ok());

    // Parsing should fail due to trailing comma
    let parse_result: Result<WorkoutAnalysis, _> = serde_json::from_str(&extracted.unwrap());
    assert!(
      parse_result.is_err(),
      "Trailing comma should cause parse error"
    );
  }

  #[test]
  fn test_extract_json_multiline_with_comments() {
    // Arrange: JSON embedded in text with // comments (invalid JSON)
    let input = r#"Here's the analysis:

```json
{
  "performance": {  // Performance metrics
    "metric_name": "pace",
    "comparison_date": "2025-12-10",
    "comparison_value": "7:20/km",
    "today_value": "7:22/km",
    "delta": "+2 sec/km",
    "insight": "Steady pace"
  }
}
```"#;

    // Act: extract_json should extract it
    let extracted = extract_json(input);
    assert!(extracted.is_ok());

    // But parsing will fail due to comments
    let json = extracted.unwrap();
    assert!(json.contains("//"), "Comments should be in extracted JSON");

    let parse_result: Result<PerformanceCard, _> = serde_json::from_str(&json);
    assert!(
      parse_result.is_err(),
      "JSON with comments should fail to parse"
    );
  }

  #[test]
  fn test_extract_json_no_json_found() {
    // Arrange: Text with no JSON at all
    let input = "This is just plain text with no JSON structure whatsoever.";

    // Act & Assert: Should return error
    let result = extract_json(input);
    assert!(result.is_err(), "Should fail when no JSON found");
  }

  #[test]
  fn test_extract_json_incomplete_json() {
    // Arrange: Incomplete JSON (missing closing brace)
    let input = r#"{"summary": "test", "risk_flags": ["#;

    // Act: extract_json might extract it
    let extracted = extract_json(input);

    // If extracted, serde should fail
    if let Ok(json) = extracted {
      let parse_result: Result<WorkoutAnalysis, _> = serde_json::from_str(&json);
      assert!(
        parse_result.is_err(),
        "Incomplete JSON should fail to parse"
      );
    }
  }

  #[test]
  fn test_v4_card_validation_missing_required_field() {
    // Arrange: V4 JSON missing required field (metric_name)
    let json = r#"{
      "performance": {
        "comparison_date": "2025-12-10",
        "comparison_value": "7:20/km",
        "today_value": "7:22/km",
        "delta": "+2 sec/km",
        "insight": "Steady"
      },
      "hr_efficiency": {
        "avg_hr": 140,
        "hr_zone": "Z2",
        "hr_pct_max": 74,
        "hr_assessment": "Good"
      },
      "training_status": {
        "tsb_value": -8.0,
        "tsb_band": "slightly_fatigued",
        "tsb_assessment": "Normal",
        "top_flags": [],
        "adherence_note": "Good",
        "progression_state": "Building"
      },
      "tomorrow": {
        "activity_type": "Run",
        "duration_min": 45,
        "duration_label": "STANDARD",
        "intensity": "Z2",
        "goal": "aerobic",
        "rationale": "Base building",
        "confidence": "high"
      }
    }"#;

    // Act & Assert: Should fail due to missing metric_name
    let result: Result<WorkoutAnalysisV4, _> = serde_json::from_str(json);
    assert!(
      result.is_err(),
      "Should fail when required field is missing"
    );
  }

  #[test]
  fn test_v4_parse_wrong_type_for_field() {
    // Arrange: V4 JSON with string instead of number for avg_hr
    let json = r#"{
      "performance": {
        "metric_name": "pace",
        "comparison_date": "2025-12-10",
        "comparison_value": "7:20/km",
        "today_value": "7:22/km",
        "delta": "+2 sec/km",
        "insight": "Steady"
      },
      "hr_efficiency": {
        "avg_hr": "140",
        "hr_zone": "Z2",
        "hr_pct_max": 74,
        "hr_assessment": "Good"
      },
      "training_status": {
        "tsb_value": -8.0,
        "tsb_band": "slightly_fatigued",
        "tsb_assessment": "Normal",
        "top_flags": [],
        "adherence_note": "Good",
        "progression_state": "Building"
      },
      "tomorrow": {
        "activity_type": "Run",
        "duration_min": 45,
        "duration_label": "STANDARD",
        "intensity": "Z2",
        "goal": "aerobic",
        "rationale": "Base",
        "confidence": "high"
      }
    }"#;

    // Act & Assert: Should fail due to type mismatch
    let result: Result<WorkoutAnalysisV4, _> = serde_json::from_str(json);
    assert!(
      result.is_err(),
      "Should fail when field has wrong type (string vs i64)"
    );
  }
}
