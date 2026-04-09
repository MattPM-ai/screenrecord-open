// ActivityWatch client wrapper for interacting with the bundled aw-server
use crate::activitywatch::types::{
    AppUsage, BucketEventsResponse, BucketInfo, CurrentStatus, DailyMetrics, 
    DateRangeEventsResponse, EventInfo
};
use aw_client_rust::AwClient;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

// Global mutex to serialize AwClient access (prevents "Another instance is already running" errors)
static AW_CLIENT_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

// Singleton AwClient instance cache (using Arc for shared ownership)
static AW_CLIENT_INSTANCE: Lazy<Mutex<Option<Arc<AwClient>>>> = Lazy::new(|| Mutex::new(None));

// Track the base_url the cached client is connected to
static AW_CLIENT_BASE_URL: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

// Get or create a singleton ActivityWatch client connected to the specified base URL
// base_url should be in format "http://host:port"
// 
// This function implements a singleton pattern to reuse the same AwClient instance
// across multiple requests, preventing "Another instance is already running" errors.
pub fn get_or_create_client(base_url: &str) -> Result<Arc<AwClient>, String> {
    // Acquire lock to serialize AwClient access
    let _lock = AW_CLIENT_LOCK.lock().unwrap();
    
    // Check if we have a cached client for this base_url
    let cached_url = AW_CLIENT_BASE_URL.lock().unwrap().clone();
    
    if let Some(cached) = cached_url {
        if cached == base_url {
            // Base URL matches, check if we have a cached client
            let client_guard = AW_CLIENT_INSTANCE.lock().unwrap();
            if let Some(client_arc) = client_guard.as_ref() {
                log::debug!("Reusing cached AwClient for {}", base_url);
                // Clone the Arc (cheap reference count increment)
                return Ok(Arc::clone(client_arc));
            }
        } else {
            // Base URL changed, invalidate cache
            log::info!("Base URL changed from {} to {}, recreating client", cached, base_url);
            *AW_CLIENT_INSTANCE.lock().unwrap() = None;
            *AW_CLIENT_BASE_URL.lock().unwrap() = None;
        }
    }
    
    // No cached client or base_url changed, create new one
    log::info!("Creating singleton AwClient for {}", base_url);
    
    // Parse base_url to extract host and port
    let url = base_url
        .strip_prefix("http://")
        .or_else(|| base_url.strip_prefix("https://"))
        .unwrap_or(base_url);
    
    let parts: Vec<&str> = url.split(':').collect();
    let host = parts.get(0).unwrap_or(&"127.0.0.1");
    let port: u16 = parts
        .get(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(5600);
    
    let client = AwClient::new(host, port, "screenrecord-tracker")
        .map_err(|e| format!("Failed to create AwClient: {}", e))?;
    
    // Wrap in Arc for shared ownership
    let client_arc = Arc::new(client);
    
    // Cache the client and base_url
    *AW_CLIENT_INSTANCE.lock().unwrap() = Some(Arc::clone(&client_arc));
    *AW_CLIENT_BASE_URL.lock().unwrap() = Some(base_url.to_string());
    
    Ok(client_arc)
}

// Invalidate the cached AwClient instance
// This should be called when the server stops or becomes unhealthy
pub fn invalidate_client() {
    let _lock = AW_CLIENT_LOCK.lock().unwrap();
    *AW_CLIENT_INSTANCE.lock().unwrap() = None;
    *AW_CLIENT_BASE_URL.lock().unwrap() = None;
    log::info!("Invalidated AwClient cache");
}

// Convert aw_models Bucket to simplified BucketInfo for frontend
fn convert_bucket(
    bucket_id: String,
    bucket: aw_models::Bucket,
) -> BucketInfo {
    BucketInfo {
        id: bucket_id,
        bucket_type: bucket._type,
        client: bucket.client,
        hostname: bucket.hostname,
        created: bucket.created.map(|dt| dt.to_rfc3339()).unwrap_or_default(),
        event_count: None, // Not provided by bucket metadata
    }
}

// Convert aw_models Event to simplified EventInfo for frontend
fn convert_event(event: aw_models::Event) -> EventInfo {
    EventInfo {
        id: event.id,
        timestamp: event.timestamp.to_rfc3339(),
        duration: event.duration.num_milliseconds() as f64 / 1000.0,
        data: serde_json::Value::Object(event.data),
    }
}

// Find all bucket IDs that match the given bucket type(s)
// Returns bucket IDs sorted by creation date (newest first) to prioritize recent buckets
fn find_buckets_by_type(
    buckets: &HashMap<String, aw_models::Bucket>,
    bucket_types: &[&str],
) -> Vec<String> {
    // Filter buckets matching any of the provided types
    let mut matching_buckets: Vec<(String, Option<DateTime<Utc>>)> = buckets
        .iter()
        .filter(|(_, bucket)| bucket_types.contains(&bucket._type.as_str()))
        .map(|(id, bucket)| (id.clone(), bucket.created))
        .collect();
    
    // Sort by creation date (newest first)
    matching_buckets.sort_by(|a, b| b.1.cmp(&a.1));
    
    // Extract bucket IDs
    let bucket_ids: Vec<String> = matching_buckets
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    
    if bucket_ids.is_empty() {
        log::debug!("No buckets found matching types {:?}", bucket_types);
    } else {
        log::debug!(
            "Found {} buckets matching types {:?}: {:?}",
            bucket_ids.len(),
            bucket_types,
            bucket_ids
        );
    }
    
    bucket_ids
}

// Fetch events from multiple buckets and merge them into a single sorted list
// Events are sorted by timestamp (chronological order)
async fn fetch_and_merge_events(
    client: &AwClient,
    bucket_ids: &[String],
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    limit: Option<u64>,
) -> Result<Vec<aw_models::Event>, String> {
    // If no buckets, return empty vector
    if bucket_ids.is_empty() {
        return Ok(Vec::new());
    }
    
    log::info!(
        "Fetching events from {} bucket(s): {:?}",
        bucket_ids.len(),
        bucket_ids
    );
    
    let mut merged_events: Vec<aw_models::Event> = Vec::new();
    let mut error_count = 0;
    
    // Fetch events from each bucket
    for bucket_id in bucket_ids {
        match client.get_events(bucket_id, start_time, end_time, limit).await {
            Ok(events) => {
                merged_events.extend(events);
            }
            Err(e) => {
                log::warn!("Failed to fetch events from bucket {}: {}", bucket_id, e);
                error_count += 1;
            }
        }
    }
    
    // If all buckets failed, return error
    if error_count == bucket_ids.len() {
        return Err("Failed to fetch events from all buckets".to_string());
    }
    
    // Sort events by timestamp (chronological order)
    merged_events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    
    log::info!(
        "Merged {} total events from {} buckets",
        merged_events.len(),
        bucket_ids.len()
    );
    
    Ok(merged_events)
}

// Fetch all buckets from the ActivityWatch server
pub async fn fetch_buckets(base_url: &str) -> Result<Vec<BucketInfo>, String> {
    let client = get_or_create_client(base_url)?;
    
    let buckets: HashMap<String, aw_models::Bucket> = client
        .get_buckets()
        .await
        .map_err(|e| format!("Failed to fetch buckets: {}", e))?;
    
    let mut bucket_list: Vec<BucketInfo> = buckets
        .into_iter()
        .map(|(id, bucket)| convert_bucket(id, bucket))
        .collect();
    
    // Sort by created date (newest first)
    bucket_list.sort_by(|a, b| b.created.cmp(&a.created));
    
    Ok(bucket_list)
}

// Fetch recent events from a specific bucket
pub async fn fetch_bucket_events(
    base_url: &str,
    bucket_id: &str,
    limit: Option<u64>,
) -> Result<BucketEventsResponse, String> {
    let client = get_or_create_client(base_url)?;
    
    // Fetch events from the last 24 hours by default
    let end_time = Utc::now();
    let start_time = end_time - Duration::days(1);
    
    let events: Vec<aw_models::Event> = client
        .get_events(bucket_id, Some(start_time), Some(end_time), limit)
        .await
        .map_err(|e| format!("Failed to fetch events for bucket {}: {}", bucket_id, e))?;
    
    let total_count = events.len();
    let event_infos: Vec<EventInfo> = events
        .into_iter()
        .map(convert_event)
        .collect();
    
    Ok(BucketEventsResponse {
        bucket_id: bucket_id.to_string(),
        events: event_infos,
        total_count,
    })
}

// Fetch current real-time status from most recent events
pub async fn fetch_current_status(base_url: &str) -> Result<CurrentStatus, String> {
    let client = get_or_create_client(base_url)?;
    
    // Get all buckets to find window and afk buckets
    let buckets: HashMap<String, aw_models::Bucket> = client
        .get_buckets()
        .await
        .map_err(|e| format!("Failed to fetch buckets: {}", e))?;
    
    // Find ALL buckets of each type (handles duplicates from hostname changes)
    let window_buckets = find_buckets_by_type(&buckets, &["currentwindow", "app.editor.activity"]);
    let afk_buckets = find_buckets_by_type(&buckets, &["afkstatus"]);
    
    let now = Utc::now();
    let start_time = now - Duration::minutes(10); // Look back 10 minutes
    
    // Fetch most recent window event
    let mut current_app: Option<String> = None;
    let mut current_title: Option<String> = None;
    
    if !window_buckets.is_empty() {
        // Fetch and merge events from all window buckets
        let events = fetch_and_merge_events(
            &client,
            &window_buckets,
            Some(start_time),
            Some(now),
            Some(10), // Fetch more events since we're merging multiple buckets
        )
        .await
        .unwrap_or_default();
        
        // Get the most recent event (events are sorted chronologically)
        if let Some(event) = events.last() {
            current_app = event.data.get("app")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            current_title = event.data.get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
    }
    
    // Fetch most recent AFK event
    let mut afk_status = "unknown".to_string();
    let mut time_in_state = 0.0;
    let mut last_input_time: Option<String> = None;
    
    if !afk_buckets.is_empty() {
        // Fetch and merge events from all AFK buckets
        let events = fetch_and_merge_events(
            &client,
            &afk_buckets,
            Some(start_time),
            Some(now),
            Some(10), // Fetch more events since we're merging multiple buckets
        )
        .await
        .unwrap_or_default();
        
        // Get the most recent event (events are sorted chronologically)
        if let Some(event) = events.last() {
            afk_status = event.data.get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            
            // Calculate time in current state
            let event_start = event.timestamp;
            let event_duration = event.duration;
            let event_end = event_start + event_duration;
            
            if event_end >= now {
                // Event is still ongoing
                time_in_state = (now - event_start).num_milliseconds() as f64 / 1000.0;
            } else {
                // Event has ended
                time_in_state = event_duration.num_milliseconds() as f64 / 1000.0;
            }
            
            // Last input time is when the event started (if not AFK)
            if afk_status == "not-afk" {
                last_input_time = Some(event_start.to_rfc3339());
            }
        }
    }
    
    Ok(CurrentStatus {
        last_update: now.to_rfc3339(),
        current_app,
        current_title,
        afk_status,
        time_in_state,
        last_input_time,
    })
}

// Fetch events for a specific date range, grouped by bucket type
pub async fn fetch_events_by_range(
    base_url: &str,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Result<DateRangeEventsResponse, String> {
    let client = get_or_create_client(base_url)?;
    
    // Get all buckets
    let buckets: HashMap<String, aw_models::Bucket> = client
        .get_buckets()
        .await
        .map_err(|e| format!("Failed to fetch buckets: {}", e))?;
    
    // Find ALL buckets of each type (handles duplicates from hostname changes)
    let window_buckets = find_buckets_by_type(&buckets, &["currentwindow", "app.editor.activity"]);
    let afk_buckets = find_buckets_by_type(&buckets, &["afkstatus"]);
    let input_buckets = find_buckets_by_type(&buckets, &["afkinput"]);
    
    // Fetch window events
    let window_events = if !window_buckets.is_empty() {
        let events = fetch_and_merge_events(
            &client,
            &window_buckets,
            Some(start_time),
            Some(end_time),
            None,
        )
        .await
        .unwrap_or_default();
        events.into_iter().map(convert_event).collect()
    } else {
        Vec::new()
    };
    
    // Fetch AFK events
    let afk_events = if !afk_buckets.is_empty() {
        let events = fetch_and_merge_events(
            &client,
            &afk_buckets,
            Some(start_time),
            Some(end_time),
            None,
        )
        .await
        .unwrap_or_default();
        events.into_iter().map(convert_event).collect()
    } else {
        Vec::new()
    };
    
    // Fetch input events
    let input_events = if !input_buckets.is_empty() {
        let events = fetch_and_merge_events(
            &client,
            &input_buckets,
            Some(start_time),
            Some(end_time),
            None,
        )
        .await
        .unwrap_or_default();
        events.into_iter().map(convert_event).collect()
    } else {
        Vec::new()
    };
    
    Ok(DateRangeEventsResponse {
        window_events,
        afk_events,
        input_events,
    })
}

// Calculate aggregated metrics for a specific day
pub async fn calculate_daily_metrics(
    base_url: &str,
    date: NaiveDate,
) -> Result<DailyMetrics, String> {
    // Convert date to start and end times (00:00:00 to 23:59:59)
    let start_time = date.and_hms_opt(0, 0, 0)
        .ok_or("Invalid start time")?
        .and_local_timezone(Utc)
        .single()
        .ok_or("Invalid timezone conversion")?;
    
    let end_time = date.and_hms_opt(23, 59, 59)
        .ok_or("Invalid end time")?
        .and_local_timezone(Utc)
        .single()
        .ok_or("Invalid timezone conversion")?;
    
    // Fetch events for the day
    let events_response = fetch_events_by_range(base_url, start_time, end_time).await?;
    
    // Calculate active and idle time from AFK events
    let mut total_active_seconds = 0.0;
    let mut total_idle_seconds = 0.0;
    let mut total_afk_seconds = 0.0;
    
    for event in &events_response.afk_events {
        let status = event.data.get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        
        match status {
            "not-afk" => total_active_seconds += event.duration,
            "afk" => total_afk_seconds += event.duration,
            _ => total_idle_seconds += event.duration,
        }
    }
    
    // Calculate utilization ratio
    let total_tracked = total_active_seconds + total_idle_seconds + total_afk_seconds;
    let utilization_ratio = if total_tracked > 0.0 {
        total_active_seconds / total_tracked
    } else {
        0.0
    };
    
    // Count app switches (consecutive window events with different apps)
    let mut app_switches = 0;
    let mut last_app: Option<String> = None;
    
    for event in &events_response.window_events {
        if let Some(app) = event.data.get("app").and_then(|v| v.as_str()) {
            if let Some(last) = &last_app {
                if last != app {
                    app_switches += 1;
                }
            }
            last_app = Some(app.to_string());
        }
    }
    
    Ok(DailyMetrics {
        date: date.to_string(),
        total_active_seconds,
        total_idle_seconds,
        total_afk_seconds,
        utilization_ratio,
        app_switches,
    })
}

// Aggregate app usage statistics for a date range
pub async fn aggregate_app_usage(
    base_url: &str,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Result<Vec<AppUsage>, String> {
    // Fetch events for the range
    let events_response = fetch_events_by_range(base_url, start_time, end_time).await?;
    
    // Group window events by app
    let mut app_map: HashMap<String, (f64, Vec<String>, i32)> = HashMap::new();
    
    for event in &events_response.window_events {
        if let Some(app) = event.data.get("app").and_then(|v| v.as_str()) {
            let title = event.data.get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            let entry = app_map.entry(app.to_string()).or_insert((0.0, Vec::new(), 0));
            entry.0 += event.duration;
            if !title.is_empty() && !entry.1.contains(&title) {
                entry.1.push(title);
            }
            entry.2 += 1;
        }
    }
    
    // Convert to AppUsage structs and sort by duration
    let mut app_usage_list: Vec<AppUsage> = app_map
        .into_iter()
        .map(|(app_name, (total_seconds, window_titles, event_count))| AppUsage {
            app_name,
            total_seconds,
            window_titles,
            event_count,
            category: None, // Frontend will apply categories
        })
        .collect();
    
    app_usage_list.sort_by(|a, b| b.total_seconds.partial_cmp(&a.total_seconds).unwrap());
    
    Ok(app_usage_list)
}

