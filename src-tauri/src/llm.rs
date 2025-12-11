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
}
