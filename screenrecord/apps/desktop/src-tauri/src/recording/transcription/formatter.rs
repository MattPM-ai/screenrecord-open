/**
 * ============================================================================
 * TRANSCRIPTION FORMATTER MODULE
 * ============================================================================
 * 
 * PURPOSE: Convert transcription segments to InfluxDB line protocol
 * 
 * MEASUREMENT: audio_transcript
 * 
 * LINE PROTOCOL FORMAT:
 * audio_transcript,speaker=employee,hostname=laptop1 text="Hello...",duration_ms=2500i,audio_path="/Users/..." 1234567890000000000
 * 
 * TAGS:
 * - speaker: Speaker label ("employee" or "customer")
 * - hostname: System hostname
 * 
 * FIELDS:
 * - text: Segment text (string)
 * - duration_ms: Segment duration in milliseconds (integer)
 * - audio_path: Absolute local path to mixed audio file (string, optional)
 * 
 * TIMESTAMP: Absolute nanoseconds (segment start time + segment offset)
 * 
 * ============================================================================
 */

use crate::collector::formatter::{escape_field_string, escape_tag_value};
use crate::recording::transcription::types::TranscriptionSegment;
use chrono::{DateTime, Duration, Utc};

/// Measurement name for audio transcription events
const MEASUREMENT: &str = "audio_transcript";

/**
 * Format a single transcription segment to line protocol
 * 
 * # Arguments
 * * `segment` - The transcription segment to format
 * * `segment_start_time` - ISO 8601 timestamp when the recording segment started
 * * `speaker_label` - Speaker label ("employee" or "customer")
 * * `hostname` - System hostname for tagging
 * * `audio_path` - Optional absolute local path to mixed audio file
 * 
 * # Returns
 * * `Ok(String)` - Line protocol string
 * * `Err(String)` - Error message
 */
pub fn format_transcription_segment(
    segment: &TranscriptionSegment,
    segment_start_time: &str,
    speaker_label: &str,
    hostname: &str,
    audio_path: Option<&str>,
) -> Result<String, String> {
    // Calculate absolute timestamp from segment start + segment offset
    let timestamp_nanos = calculate_segment_timestamp(segment_start_time, segment.start_ms)?;

    // Calculate duration from start_ms and end_ms
    let duration_ms = segment.end_ms.saturating_sub(segment.start_ms);

    // Format tags
    let speaker_tag = escape_tag_value(speaker_label);
    let hostname_tag = escape_tag_value(hostname);

    // Format fields
    let text_field = escape_field_string(&segment.text);
    
    // Build audio_path field if present (local absolute path, not S3 URL)
    let audio_path_field = match audio_path {
        Some(path) => format!(",audio_path={}", escape_field_string(path)),
        None => String::new(),
    };

    Ok(format!(
        "{},speaker={},hostname={} text={},duration_ms={}i{} {}",
        MEASUREMENT,
        speaker_tag,
        hostname_tag,
        text_field,
        duration_ms,
        audio_path_field,
        timestamp_nanos
    ))
}

/**
 * Calculate absolute timestamp for a transcription segment
 * 
 * Combines segment start time (ISO 8601) with segment offset (milliseconds)
 * 
 * # Arguments
 * * `segment_start_time` - ISO 8601 timestamp when the recording segment started
 * * `offset_ms` - Millisecond offset from audio beginning
 * 
 * # Returns
 * * `Ok(i64)` - Timestamp in nanoseconds
 * * `Err(String)` - Error message if parsing fails
 */
fn calculate_segment_timestamp(segment_start_time: &str, offset_ms: u64) -> Result<i64, String> {
    // Parse segment start time
    let segment_start = DateTime::parse_from_rfc3339(segment_start_time)
        .map_err(|e| format!("Invalid segment start time '{}': {}", segment_start_time, e))?
        .with_timezone(&Utc);

    // Add offset
    let segment_timestamp = segment_start + Duration::milliseconds(offset_ms as i64);

    // Convert to nanoseconds
    segment_timestamp
        .timestamp_nanos_opt()
        .ok_or_else(|| "Timestamp out of range for nanoseconds".to_string())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_segment_timestamp_no_offset() {
        let segment_start = "2025-01-27T10:00:00Z";
        
        // Zero offset should match segment start
        let ts = calculate_segment_timestamp(segment_start, 0).unwrap();
        let expected = crate::collector::formatter::timestamp_to_nanos(segment_start).unwrap();
        assert_eq!(ts, expected);
    }

    #[test]
    fn test_calculate_segment_timestamp_with_offset() {
        let segment_start = "2025-01-27T10:00:00Z";
        
        // 2500ms offset
        let ts = calculate_segment_timestamp(segment_start, 2500).unwrap();
        let expected = crate::collector::formatter::timestamp_to_nanos("2025-01-27T10:00:02.500Z").unwrap();
        assert_eq!(ts, expected);
    }

    #[test]
    fn test_calculate_segment_timestamp_invalid() {
        assert!(calculate_segment_timestamp("invalid", 0).is_err());
    }

    #[test]
    fn test_format_transcription_segment() {
        let segment = TranscriptionSegment {
            start_ms: 0,
            end_ms: 2500,
            text: "Hello, how can I help you today?".to_string(),
            words: vec![],
        };

        let result = format_transcription_segment(
            &segment,
            "2025-01-27T10:00:00Z",
            "employee",
            "laptop1",
            None,
        );

        assert!(result.is_ok());
        let line = result.unwrap();
        
        assert!(line.starts_with("audio_transcript,"));
        assert!(line.contains("speaker=employee"));
        assert!(line.contains("hostname=laptop1"));
        assert!(line.contains("text=\"Hello, how can I help you today?\""));
        assert!(line.contains("duration_ms=2500i"));
    }

    #[test]
    fn test_format_transcription_segment_with_audio_path() {
        let segment = TranscriptionSegment {
            start_ms: 0,
            end_ms: 2500,
            text: "Hello".to_string(),
            words: vec![],
        };

        let audio_path = "/Users/username/.screenrecord/audio/2025-01-15/segment_123.mp4";
        let result = format_transcription_segment(
            &segment,
            "2025-01-27T10:00:00Z",
            "employee",
            "laptop1",
            Some(audio_path),
        );

        assert!(result.is_ok());
        let line = result.unwrap();
        
        assert!(line.contains("audio_path=\"/Users/username/.screenrecord/audio/2025-01-15/segment_123.mp4\""));
    }

    #[test]
    fn test_format_transcription_segment_customer() {
        let segment = TranscriptionSegment {
            start_ms: 5000,
            end_ms: 8000,
            text: "I need help with my account.".to_string(),
            words: vec![],
        };

        let result = format_transcription_segment(
            &segment,
            "2025-01-27T10:00:00Z",
            "customer",
            "laptop1",
            None,
        );

        assert!(result.is_ok());
        let line = result.unwrap();
        
        assert!(line.contains("speaker=customer"));
        assert!(line.contains("duration_ms=3000i"));
    }

    #[test]
    fn test_format_transcription_segment_special_chars() {
        let segment = TranscriptionSegment {
            start_ms: 0,
            end_ms: 1000,
            text: "Text with \"quotes\" and\nnewlines".to_string(),
            words: vec![],
        };

        let result = format_transcription_segment(
            &segment,
            "2025-01-27T10:00:00Z",
            "employee",
            "host,name",
            None,
        );

        assert!(result.is_ok());
        let line = result.unwrap();
        
        // Verify special characters are escaped
        assert!(line.contains("hostname=host\\,name"));
        assert!(line.contains("\\\"quotes\\\""));
        assert!(line.contains("\\n"));
    }
}
