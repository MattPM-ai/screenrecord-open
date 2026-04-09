/**
 * ============================================================================
 * LINE PROTOCOL FORMATTER MODULE
 * ============================================================================
 * 
 * PURPOSE: Convert ActivityWatch events to InfluxDB line protocol format
 * 
 * LINE PROTOCOL FORMAT:
 * measurement,tag1=value1,tag2=value2 field1=value1,field2=value2 timestamp
 * 
 * ESCAPING RULES:
 * - Tag keys/values: Escape spaces, commas, equals signs
 * - Field strings: Escape double quotes, wrap in quotes
 * - Timestamps: Unix nanoseconds
 * 
 * REFERENCE: https://docs.influxdata.com/influxdb/v2.0/reference/syntax/line-protocol/
 * 
 * ============================================================================
 */

use chrono::DateTime;

/**
 * Escape tag key or value according to InfluxDB line protocol spec
 * Escapes: space, comma, equals sign
 * Strips: newlines (not allowed in tag values)
 */
pub fn escape_tag_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")  // Escape backslashes first
        .replace('\n', " ")     // Replace newlines with space (not allowed in tags)
        .replace('\r', "")      // Remove carriage returns
        .replace(' ', "\\ ")    // Escape spaces
        .replace(',', "\\,")    // Escape commas
        .replace('=', "\\=")    // Escape equals signs
}

/**
 * Escape and wrap field string value according to InfluxDB line protocol spec
 * Escapes double quotes, backslashes, and newlines, then wraps in double quotes
 */
pub fn escape_field_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")  // Escape backslashes first
        .replace('"', "\\\"")   // Escape double quotes
        .replace('\n', "\\n")   // Escape newlines
        .replace('\r', "");     // Remove carriage returns
    format!("\"{}\"", escaped)
}

/**
 * Convert ISO 8601 timestamp to Unix nanoseconds
 * Returns Result with nanoseconds or error message
 */
pub fn timestamp_to_nanos(timestamp: &str) -> Result<i64, String> {
    let dt = DateTime::parse_from_rfc3339(timestamp)
        .map_err(|e| format!("Failed to parse timestamp '{}': {}", timestamp, e))?;
    
    Ok(dt.timestamp_nanos_opt()
        .ok_or_else(|| format!("Timestamp '{}' out of range for nanoseconds", timestamp))?)
}

/**
 * Format window activity event to line protocol
 * 
 * Collector expects: duration as integer (seconds)
 * 
 * InfluxDB 2.0 requires: Tags cannot have empty values
 * 
 * Example output:
 * window_activity,app=Chrome,hostname=laptop1 title="Google Search",duration=45 1234567890000000000
 */
pub fn format_window_event(
    timestamp: &str,
    duration: f64,
    app: &str,
    title: &str,
    hostname: &str,
) -> Result<String, String> {
    let nanos = timestamp_to_nanos(timestamp)?;
    
    let measurement = "window_activity";
    let hostname_tag = escape_tag_value(hostname);
    let title_field = escape_field_string(title);
    
    // Convert duration to integer (seconds) as expected by collector
    let duration_seconds = duration as i64;
    
    // Build tags: only include app tag if it's not empty (InfluxDB 2.0 requirement)
    let tags = if app.is_empty() {
        format!("hostname={}", hostname_tag)
    } else {
        let app_tag = escape_tag_value(app);
        format!("app={},hostname={}", app_tag, hostname_tag)
    };
    
    Ok(format!(
        "{},{} title={},duration={} {}",
        measurement, tags, title_field, duration_seconds, nanos
    ))
}

/**
 * Format AFK status event to line protocol
 * 
 * Collector expects: duration as integer (seconds)
 * 
 * Example output:
 * afk_status,status=active,hostname=laptop1 duration=120 1234567890000000000
 */
pub fn format_afk_event(
    timestamp: &str,
    duration: f64,
    status: &str,
    hostname: &str,
) -> Result<String, String> {
    let nanos = timestamp_to_nanos(timestamp)?;
    
    let measurement = "afk_status";
    let status_tag = escape_tag_value(status);
    let hostname_tag = escape_tag_value(hostname);
    
    // Convert duration to integer (seconds) as expected by collector
    let duration_seconds = duration as i64;
    
    Ok(format!(
        "{},status={},hostname={} duration={} {}",
        measurement, status_tag, hostname_tag, duration_seconds, nanos
    ))
}

/**
 * Format daily metrics to line protocol
 * 
 * Collector expects: active_seconds, idle_seconds, afk_seconds as integers
 * 
 * Example output:
 * daily_metrics,date=2025-01-15,hostname=laptop1 active_seconds=25200,idle_seconds=3600,afk_seconds=1800,utilization_ratio=0.875,app_switches=42i 1234567890000000000
 */
