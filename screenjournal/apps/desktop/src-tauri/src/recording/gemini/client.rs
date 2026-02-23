/**
 * ============================================================================
 * GEMINI API CLIENT MODULE
 * ============================================================================
 * 
 * PURPOSE: HTTP client for Google Gemini AI video analysis
 * 
 * API ENDPOINT:
 * POST https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent
 * 
 * REQUEST FLOW:
 * 1. Read video file as bytes
 * 2. Base64 encode video data
 * 3. Build request with prompt and inline video
 * 4. Send to Gemini API with thinking budget config
 * 5. Parse JSON timeline response
 * 
 * ============================================================================
 */

use crate::recording::gemini::{
    prompt::build_timeline_prompt,
    types::{GeminiConfig, GeminiError, GeminiTimelineResponse, TimelineAnalysis, TimelineEntry},
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// Gemini API base URL
const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Model to use for video analysis
const GEMINI_MODEL: &str = "gemini-2.5-flash";

/// Request timeout (video upload can take time)
const REQUEST_TIMEOUT_SECS: u64 = 300; // 5 minutes

// =============================================================================
// API Request/Response Structures
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    contents: Vec<Content>,
    generation_config: GenerationConfig,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    thinking_config: ThinkingConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThinkingConfig {
    thinking_budget: u32,
}

#[derive(Debug, Deserialize)]
struct GeminiApiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GeminiApiErrorBody>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: CandidateContent,
}

#[derive(Debug, Deserialize)]
struct CandidateContent {
    parts: Vec<ResponsePart>,
}

#[derive(Debug, Deserialize)]
struct ResponsePart {
    text: Option<String>,
}

/// Internal struct for parsing Gemini API error responses
#[derive(Debug, Deserialize)]
struct GeminiApiErrorBody {
    message: String,
    code: Option<i32>,
}

// =============================================================================
// Public API
// =============================================================================

/**
 * Analyze a video file using Gemini AI
 * 
 * # Arguments
 * * `video_path` - Path to the MP4 video file
 * * `segment_id` - Segment identifier for the result
 * * `display_index` - Display index for multi-monitor setups
 * * `video_duration_seconds` - Duration of the video
 * * `segment_start_time` - ISO 8601 timestamp when segment started
 * * `config` - Gemini configuration
 * 
 * # Returns
 * * `Ok(TimelineAnalysis)` on success
 * * `Err(GeminiError)` with classified error for retry handling
 */
pub async fn analyze_video(
    video_path: &Path,
    segment_id: &str,
    display_index: u32,
    video_duration_seconds: f64,
    segment_start_time: &str,
    config: &GeminiConfig,
    app: Option<&tauri::AppHandle>,
) -> Result<TimelineAnalysis, GeminiError> {
    log::info!(
        "Starting Gemini analysis for segment {} display {} ({:.1}s video)",
        segment_id,
        display_index,
        video_duration_seconds
    );

    // Get API key (check user-provided key first if AppHandle is available)
    let api_key = if let Some(app_handle) = app {
        super::get_api_key_with_app(app_handle).map_err(|e| GeminiError::Permanent {
            message: e,
        })?
    } else {
        super::get_api_key().map_err(|e| GeminiError::Permanent {
            message: e,
        })?
    };

    // Read and encode video file
    let video_data = read_and_encode_video(video_path).map_err(|e| GeminiError::Permanent {
        message: e,
    })?;
    log::info!(
        "Video encoded: {} bytes base64 (from {:?})",
        video_data.len(),
        video_path
    );

    // Build the prompt
    let prompt = build_timeline_prompt(1.0, video_duration_seconds);

    // Build request
    let request = GeminiRequest {
        contents: vec![Content {
            parts: vec![
                Part::Text { text: prompt },
                Part::InlineData {
                    inline_data: InlineData {
                        mime_type: "video/mp4".to_string(),
                        data: video_data,
                    },
                },
            ],
        }],
        generation_config: GenerationConfig {
            thinking_config: ThinkingConfig {
                thinking_budget: config.thinking_budget,
            },
        },
    };

    // Send request to Gemini
    let timeline_entries = send_gemini_request(
        &api_key, 
        request, 
        config.rate_limit_max_wait_seconds
    ).await?;

    // Build analysis result
    let analysis = TimelineAnalysis {
        segment_id: segment_id.to_string(),
        display_index,
        analyzed_at: chrono::Utc::now().to_rfc3339(),
        video_duration_seconds,
        segment_start_time: segment_start_time.to_string(),
        timeline: timeline_entries,
    };

    log::info!(
        "Gemini analysis complete for segment {} display {}: {} timeline entries",
        segment_id,
        display_index,
        analysis.timeline.len()
    );

    Ok(analysis)
}

