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
/// Workout Analysis Response (from Claude)
/// ---------------------------------------------------------------------------

/// Enhanced workout analysis with deep dive content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutAnalysisV2 {
  /// Deep workout analysis
  pub workout_analysis: WorkoutBreakdown,

  /// Progression status (echoes what Rust computed, with explanations)
  pub progression: Option<ProgressionResponse>,

  /// Plan status
  pub plan_status: Option<PlanStatusResponse>,

  /// What to do tomorrow
  pub tomorrow: String,

  /// Risk flags
  pub risk_flags: Vec<String>,

  /// Goal-specific notes
  pub goal_notes: Option<String>,
}

/// Deep workout breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutBreakdown {
  pub summary: String,
  pub execution: String,
  pub hr_insights: String,
  pub comparison: Option<String>,
}

/// Progression status from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressionResponse {
  pub run_interval_status: String,
  pub run_interval_note: String,
  pub long_run_status: Option<String>,
  pub long_run_note: Option<String>,
}

/// Plan status from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStatusResponse {
  pub week_on_track: bool,
  pub adjustment_needed: Option<String>,
}

/// Legacy format (backward compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutAnalysis {
  /// Brief summary of the workout
  pub summary: String,

  /// What to do tomorrow based on current state
  pub tomorrow_recommendation: String,

  /// Risk flags or concerns identified
  pub risk_flags: Vec<String>,

  /// Notes specific to Kilimanjaro or marathon training goals
  pub goal_notes: Option<String>,
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

  /// Analyze a workout with structured JSON output (legacy format, for backward compatibility)
  pub async fn analyze_workout(
    &self,
    context_json: &str,
  ) -> Result<(WorkoutAnalysis, Usage), LlmError> {
    // Try enhanced format first, fall back to legacy if needed
    match self.analyze_workout_enhanced(context_json).await {
      Ok((v2, usage)) => Ok((v2.into(), usage)),
      Err(_) => self.analyze_workout_legacy(context_json).await,
    }
  }

  /// Analyze a workout with the enhanced V2 format (deep analysis)
  pub async fn analyze_workout_enhanced(
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
  "goal_notes": "Optional notes about Kilimanjaro/marathon progress, or null if nothing relevant"
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