pub fn format_daily_metrics(
    date: &str,
    active_seconds: f64,
    idle_seconds: f64,
    afk_seconds: f64,
    utilization_ratio: f64,
    app_switches: i32,
    hostname: &str,
) -> Result<String, String> {
    // Use end of day as timestamp
    let end_of_day = format!("{}T23:59:59Z", date);
    let nanos = timestamp_to_nanos(&end_of_day)?;
    
    let measurement = "daily_metrics";
    let date_tag = escape_tag_value(date);
    let hostname_tag = escape_tag_value(hostname);
    
    // Convert seconds to integers as expected by collector
    let active_seconds_int = active_seconds as i64;
    let idle_seconds_int = idle_seconds as i64;
    let afk_seconds_int = afk_seconds as i64;
    
    Ok(format!(
        "{},date={},hostname={} active_seconds={},idle_seconds={},afk_seconds={},utilization_ratio={},app_switches={}i {}",
        measurement, date_tag, hostname_tag, active_seconds_int, idle_seconds_int, afk_seconds_int, utilization_ratio, app_switches, nanos
    ))
}

/**
 * Format app usage statistics to line protocol
 * 
 * InfluxDB 2.0 requires: Tags cannot have empty values
 * 
 * Example output:
 * app_usage,app_name=VSCode,category=productive,hostname=laptop1 duration_seconds=7200,event_count=45i 1234567890000000000
 */
