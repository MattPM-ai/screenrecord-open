/**
 * ============================================================================
 * TRANSCRIPTION TYPES MODULE
 * ============================================================================
 * 
 * PURPOSE: Data structures for local audio transcription system
 * 
 * TYPES:
 * - AudioSource: Enum distinguishing microphone vs system audio
 * - TranscribedWord: Single word with timestamp and confidence
 * - TranscriptionResult: Complete transcript for one audio source
 * - TranscriptionJob: Queue job for processing
 * - TranscriptionConfig: Configuration for transcription system
 * 
 * ============================================================================
 */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// =============================================================================
// Audio Source
// =============================================================================

/**
 * Distinguishes between microphone (employee) and system audio (customer/client)
 * 
 * This enables natural diarization without ML-based speaker identification:
 * - Microphone captures the employee/user's voice
 * - System audio captures remote participants (calls, meetings, etc.)
 */
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioSource {
    /// Microphone input - captures employee/user speech
    Microphone,
    /// System audio - captures customer/client speech from calls, meetings, etc.
    SystemAudio,
}

impl AudioSource {
    /// Get human-readable speaker label for this audio source
    /// 
    /// # Returns
    /// - "employee" for Microphone
    /// - "customer" for SystemAudio
    pub fn speaker_label(&self) -> &'static str {
        match self {
            AudioSource::Microphone => "employee",
            AudioSource::SystemAudio => "customer",
        }
    }
    
    /// Get file suffix for this audio source
    /// 
    /// Used in file naming: {segment_id}_d{display}.{suffix}.wav
    pub fn file_suffix(&self) -> &'static str {
        match self {
            AudioSource::Microphone => "mic",
            AudioSource::SystemAudio => "audio",
        }
    }
}

impl std::fmt::Display for AudioSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioSource::Microphone => write!(f, "microphone"),
            AudioSource::SystemAudio => write!(f, "system_audio"),
        }
    }
}

// =============================================================================
// Transcription Result Types
// =============================================================================

/**
 * A single transcribed word with timing and confidence information
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribedWord {
    /// The transcribed word text
    pub word: String,
    
    /// Start time offset from audio beginning (milliseconds)
    pub start_ms: u64,
    
    /// End time offset from audio beginning (milliseconds)
    pub end_ms: u64,
    
    /// Confidence/probability score from Whisper (0.0 to 1.0)
    pub probability: f32,
}

/**
 * A transcription segment (sentence/phrase level grouping)
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    /// Segment start time offset (milliseconds)
    pub start_ms: u64,
    
    /// Segment end time offset (milliseconds)
    pub end_ms: u64,
    
    /// Full text of the segment
    pub text: String,
    
    /// Words in this segment with individual timestamps
    pub words: Vec<TranscribedWord>,
}

/**
 * Complete transcription result for one audio source
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Recording segment ID this transcript belongs to
    pub segment_id: String,
    
    /// Display index (for multi-monitor setups)
    pub display_index: u32,
    
    /// Audio source (Microphone or SystemAudio)
    pub source: AudioSource,
    
    /// Human-readable speaker label ("employee" or "customer")
    pub speaker_label: String,
    
    /// Whisper model used (e.g., "tiny.en")
    pub model: String,
    
    /// Detected/configured language (e.g., "en")
    pub language: String,
    
    /// ISO 8601 timestamp when transcription completed
    pub transcribed_at: String,
    
    /// Duration of the source audio in milliseconds
    pub audio_duration_ms: u64,
    
    /// Time taken to process transcription in milliseconds
    pub processing_time_ms: u64,
    
    /// Transcription segments (sentence/phrase groupings)
    pub segments: Vec<TranscriptionSegment>,
    
    /// Complete transcript text (concatenated from segments)
    pub full_text: String,
}

// =============================================================================
// Job Processing Types
// =============================================================================

/**
 * Queue job for transcription processing
 * 
 * Contains all information needed to process an audio file.
 * Jobs are processed sequentially to manage CPU load.
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionJob {
    /// Recording segment ID
    pub segment_id: String,
    
    /// Display index for multi-monitor setups
    pub display_index: u32,
    
    /// Audio source (Microphone or SystemAudio)
    pub source: AudioSource,
    
    /// Path to the WAV audio file
    pub audio_path: PathBuf,
    
    /// Segment start time (ISO 8601) for absolute timestamp calculation
    pub segment_start_time: String,
    
    /// Number of retry attempts for this job
    pub retry_count: u32,
    
    /// When the job was created
    pub created_at: DateTime<Utc>,
    
    /// Optional absolute path to mixed audio file (local storage, not S3)
    /// Both mic and system audio jobs share the same path (mixed file)
    pub audio_path_local: Option<PathBuf>,
}

// =============================================================================
// Configuration
// =============================================================================

/**
 * Configuration for the transcription system
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionConfig {
    /// Whether transcription is enabled
    pub enabled: bool,
    
    /// Whisper model to use (e.g., "tiny.en", "base.en")
    /// Currently only "tiny.en" is bundled
    #[serde(default = "default_model")]
    pub model: String,
    
    /// Maximum retry attempts for failed transcription jobs
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Base delay between retries in seconds (exponential backoff)
    #[serde(default = "default_retry_delay")]
    pub retry_delay_seconds: u64,
    
    /// Delay before starting transcription after segment finishes (seconds)
    /// Gives system time to settle after recording
    #[serde(default = "default_processing_delay")]
    pub processing_delay_seconds: u64,
}

fn default_model() -> String {
    "tiny.en".to_string()
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    5
}

fn default_processing_delay() -> u64 {
    5
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Enabled by default
            model: default_model(),
            max_retries: default_max_retries(),
            retry_delay_seconds: default_retry_delay(),
            processing_delay_seconds: default_processing_delay(),
        }
    }
}

// =============================================================================
// Queue Statistics
// =============================================================================

/**
 * Statistics for the transcription queue
 */
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueStats {
    /// Total jobs submitted since queue started
    pub jobs_submitted: u64,
    
    /// Jobs completed successfully
    pub jobs_completed: u64,
    
    /// Jobs that failed after all retries
    pub jobs_failed: u64,
    
    /// Jobs currently waiting in queue
    pub jobs_pending: u64,
    
    /// Last error message (if any)
    pub last_error: Option<String>,
    
    /// Total audio duration processed (milliseconds)
    pub total_audio_processed_ms: u64,
    
    /// Total processing time (milliseconds)
    pub total_processing_time_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_source_speaker_label() {
        assert_eq!(AudioSource::Microphone.speaker_label(), "employee");
        assert_eq!(AudioSource::SystemAudio.speaker_label(), "customer");
    }

    #[test]
    fn test_audio_source_file_suffix() {
        assert_eq!(AudioSource::Microphone.file_suffix(), "mic");
        assert_eq!(AudioSource::SystemAudio.file_suffix(), "audio");
    }

    #[test]
    fn test_transcription_config_default() {
        let config = TranscriptionConfig::default();
        assert!(config.enabled); // Enabled by default
        assert_eq!(config.model, "tiny.en");
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_seconds, 5);
        assert_eq!(config.processing_delay_seconds, 5);
    }
}
