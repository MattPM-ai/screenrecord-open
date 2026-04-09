/**
 * ============================================================================
 * RECORDING TYPES MODULE
 * ============================================================================
 * 
 * PURPOSE: Data structures for multi-display MP4 screen recording system
 * 
 * TYPES:
 * - RecordingConfig: Capture configuration
 * - RecordingMetadata: Metadata for captured segments (JSON sidecar)
 * - DisplayRecording: Per-display recording information
 * - MonitorInfo: Display information
 * - RecordingStatus: Status for frontend display
 * 
 * Gemini-related types are in recording/gemini/types.rs
 * 
 * ============================================================================
 */

use serde::{Deserialize, Serialize};

// Re-export GeminiConfig for convenience
pub use crate::recording::gemini::types::GeminiConfig;

// =============================================================================
// Audio Feature Configuration
// =============================================================================

/**
 * Audio Feature Configuration
 * 
 * Controls audio recording (microphone + system audio) and Whisper transcription.
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioFeatureConfig {
    /// Enable audio recording (mic + system audio)
    pub enabled: bool,
    
    /// Enable Whisper transcription (separate from recording)
    #[serde(default = "default_transcription_enabled")]
    pub transcription_enabled: bool,
    
    /// Whisper model to use (e.g., "tiny.en", "base.en")
    #[serde(default = "default_transcription_model")]
    pub transcription_model: String,
    
    /// Maximum retry attempts for failed transcription jobs
    #[serde(default = "default_transcription_max_retries")]
    pub transcription_max_retries: u32,
    
    /// Base delay between transcription retries (seconds)
    #[serde(default = "default_transcription_retry_delay")]
    pub transcription_retry_delay_seconds: u64,
    
    /// Delay before starting transcription after segment finishes (seconds)
    #[serde(default = "default_transcription_processing_delay")]
    pub transcription_processing_delay_seconds: u64,
}

fn default_transcription_enabled() -> bool { true }
fn default_transcription_model() -> String { "tiny.en".to_string() }
fn default_transcription_max_retries() -> u32 { 3 }
fn default_transcription_retry_delay() -> u64 { 5 }
fn default_transcription_processing_delay() -> u64 { 5 }

impl Default for AudioFeatureConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Audio recording enabled by default
            transcription_enabled: default_transcription_enabled(),
            transcription_model: default_transcription_model(),
            transcription_max_retries: default_transcription_max_retries(),
            transcription_retry_delay_seconds: default_transcription_retry_delay(),
            transcription_processing_delay_seconds: default_transcription_processing_delay(),
        }
    }
}

impl AudioFeatureConfig {
    /// Check if config changes require recording system restart
    pub fn needs_recording_restart(&self, other: &AudioFeatureConfig) -> bool {
        self.enabled != other.enabled
    }
}

// =============================================================================
// Screen Recording Configuration
// =============================================================================

// Configuration for screen recording capture
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordingConfig {
    // Recording enabled/disabled
    pub enabled: bool,
    
    // Duration of each recording segment (seconds)
    pub segment_duration_seconds: u64,
    
    // Target framerate for recording (fps)
    pub framerate: u8,
    
    // Retention time (days)
    pub retention_days: u32,
    
    // Maximum storage size (bytes)
    pub max_storage_bytes: u64,
    
    // Output width for recordings (height calculated to maintain aspect ratio)
    // Default: 1280 (720p) - optimized for Gemini AI analysis
    #[serde(default = "default_output_width")]
    pub output_width: u32,
    
    // CRF quality (0-51, lower = better quality, higher = smaller files)
    // Default: 30 - good balance for screen content AI analysis
    #[serde(default = "default_crf")]
    pub crf: u8,
    
    // FFmpeg preset (ultrafast, superfast, veryfast, faster, fast, medium, slow)
    // Default: "fast" - good compression with reasonable CPU usage
    #[serde(default = "default_preset")]
    pub preset: String,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            segment_duration_seconds: 300, // 5 minutes - optimized for Gemini chunk size
            framerate: 4,
            retention_days: 3,
            max_storage_bytes: 5_000_000_000, // 5GB
            output_width: default_output_width(),
            crf: default_crf(),
            preset: default_preset(),
        }
    }
}

fn default_output_width() -> u32 {
    1280 // 720p width
}

fn default_crf() -> u8 {
    30 // Good balance for AI analysis
}

fn default_preset() -> String {
    "fast".to_string()
}

impl RecordingConfig {
    // Check if config changes require recording system restart
    pub fn needs_recording_restart(&self, other: &RecordingConfig) -> bool {
        self.enabled != other.enabled
            || self.segment_duration_seconds != other.segment_duration_seconds
            || self.framerate != other.framerate
            || self.output_width != other.output_width
            || self.crf != other.crf
            || self.preset != other.preset
    }
}

// Information about a monitor/display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub scale_factor: f32,
    pub is_primary: bool,
}

// Per-display recording information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayRecording {
    // Display index
    pub display_index: u32,
    
    // Frame width in pixels
    pub width: u32,
    
    // Frame height in pixels
    pub height: u32,
    
    // Number of frames captured
    pub frame_count: u64,
    
    // File size in bytes
    pub file_size_bytes: u64,
    
    // Filename (relative to segment directory)
    pub filename: String,
}

// Metadata for a captured recording segment (stored as JSON sidecar)
// A segment may contain recordings from multiple displays
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    // Unique segment ID (timestamp + random suffix)
    pub id: String,
    
    // Video format (e.g., "mp4")
    #[serde(default = "default_format")]
    pub format: String,
    
    // Video codec (e.g., "h264")
    #[serde(default = "default_codec")]
    pub codec: String,
    
    // Recording framerate
    pub framerate: u8,
    
    // ISO 8601 timestamp of recording start
    pub start_time: String,
    
    // ISO 8601 timestamp of recording end
    pub end_time: String,
    
    // Duration in seconds
    pub duration_seconds: f64,
    
    // Total file size across all displays (bytes)
    pub total_file_size_bytes: u64,
    
    // Number of displays recorded
    pub display_count: u32,
    
    // Per-display recording information
    pub displays: Vec<DisplayRecording>,
}

fn default_format() -> String {
    "mp4".to_string()
}

fn default_codec() -> String {
    "h264".to_string()
}

// Recording status for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStatus {
    pub enabled: bool,
    pub is_recording: bool,
    pub current_segment_id: Option<String>,
    pub current_segment_start: Option<String>,
    pub current_segment_duration_seconds: Option<f64>,
    pub display_count: u32,
    pub total_segments: u64,
    pub total_storage_bytes: u64,
}

// Response for recordings query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingsResponse {
    pub recordings: Vec<RecordingMetadata>,
    pub total_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_config_default() {
        let config = RecordingConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.segment_duration_seconds, 300);
        assert_eq!(config.framerate, 4);
        assert_eq!(config.retention_days, 3);
        assert_eq!(config.output_width, 1280);
        assert_eq!(config.crf, 30);
        assert_eq!(config.preset, "fast");
    }

    #[test]
    fn test_config_needs_restart() {
        let config1 = RecordingConfig::default();
        let mut config2 = config1.clone();
        
        // Changing retention_days should NOT require restart
        config2.retention_days = 7;
        assert!(!config1.needs_recording_restart(&config2));
        
        // Changing framerate SHOULD require restart
        config2.framerate = 10;
        assert!(config1.needs_recording_restart(&config2));
        
        // Reset and test new fields
        config2 = config1.clone();
        
        // Changing output_width SHOULD require restart
        config2.output_width = 1920;
        assert!(config1.needs_recording_restart(&config2));
        
        // Reset and test crf
        config2 = config1.clone();
        config2.crf = 25;
        assert!(config1.needs_recording_restart(&config2));
        
        // Reset and test preset
        config2 = config1.clone();
        config2.preset = "medium".to_string();
        assert!(config1.needs_recording_restart(&config2));
    }
}