pub fn format_app_usage(
    timestamp: &str,
    app_name: &str,
    duration_seconds: f64,
    event_count: i32,
    category: Option<&str>,
    hostname: &str,
) -> Result<String, String> {
    let nanos = timestamp_to_nanos(timestamp)?;
    
    let measurement = "app_usage";
    let hostname_tag = escape_tag_value(hostname);
    
    // Build tags: only include app_name and category if they're not empty (InfluxDB 2.0 requirement)
    let mut tag_parts = Vec::new();
    
    if !app_name.is_empty() {
        let app_tag = escape_tag_value(app_name);
        tag_parts.push(format!("app_name={}", app_tag));
    }
    
    if let Some(cat) = category {
        if !cat.is_empty() {
            let cat_tag = escape_tag_value(cat);
            tag_parts.push(format!("category={}", cat_tag));
        }
    }
    
    tag_parts.push(format!("hostname={}", hostname_tag));
    
    let tags = tag_parts.join(",");
    
    Ok(format!(
        "{},{} duration_seconds={},event_count={}i {}",
        measurement, tags, duration_seconds, event_count, nanos
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_tag_value() {
        assert_eq!(escape_tag_value("simple"), "simple");
        assert_eq!(escape_tag_value("with space"), "with\\ space");
        assert_eq!(escape_tag_value("with,comma"), "with\\,comma");
        assert_eq!(escape_tag_value("with=equals"), "with\\=equals");
        assert_eq!(escape_tag_value("with backslash\\"), "with\\ backslash\\\\");
        assert_eq!(escape_tag_value("all ,=\\ special"), "all\\ \\,\\=\\\\\\ special");
        // Newline handling
        assert_eq!(escape_tag_value("with\nnewline"), "with\\ newline");
        assert_eq!(escape_tag_value("with\r\nwindows"), "with\\ windows");
        assert_eq!(escape_tag_value("multi\nline\nvalue"), "multi\\ line\\ value");
    }

    #[test]
    fn test_escape_field_string() {
        assert_eq!(escape_field_string("simple"), "\"simple\"");
        assert_eq!(escape_field_string("with \"quotes\""), "\"with \\\"quotes\\\"\"");
        assert_eq!(escape_field_string("with\\backslash"), "\"with\\\\backslash\"");
        assert_eq!(escape_field_string("both\"\\chars"), "\"both\\\"\\\\chars\"");
        // Newline handling
        assert_eq!(escape_field_string("with\nnewline"), "\"with\\nnewline\"");
        assert_eq!(escape_field_string("with\r\nwindows"), "\"with\\nwindows\"");
        assert_eq!(escape_field_string("multi\nline\nvalue"), "\"multi\\nline\\nvalue\"");
    }

    #[test]
    fn test_timestamp_to_nanos() {
        // Valid ISO 8601 timestamp
        let result = timestamp_to_nanos("2025-01-19T12:00:00Z");
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);

        // Invalid timestamp
        let result = timestamp_to_nanos("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_format_window_event() {
        let result = format_window_event(
            "2025-01-19T12:00:00Z",
            45.2,
            "Google Chrome",
            "Test Page - Google",
            "laptop1",
        );
        
        assert!(result.is_ok());
        let line = result.unwrap();
        assert!(line.starts_with("window_activity,"));
        assert!(line.contains("app=Google\\ Chrome"));
        assert!(line.contains("hostname=laptop1"));
        assert!(line.contains("title=\"Test Page - Google\""));
        assert!(line.contains("duration=45")); // Duration is now integer
    }

    #[test]
    fn test_format_window_event_with_special_chars() {
        let result = format_window_event(
            "2025-01-19T12:00:00Z",
            10.0,
            "App, With=Special",
            "Title with \"quotes\"",
            "host,name",
        );
        
        assert!(result.is_ok());
        let line = result.unwrap();
        assert!(line.contains("app=App\\,\\ With\\=Special"));
        assert!(line.contains("title=\"Title with \\\"quotes\\\"\""));
        assert!(line.contains("hostname=host\\,name"));
        assert!(line.contains("duration=10")); // Duration is now integer
    }

    #[test]
    fn test_format_window_event_with_empty_app() {
        // InfluxDB 2.0: Empty tag values are not allowed, so app tag should be skipped
        let result = format_window_event(
            "2025-01-19T12:00:00Z",
            5.0,
            "",
            "Some Title",
            "laptop1",
        );
        
        assert!(result.is_ok());
        let line = result.unwrap();
        assert!(line.starts_with("window_activity,"));
        assert!(!line.contains("app=")); // Empty app tag should be skipped
        assert!(line.contains("hostname=laptop1"));
        assert!(line.contains("title=\"Some Title\""));
        assert!(line.contains("duration=5"));
    }

    #[test]
    fn test_format_afk_event() {
        let result = format_afk_event(
            "2025-01-19T12:00:00Z",
            120.5,
            "active",
            "laptop1",
        );
        
        assert!(result.is_ok());
        let line = result.unwrap();
        assert!(line.starts_with("afk_status,"));
        assert!(line.contains("status=active"));
        assert!(line.contains("hostname=laptop1"));
        assert!(line.contains("duration=120")); // Duration is now integer
    }

    #[test]
    fn test_format_daily_metrics() {
        let result = format_daily_metrics(
            "2025-01-19",
            25200.0,
            3600.0,
            1800.0,
            0.875,
            42,
            "laptop1",
        );
        
        assert!(result.is_ok());
        let line = result.unwrap();
        assert!(line.starts_with("daily_metrics,"));
        assert!(line.contains("date=2025-01-19"));
        assert!(line.contains("hostname=laptop1"));
        assert!(line.contains("active_seconds=25200"));
        assert!(line.contains("idle_seconds=3600"));
        assert!(line.contains("afk_seconds=1800"));
        assert!(line.contains("utilization_ratio=0.875"));
        assert!(line.contains("app_switches=42i"));
    }

    #[test]
    fn test_format_app_usage() {
        let result = format_app_usage(
            "2025-01-19T12:00:00Z",
            "Visual Studio Code",
            7200.0,
            45,
            Some("productive"),
            "laptop1",
        );
        
        assert!(result.is_ok());
        let line = result.unwrap();
        assert!(line.starts_with("app_usage,"));
        assert!(line.contains("app_name=Visual\\ Studio\\ Code"));
        assert!(line.contains("category=productive"));
        assert!(line.contains("hostname=laptop1"));
        assert!(line.contains("duration_seconds=7200"));
        assert!(line.contains("event_count=45i"));
    }

    #[test]
    fn test_format_app_usage_without_category() {
        let result = format_app_usage(
            "2025-01-19T12:00:00Z",
            "Unknown App",
            100.0,
            5,
            None,
            "laptop1",
        );
        
        assert!(result.is_ok());
        let line = result.unwrap();
        assert!(line.starts_with("app_usage,"));
        assert!(!line.contains("category="));
        assert!(line.contains("app_name=Unknown\\ App"));
    }

    #[test]
    fn test_format_app_usage_with_empty_app_name() {
        // InfluxDB 2.0: Empty tag values are not allowed, so app_name tag should be skipped
        let result = format_app_usage(
            "2025-01-19T12:00:00Z",
            "",
            100.0,
            5,
            None,
            "laptop1",
        );
        
        assert!(result.is_ok());
        let line = result.unwrap();
        assert!(line.starts_with("app_usage,"));
        assert!(!line.contains("app_name=")); // Empty app_name tag should be skipped
        assert!(line.contains("hostname=laptop1"));
        assert!(line.contains("duration_seconds=100"));
        assert!(line.contains("event_count=5i"));
    }

    #[test]
    fn test_format_app_usage_with_empty_category() {
        // Empty category should be skipped even if provided
        let result = format_app_usage(
            "2025-01-19T12:00:00Z",
            "Some App",
            100.0,
            5,
            Some(""),
            "laptop1",
        );
        
        assert!(result.is_ok());
        let line = result.unwrap();
        assert!(line.starts_with("app_usage,"));
        assert!(!line.contains("category=")); // Empty category should be skipped
        assert!(line.contains("app_name=Some\\ App"));
        assert!(line.contains("hostname=laptop1"));
    }
}