// =============================================================================
// Internal Functions
// =============================================================================

/**
 * Parse retry delay from Gemini error message
 * Extracts duration from "Please retry in X.XXXs" pattern
 * 
 * # Arguments
 * * `message` - Error message from Gemini API
 * * `max_wait_seconds` - Maximum allowed wait duration
 * 
 * # Returns
 * * `Some(Duration)` if pattern found and parsed
 * * `None` if pattern not found
 */
pub fn parse_retry_delay_from_error(message: &str, max_wait_seconds: u64) -> Option<Duration> {
    // Pattern: "Please retry in X.XXXXXs" or "Please retry in Xs"
    let re = Regex::new(r"Please retry in (\d+\.?\d*)s").ok()?;
    
    let captures = re.captures(message)?;
    let seconds_str = captures.get(1)?.as_str();
    let seconds: f64 = seconds_str.parse().ok()?;
    
    // Add 1 second buffer and cap at maximum
    let wait_seconds = (seconds + 1.0).min(max_wait_seconds as f64);
    
    Some(Duration::from_secs_f64(wait_seconds))
}

/**
 * Read video file and encode as base64
 */
fn read_and_encode_video(video_path: &Path) -> Result<String, String> {
    // Check file exists
    if !video_path.exists() {
        return Err(format!("Video file not found: {:?}", video_path));
    }

    // Read file bytes
    let video_bytes = std::fs::read(video_path)
        .map_err(|e| format!("Failed to read video file {:?}: {}", video_path, e))?;

    // Check file size (Gemini has limits)
    let file_size_mb = video_bytes.len() as f64 / 1_000_000.0;
    if file_size_mb > 100.0 {
        return Err(format!(
            "Video file too large ({:.1} MB). Maximum is 100 MB.",
            file_size_mb
        ));
    }

    log::debug!("Read video file: {:.2} MB", file_size_mb);

    // Base64 encode
    Ok(BASE64.encode(&video_bytes))
}

/**
 * Send request to Gemini API and parse response
 * Returns classified GeminiError for intelligent retry handling
 */
