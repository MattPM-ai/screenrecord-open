/**
 * ============================================================================
 * GEMINI FORMATTER MODULE
 * ============================================================================
 * 
 * PURPOSE: Convert Gemini timeline analysis to InfluxDB line protocol
 * 
 * MEASUREMENT: screen_timeline
 * 
 * LINE PROTOCOL FORMAT:
 * screen_timeline,display=0,app=VSCode,hostname=laptop1 description="...",productive_score=5i,app_title="file.rs",duration_seconds=30i 1234567890000000000
 * 
 * ============================================================================
 */

use crate::collector::formatter::{escape_field_string, escape_tag_value};
use crate::recording::gemini::types::{TimelineAnalysis, TimelineEntry};
use chrono::{DateTime, Duration, Utc};

/// Measurement name for screen timeline events
const MEASUREMENT: &str = "screen_timeline";

/**
 * Format a single timeline entry to line protocol
 * 
 * # Arguments
 * * `analysis` - The complete analysis for context (segment info, timestamps)
 * * `entry` - The timeline entry to format
 * * `hostname` - System hostname for tagging
 * 
 * # Returns
 * * `Ok(String)` - Line protocol string
 * * `Err(String)` - Error message
 */
pub fn format_timeline_entry(
    analysis: &TimelineAnalysis,
    entry: &TimelineEntry,
    hostname: &str,
) -> Result<String, String> {
    // Calculate absolute timestamp from segment start + entry start time
    let timestamp_nanos = calculate_entry_timestamp(
        &analysis.segment_start_time,
        &entry.start_time,
    )?;

    // Calculate duration from start_time and end_time
    let duration_seconds = calculate_duration_seconds(&entry.start_time, &entry.end_time)?;

    // Format tags
    let display_tag = analysis.display_index.to_string();
    let app_tag = escape_tag_value(&entry.active_application);
    let hostname_tag = escape_tag_value(hostname);

    // Format fields
    let description_field = escape_field_string(&entry.description);
    let app_title_field = escape_field_string(&entry.active_window_title);

    Ok(format!(
        "{},display={},app={},hostname={} description={},productive_score={}i,app_title={},duration_seconds={}i {}",
        MEASUREMENT,
        display_tag,
        app_tag,
        hostname_tag,
        description_field,
        entry.productive_score,
        app_title_field,
        duration_seconds,
        timestamp_nanos
    ))
}

/**
 * Calculate absolute timestamp for a timeline entry
 * 
 * Combines segment start time (ISO 8601) with entry start time (MM:SS)
 */
fn calculate_entry_timestamp(
    segment_start_time: &str,
    entry_start_time: &str,
) -> Result<i64, String> {
    // Parse segment start time
    let segment_start = DateTime::parse_from_rfc3339(segment_start_time)
        .map_err(|e| format!("Invalid segment start time '{}': {}", segment_start_time, e))?
        .with_timezone(&Utc);

    // Parse entry start time (MM:SS)
    let (minutes, seconds) = parse_mm_ss(entry_start_time)?;
    let offset_seconds = (minutes * 60 + seconds) as i64;

    // Calculate absolute timestamp
    let entry_timestamp = segment_start + Duration::seconds(offset_seconds);

    // Convert to nanoseconds
    entry_timestamp.timestamp_nanos_opt()
        .ok_or_else(|| format!("Timestamp out of range for nanoseconds"))
}

/**
 * Parse MM:SS format to minutes and seconds
 */
fn parse_mm_ss(time_str: &str) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = time_str.split(':').collect();
    
    if parts.len() != 2 {
        return Err(format!("Invalid time format '{}', expected MM:SS", time_str));
    }

    let minutes: u32 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid minutes in '{}'", time_str))?;
    
    let seconds: u32 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid seconds in '{}'", time_str))?;

    Ok((minutes, seconds))
}

/**
 * Calculate duration in seconds between two MM:SS timestamps
 * 
 * # Arguments
 * * `start_time` - Start time in "MM:SS" format
 * * `end_time` - End time in "MM:SS" format
 * 
 * # Returns
 * * `Ok(i64)` - Duration in seconds
 * * `Err(String)` - Error message if parsing fails
 */
