// Types used by ActivityWatch manager and Tauri commands
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub base_url: String,
    pub port: u16,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub healthy: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherStatus {
    pub name: String,
    pub running: bool,
}

// Simplified bucket information for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    pub id: String,
    pub bucket_type: String,
    pub client: String,
    pub hostname: String,
    pub created: String,           // ISO 8601 string for frontend
    pub event_count: Option<i64>,  // May not be available initially
}

// Simplified event information for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInfo {
    pub id: Option<i64>,
    pub timestamp: String,         // ISO 8601 string for frontend
    pub duration: f64,             // Duration in seconds (easier for frontend)
    pub data: JsonValue,           // Raw JSON data from the event
}

// Response wrapper for bucket events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketEventsResponse {
    pub bucket_id: String,
    pub events: Vec<EventInfo>,
    pub total_count: usize,
}

// Current real-time status of user activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentStatus {
    pub last_update: String,          // ISO 8601 timestamp
    pub current_app: Option<String>,
    pub current_title: Option<String>,
    pub afk_status: String,           // "active" | "idle" | "afk"
    pub time_in_state: f64,           // seconds in current state
    pub last_input_time: Option<String>,
}

// Events grouped by type for a date range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRangeEventsResponse {
    pub window_events: Vec<EventInfo>,
    pub afk_events: Vec<EventInfo>,
    pub input_events: Vec<EventInfo>,
}

// Aggregated metrics for a specific day
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyMetrics {
    pub date: String,
    pub total_active_seconds: f64,
    pub total_idle_seconds: f64,
    pub total_afk_seconds: f64,
    pub utilization_ratio: f64,      // active / (active + idle + afk)
    pub app_switches: i32,
}

// Application usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUsage {
    pub app_name: String,
    pub total_seconds: f64,
    pub window_titles: Vec<String>,
    pub event_count: i32,
    pub category: Option<String>,    // "productive" | "neutral" | "unproductive"
}
