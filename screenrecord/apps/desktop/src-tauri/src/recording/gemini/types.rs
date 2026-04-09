/**
 * ============================================================================
 * GEMINI TYPES MODULE
 * ============================================================================
 * 
 * PURPOSE: Data structures for Gemini AI timeline analysis
 * 
 * TYPES:
 * - TimelineEntry: Single activity event from video analysis
 * - TimelineAnalysis: Complete analysis result for a segment
 * - GeminiJob: Queue job for processing
 * - GeminiJobStatus: Processing status enum
 * - GeminiConfig: Configuration for Gemini integration
 * 
 * ============================================================================
 */

use crate::recording::types::RecordingMetadata;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

// =============================================================================
// Error Types
// =============================================================================

/**
 * Gemini API error types for intelligent retry handling
 * Allows different retry strategies based on error category
 */
#[derive(Debug, Clone)]
pub enum GeminiError {
    /// Rate limited (429) - use API-provided retry delay
    RateLimited {
        message: String,
        retry_after: Option<Duration>,
    },
    
    /// Service unavailable (503) - temporary overload
    ServiceUnavailable {
        message: String,
        retry_after: Option<Duration>,
    },
    
    /// Permanent error - do not retry
    Permanent {
        message: String,
    },
}

impl std::fmt::Display for GeminiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeminiError::RateLimited { message, retry_after } => {
                if let Some(duration) = retry_after {
                    write!(f, "Rate limited: {} (retry after {:.1}s)", message, duration.as_secs_f64())
                } else {
                    write!(f, "Rate limited: {}", message)
                }
            }
            GeminiError::ServiceUnavailable { message, retry_after } => {
                if let Some(duration) = retry_after {
                    write!(f, "Service unavailable: {} (retry after {:.1}s)", message, duration.as_secs_f64())
                } else {
                    write!(f, "Service unavailable: {}", message)
                }
            }
            GeminiError::Permanent { message } => {
                write!(f, "Permanent error: {}", message)
            }
        }
    }
}

impl std::error::Error for GeminiError {}

// =============================================================================
// Timeline Data Structures
// =============================================================================