async fn send_gemini_request(
    api_key: &str,
    request: GeminiRequest,
    rate_limit_max_wait_seconds: u64,
) -> Result<Vec<TimelineEntry>, GeminiError> {
    let url = format!(
        "{}/{}:generateContent?key={}",
        GEMINI_API_BASE, GEMINI_MODEL, api_key
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| GeminiError::Permanent {
            message: format!("Failed to create HTTP client: {}", e),
        })?;

    log::info!("[GEMINI] Sending request to Gemini API (model: {})...", GEMINI_MODEL);

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| GeminiError::Permanent {
            message: format!("Gemini API request failed: {}", e),
        })?;

    let status = response.status();
    let status_code = status.as_u16();
    log::info!("[GEMINI] Response status: {}", status);
    
    let response_text = response
        .text()
        .await
        .map_err(|e| GeminiError::Permanent {
            message: format!("Failed to read response: {}", e),
        })?;

    log::debug!("[GEMINI] Raw response length: {} bytes", response_text.len());

    if !status.is_success() {
        // Extract error message from response
        let error_message = if let Ok(error_response) = serde_json::from_str::<GeminiApiResponse>(&response_text) {
            if let Some(error) = error_response.error {
                format!("Gemini API error ({}): {}", error.code.unwrap_or(status_code as i32), error.message)
            } else {
                format!("Gemini API request failed with status {}: {}", status, response_text)
            }
        } else {
            format!("Gemini API request failed with status {}: {}", status, response_text)
        };

        // Classify error based on HTTP status code
        return match status_code {
            429 => {
                // Rate limited - parse retry delay from message
                let retry_after = parse_retry_delay_from_error(&error_message, rate_limit_max_wait_seconds);
                log::warn!(
                    "[GEMINI] Rate limited (429). Retry after: {:?}",
                    retry_after.map(|d| format!("{:.1}s", d.as_secs_f64()))
                );
                Err(GeminiError::RateLimited {
                    message: error_message,
                    retry_after,
                })
            }
            503 => {
                // Service unavailable - model overloaded
                let retry_after = parse_retry_delay_from_error(&error_message, rate_limit_max_wait_seconds);
                log::warn!(
                    "[GEMINI] Service unavailable (503). Retry after: {:?}",
                    retry_after.map(|d| format!("{:.1}s", d.as_secs_f64()))
                );
                Err(GeminiError::ServiceUnavailable {
                    message: error_message,
                    retry_after,
                })
            }
            500 | 502 | 504 => {
                // Server errors - may be temporary
                Err(GeminiError::ServiceUnavailable {
                    message: error_message,
                    retry_after: None,
                })
            }
            _ => {
                // All other errors are permanent (4xx client errors, etc.)
                Err(GeminiError::Permanent {
                    message: error_message,
                })
            }
        };
    }

    // Parse response
    let gemini_response: GeminiApiResponse = serde_json::from_str(&response_text)
        .map_err(|e| GeminiError::Permanent {
            message: format!("Failed to parse Gemini response: {}", e),
        })?;

    // Extract text from response
    let text = gemini_response
        .candidates
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.content.parts.into_iter().next())
        .and_then(|p| p.text)
        .ok_or_else(|| GeminiError::Permanent {
            message: "No text content in Gemini response".to_string(),
        })?;

    // Parse timeline JSON from text
    log::info!("[GEMINI] Parsing timeline from response text ({} chars)", text.len());
    parse_timeline_json(&text).map_err(|e| GeminiError::Permanent { message: e })
}

/**
 * Parse timeline JSON from Gemini response text
 * Handles both clean JSON and JSON with markdown code blocks
 */
fn parse_timeline_json(text: &str) -> Result<Vec<TimelineEntry>, String> {
    // Try direct JSON parse first
    if let Ok(response) = serde_json::from_str::<GeminiTimelineResponse>(text) {
        log_timeline_entries(&response.timeline);
        return Ok(response.timeline);
    }

    // Try to extract JSON from markdown code blocks
    let json_str = extract_json_from_text(text);
    
    serde_json::from_str::<GeminiTimelineResponse>(&json_str)
        .map(|r| {
            log_timeline_entries(&r.timeline);
            r.timeline
        })
        .map_err(|e| {
            log::error!("[GEMINI] Failed to parse timeline JSON: {}", e);
            log::error!("[GEMINI] Raw response text: {}", text);
            format!("Failed to parse timeline JSON: {}. Raw text: {}", e, &text[..text.len().min(500)])
        })
}

/**
 * Log all timeline entries for debugging
 */
fn log_timeline_entries(entries: &[TimelineEntry]) {
    log::info!("[GEMINI] ========== TIMELINE RESULTS ({} entries) ==========", entries.len());
    for (i, entry) in entries.iter().enumerate() {
        log::info!(
            "[GEMINI] [{:02}] {} - {} | App: {} | Score: {} | {}",
            i + 1,
            entry.start_time,
            entry.end_time,
            entry.active_application,
            entry.productive_score,
            entry.description
        );
    }
    log::info!("[GEMINI] ===================================================");
}