fn calculate_duration_seconds(start_time: &str, end_time: &str) -> Result<i64, String> {
    let (start_min, start_sec) = parse_mm_ss(start_time)?;
    let (end_min, end_sec) = parse_mm_ss(end_time)?;
    
    let start_total = (start_min * 60 + start_sec) as i64;
    let end_total = (end_min * 60 + end_sec) as i64;
    
    Ok(end_total - start_total)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::formatter::timestamp_to_nanos;

    #[test]
    fn test_parse_mm_ss() {
        assert_eq!(parse_mm_ss("00:00").unwrap(), (0, 0));
        assert_eq!(parse_mm_ss("00:30").unwrap(), (0, 30));
        assert_eq!(parse_mm_ss("01:00").unwrap(), (1, 0));
        assert_eq!(parse_mm_ss("05:45").unwrap(), (5, 45));
        assert_eq!(parse_mm_ss("60:00").unwrap(), (60, 0));
    }

    #[test]
    fn test_parse_mm_ss_invalid() {
        assert!(parse_mm_ss("invalid").is_err());
        assert!(parse_mm_ss("1:2:3").is_err());
        assert!(parse_mm_ss("ab:cd").is_err());
    }

    #[test]
    fn test_calculate_duration_seconds() {
        // 30 second duration
        assert_eq!(calculate_duration_seconds("00:00", "00:30").unwrap(), 30);
        // 1 minute duration
        assert_eq!(calculate_duration_seconds("00:00", "01:00").unwrap(), 60);
        // 90 second duration
        assert_eq!(calculate_duration_seconds("00:30", "02:00").unwrap(), 90);
        // 5 minute duration
        assert_eq!(calculate_duration_seconds("01:00", "06:00").unwrap(), 300);
        // Same start and end (0 duration)
        assert_eq!(calculate_duration_seconds("01:30", "01:30").unwrap(), 0);
    }

    #[test]
    fn test_calculate_entry_timestamp() {
        let segment_start = "2025-01-15T10:00:00Z";
        
        // Entry at 00:00 should match segment start
        let ts_0 = calculate_entry_timestamp(segment_start, "00:00").unwrap();
        let expected_0 = timestamp_to_nanos(segment_start).unwrap();
        assert_eq!(ts_0, expected_0);
        
        // Entry at 01:30 should be 90 seconds later
        let ts_90 = calculate_entry_timestamp(segment_start, "01:30").unwrap();
        let expected_90 = timestamp_to_nanos("2025-01-15T10:01:30Z").unwrap();
        assert_eq!(ts_90, expected_90);
    }

    #[test]
    fn test_format_timeline_entry() {
        let analysis = TimelineAnalysis {
            segment_id: "segment_123_abc".to_string(),
            display_index: 0,
            analyzed_at: "2025-01-15T10:05:00Z".to_string(),
            video_duration_seconds: 300.0,
            segment_start_time: "2025-01-15T10:00:00Z".to_string(),
            timeline: vec![],
        };

        let entry = TimelineEntry {
            start_time: "00:00".to_string(),
            end_time: "00:30".to_string(),
            description: "User editing code".to_string(),
            active_application: "VS Code".to_string(),
            active_window_title: "main.rs - project".to_string(),
            productive_score: 5,
        };

        let result = format_timeline_entry(&analysis, &entry, "laptop1");
        assert!(result.is_ok());
        
        let line = result.unwrap();
        assert!(line.starts_with("screen_timeline,"));
        assert!(line.contains("display=0"));
        assert!(line.contains("app=VS\\ Code"));
        assert!(line.contains("hostname=laptop1"));
        assert!(line.contains("productive_score=5i"));
        assert!(line.contains("description=\"User editing code\""));
        assert!(line.contains("app_title=\"main.rs - project\""));
        assert!(line.contains("duration_seconds=30i"));
        // Verify removed fields/tags are NOT present
        assert!(!line.contains("window="));
        assert!(!line.contains("segment_id="));
        assert!(!line.contains("start_time="));
        assert!(!line.contains("end_time="));
    }
}