/**
 * Single timeline entry from Gemini analysis
 * Represents one distinct activity or context switch in the video
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    /// Start time in "MM:SS" format
    pub start_time: String,
    
    /// End time in "MM:SS" format
    pub end_time: String,
    
    /// Short description of user activity
    pub description: String,
    
    /// Application being used (e.g., "VS Code", "Chrome")
    pub active_application: String,
    
    /// Window title or tab name if visible
    pub active_window_title: String,
    
    /// Productivity score 1-5
    /// 5: Highly Productive, 4: Productive, 3: Neutral, 2: Low, 1: Distraction
    pub productive_score: u8,
}

/**
 * Raw Gemini API response structure
 * Used for deserializing the JSON response
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiTimelineResponse {
    pub timeline: Vec<TimelineEntry>,
}

/**
 * Complete analysis result for a recording segment
 * Contains all timeline entries plus metadata
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineAnalysis {
    /// Segment ID from recording metadata
    pub segment_id: String,
    
    /// Display index (0, 1, 2, etc.)
    pub display_index: u32,
    
    /// ISO 8601 timestamp when analysis completed
    pub analyzed_at: String,
    
    /// Video duration in seconds
    pub video_duration_seconds: f64,
    
    /// Segment start time (ISO 8601) for timestamp calculation
    pub segment_start_time: String,
    
    /// All timeline entries from analysis
    pub timeline: Vec<TimelineEntry>,
}

// =============================================================================
// Job Processing Structures
// =============================================================================

/**
 * Queue job for Gemini processing
 * Contains all information needed to process a video segment
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiJob {
    /// Unique segment identifier
    pub segment_id: String,
    
    /// Display index for multi-monitor setups
    pub display_index: u32,
    
    /// Path to the MP4 video file
    pub video_path: PathBuf,
    
    /// Recording metadata for context
    pub metadata: RecordingMetadata,
    
    /// Number of retry attempts (for non-rate-limit errors)
    pub retry_count: u32,
    
    /// Number of rate-limit waits (separate from retry_count)
    /// Rate limits don't count against max_retries
    #[serde(default)]
    pub rate_limit_waits: u32,
    
    /// When the job was created
    pub created_at: DateTime<Utc>,
}

/**
 * Status of a Gemini processing job
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GeminiJobStatus {
    /// Job is waiting in queue
    Pending,
    
    /// Video is being uploaded to Gemini
    Uploading,
    
    /// Waiting for Gemini analysis response
    Analyzing,
    
    /// Analysis completed successfully
    Completed(TimelineAnalysis),
    
    /// Job failed with error message
    Failed(String),
}

// =============================================================================
// Configuration
// =============================================================================

/**
 * Configuration for Gemini AI integration
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeminiConfig {
    /// Whether Gemini analysis is enabled
    pub enabled: bool,
    
    /// Rate limit (requests per minute), 0 = no limit
    #[serde(default)]
    pub rate_limit_per_minute: u32,
    
    /// Maximum retry attempts for failed jobs (non-rate-limit errors)
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Base delay between retries (seconds), increases exponentially
    #[serde(default = "default_retry_delay")]
    pub retry_delay_seconds: u64,
    
    /// Thinking budget for Gemini 2.5 Flash Lite
    #[serde(default = "default_thinking_budget")]
    pub thinking_budget: u32,
    
    /// Maximum number of rate-limit waits before failing job
    #[serde(default = "default_rate_limit_max_waits")]
    pub rate_limit_max_waits: u32,
    
    /// Maximum wait duration for a single rate limit (seconds)
    #[serde(default = "default_rate_limit_max_wait_seconds")]
    pub rate_limit_max_wait_seconds: u64,
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    5
}

fn default_thinking_budget() -> u32 {
    1024
}

fn default_rate_limit_max_waits() -> u32 {
    5
}

fn default_rate_limit_max_wait_seconds() -> u64 {
    120
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rate_limit_per_minute: 0, // No rate limiting initially
            max_retries: default_max_retries(),
            retry_delay_seconds: default_retry_delay(),
            thinking_budget: default_thinking_budget(),
            rate_limit_max_waits: default_rate_limit_max_waits(),
            rate_limit_max_wait_seconds: default_rate_limit_max_wait_seconds(),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeline_entry_deserialization() {
        let json = r#"{
            "startTime": "00:00",
            "endTime": "00:30",
            "description": "User editing Rust file",
            "activeApplication": "VS Code",
            "activeWindowTitle": "types.rs - screenrecord",
            "productiveScore": 5
        }"#;
        
        let entry: TimelineEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.start_time, "00:00");
        assert_eq!(entry.end_time, "00:30");
        assert_eq!(entry.productive_score, 5);
    }

    #[test]
    fn test_gemini_response_deserialization() {
        let json = r#"{
            "timeline": [
                {
                    "startTime": "00:00",
                    "endTime": "00:30",
                    "description": "Test",
                    "activeApplication": "Chrome",
                    "activeWindowTitle": "Google",
                    "productiveScore": 3
                }
            ]
        }"#;
        
        let response: GeminiTimelineResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.timeline.len(), 1);
    }

    #[test]
    fn test_gemini_config_default() {
        let config = GeminiConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.rate_limit_per_minute, 0);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_seconds, 5);
        assert_eq!(config.thinking_budget, 1024);
        assert_eq!(config.rate_limit_max_waits, 5);
        assert_eq!(config.rate_limit_max_wait_seconds, 120);
    }
    
    #[test]
    fn test_gemini_error_display() {
        let rate_limited = GeminiError::RateLimited {
            message: "Quota exceeded".to_string(),
            retry_after: Some(Duration::from_secs_f64(58.5)),
        };
        assert!(rate_limited.to_string().contains("Rate limited"));
        assert!(rate_limited.to_string().contains("58.5s"));
        
        let unavailable = GeminiError::ServiceUnavailable {
            message: "Model overloaded".to_string(),
            retry_after: None,
        };
        assert!(unavailable.to_string().contains("Service unavailable"));
        
        let permanent = GeminiError::Permanent {
            message: "Invalid API key".to_string(),
        };
        assert!(permanent.to_string().contains("Permanent error"));
    }
}