/**
 * Extract JSON from text that may contain markdown code blocks
 */
fn extract_json_from_text(text: &str) -> String {
    let text = text.trim();
    
    // Remove markdown code blocks if present
    let text = if text.starts_with("```json") {
        text.strip_prefix("```json")
            .unwrap_or(text)
            .trim_start()
    } else if text.starts_with("```") {
        text.strip_prefix("```")
            .unwrap_or(text)
            .trim_start()
    } else {
        text
    };

    let text = if text.ends_with("```") {
        text.strip_suffix("```").unwrap_or(text).trim_end()
    } else {
        text
    };

    text.to_string()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_text_clean() {
        let input = r#"{"timeline": []}"#;
        let result = extract_json_from_text(input);
        assert_eq!(result, r#"{"timeline": []}"#);
    }

    #[test]
    fn test_extract_json_from_text_markdown() {
        let input = "```json\n{\"timeline\": []}\n```";
        let result = extract_json_from_text(input);
        assert_eq!(result, "{\"timeline\": []}");
    }

    #[test]
    fn test_extract_json_from_text_markdown_no_lang() {
        let input = "```\n{\"timeline\": []}\n```";
        let result = extract_json_from_text(input);
        assert_eq!(result, "{\"timeline\": []}");
    }

    #[test]
    fn test_parse_timeline_json() {
        let json = r#"{"timeline": [{"startTime": "00:00", "endTime": "00:30", "description": "Test", "activeApplication": "Chrome", "activeWindowTitle": "Google", "productiveScore": 3}]}"#;
        let result = parse_timeline_json(json);
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].start_time, "00:00");
        assert_eq!(entries[0].productive_score, 3);
    }

    #[test]
    fn test_parse_retry_delay_with_decimal() {
        let message = "You exceeded your current quota. Please retry in 58.731525401s.";
        let result = parse_retry_delay_from_error(message, 120);
        assert!(result.is_some());
        let duration = result.unwrap();
        // Should be ~59.73s (58.73 + 1s buffer)
        assert!(duration.as_secs_f64() > 59.0);
        assert!(duration.as_secs_f64() < 60.0);
    }

    #[test]
    fn test_parse_retry_delay_integer() {
        let message = "Rate limited. Please retry in 30s.";
        let result = parse_retry_delay_from_error(message, 120);
        assert!(result.is_some());
        let duration = result.unwrap();
        // Should be 31s (30 + 1s buffer)
        assert_eq!(duration.as_secs(), 31);
    }

    #[test]
    fn test_parse_retry_delay_capped_at_max() {
        let message = "Please retry in 300s.";
        let result = parse_retry_delay_from_error(message, 120);
        assert!(result.is_some());
        let duration = result.unwrap();
        // Should be capped at 120s
        assert_eq!(duration.as_secs(), 120);
    }

    #[test]
    fn test_parse_retry_delay_no_match() {
        let message = "Invalid API key";
        let result = parse_retry_delay_from_error(message, 120);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_retry_delay_from_real_error() {
        // Real error message from logs
        let message = "Gemini API error (429): You exceeded your current quota, please check your plan and billing details. For more information on this error, head to: https://ai.google.dev/gemini-api/docs/rate-limits. To monitor your current usage, head to: https://ai.dev/usage?tab=rate-limit. \n* Quota exceeded for metric: generativelanguage.googleapis.com/generate_content_free_tier_requests, limit: 20, model: gemini-2.5-flash\nPlease retry in 58.731525401s.";
        let result = parse_retry_delay_from_error(message, 120);
        assert!(result.is_some());
        let duration = result.unwrap();
        assert!(duration.as_secs_f64() > 59.0);
    }
}

