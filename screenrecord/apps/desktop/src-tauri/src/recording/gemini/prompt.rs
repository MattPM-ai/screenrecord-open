/**
 * ============================================================================
 * GEMINI PROMPT MODULE
 * ============================================================================
 * 
 * PURPOSE: Build prompts for Gemini video analysis
 * 
 * PROMPT STRATEGY:
 * - Instruct Gemini to analyze entire video duration
 * - Extract timeline of activities with productivity scores
 * - Return structured JSON for parsing
 * 
 * ============================================================================
 */

/**
 * Format duration in seconds to "MM:SS" string
 * 
 * # Arguments
 * * `seconds` - Duration in seconds
 * 
 * # Returns
 * String in "MM:SS" format
 */
pub fn format_duration(seconds: f64) -> String {
    let total_seconds = seconds.round() as u64;
    let minutes = total_seconds / 60;
    let secs = total_seconds % 60;
    format!("{:02}:{:02}", minutes, secs)
}

/**
 * Build the timeline analysis prompt for Gemini
 * 
 * # Arguments
 * * `playback_speed` - Video playback speed (1.0 = normal)
 * * `video_duration_seconds` - Total video duration in seconds
 * 
 * # Returns
 * Complete prompt string for Gemini API
 */
pub fn build_timeline_prompt(playback_speed: f64, video_duration_seconds: f64) -> String {
    let speed_note = if playback_speed > 1.0 {
        "\n\nIMPORTANT: The timestamps you provide should be based on the video's actual playback time (as if played at normal 1x speed).".to_string()
    } else {
        String::new()
    };

    let duration_str = format_duration(video_duration_seconds);
    let duration_note = if video_duration_seconds > 0.0 {
        format!(
            "\n\nVIDEO DURATION: This video is exactly {} ({} seconds) long. Your timeline MUST cover from 00:00 to {}. The last entry's endTime must be {} or very close to it.",
            duration_str,
            video_duration_seconds.round() as u64,
            duration_str,
            duration_str
        )
    } else {
        String::new()
    };

    format!(
        r#"Analyze this screen recording video and extract a timeline of activities.{}{}

CRITICAL: You MUST analyze and provide timeline entries for the ENTIRE video from start to finish. Do NOT truncate, cut off, or skip any portion of the video.

VALIDATION: Before responding, verify your timeline:
1. Does your first entry start at "00:00"?
2. Does your last entry end at or near "{}"?
3. Are there any gaps between entries?
If any of these checks fail, re-analyze the video and fix your response.

For each distinct activity or context switch you observe, create a timeline entry with:
- startTime: When this activity started in the video (format: "MM:SS")
- endTime: When this activity ended in the video (format: "MM:SS")
- description: Short concise context of what the user is doing that you have seen in the screen. Highlight the content that can cause negative sentiment or damaging company reputation.
- activeApplication: The application being used (e.g., "VS Code", "Chrome", "Terminal", "Slack")
- activeWindowTitle: The window title or tab name if visible
- productiveScore: A productivity level from 1-5 (integer only). Note that if the distraction is coming from a productive task for example error in a software is considered productive.

Guidelines for productiveScore (1-5 levels):
- 5: Highly Productive - Coding, writing documents, focused work, learning, deep concentration tasks
- 4: Productive - Email for work, work meetings, documentation, communication tools for work tasks
- 3: Neutral - General browsing for research, reading articles, light administrative tasks
- 2: Low Productivity - Casual browsing, social media for short breaks, off-topic reading
- 1: Distraction - Entertainment, gaming, extended social media use, watching videos unrelated to work

Return ONLY a valid JSON object with this exact structure (no markdown, no code blocks):
{{"timeline": [{{"startTime": "00:00", "endTime": "00:30", "description": "...", "activeApplication": "...", "activeWindowTitle": "...", "productiveScore": 5}}]}}"#,
        speed_note,
        duration_note,
        duration_str
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0.0), "00:00");
        assert_eq!(format_duration(30.0), "00:30");
        assert_eq!(format_duration(60.0), "01:00");
        assert_eq!(format_duration(90.0), "01:30");
        assert_eq!(format_duration(300.0), "05:00");
        assert_eq!(format_duration(3600.0), "60:00");
        assert_eq!(format_duration(65.5), "01:06"); // Rounds to 66 seconds
    }

    #[test]
    fn test_build_timeline_prompt_basic() {
        let prompt = build_timeline_prompt(1.0, 300.0);
        
        // Check key elements are present
        assert!(prompt.contains("Analyze this screen recording video"));
        assert!(prompt.contains("05:00"));
        assert!(prompt.contains("300 seconds"));
        assert!(prompt.contains("productiveScore"));
        assert!(prompt.contains("startTime"));
        assert!(prompt.contains("endTime"));
    }

    #[test]
    fn test_build_timeline_prompt_with_speed() {
        let prompt = build_timeline_prompt(2.0, 300.0);
        
        // Check speed note is included
        assert!(prompt.contains("actual playback time"));
        assert!(prompt.contains("normal 1x speed"));
    }

    #[test]
    fn test_build_timeline_prompt_no_duration() {
        let prompt = build_timeline_prompt(1.0, 0.0);
        
        // Duration note should not be present
        assert!(!prompt.contains("VIDEO DURATION"));
    }

    #[test]
    fn test_prompt_json_structure() {
        let prompt = build_timeline_prompt(1.0, 60.0);
        
        // Verify JSON example structure
        assert!(prompt.contains(r#""timeline""#));
        assert!(prompt.contains(r#""startTime""#));
        assert!(prompt.contains(r#""endTime""#));
        assert!(prompt.contains(r#""description""#));
        assert!(prompt.contains(r#""activeApplication""#));
        assert!(prompt.contains(r#""activeWindowTitle""#));
        assert!(prompt.contains(r#""productiveScore""#));
    }
}

