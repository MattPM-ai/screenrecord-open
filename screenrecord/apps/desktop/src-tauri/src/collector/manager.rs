/**
 * ============================================================================
 * TRANSMISSION MANAGER MODULE
 * ============================================================================
 * 
 * PURPOSE: High-level orchestration of data transmission lifecycle
 * 
 * RESPONSIBILITIES:
 * - Manage WebSocket client lifecycle
 * - Coordinate batch flushing and transmission
 * - Implement retry logic with exponential backoff
 * - Handle reconnection on disconnect
 * - Provide Tauri commands for frontend control
 * 
 * BACKGROUND TASK:
 * - Runs continuously when collector is started
 * - Monitors batch manager for flush triggers
 * - Attempts transmission with retry on failure
 * - Sends keepalive pings
 * - Handles offline queue drainage
 * 
 * ============================================================================
 */

use crate::collector::{batch, client, config, types, bridge};
use crate::activitywatch::{manager as aw_manager, client as aw_client};
use once_cell::sync::Lazy;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;
use tokio::task::JoinHandle;
use chrono::{DateTime, Utc};

// Global background task handle
static BACKGROUND_HANDLE: Lazy<Mutex<Option<JoinHandle<()>>>> = 
    Lazy::new(|| Mutex::new(None));

// ActivityWatch polling task handle
static AW_POLLING_HANDLE: Lazy<Mutex<Option<JoinHandle<()>>>> = 
    Lazy::new(|| Mutex::new(None));

// Daily metrics collection task handle
static DAILY_METRICS_HANDLE: Lazy<Mutex<Option<JoinHandle<()>>>> = 
    Lazy::new(|| Mutex::new(None));

// Shutdown signal for background task
static SHUTDOWN_SIGNAL: Lazy<Arc<AtomicBool>> = 
    Lazy::new(|| Arc::new(AtomicBool::new(false)));

// Last event timestamp to avoid duplicates
static LAST_EVENT_TIMESTAMP: Lazy<Arc<Mutex<Option<DateTime<Utc>>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(None)));

// Global connection status - shared between background task and status queries
static CONNECTION_STATUS: Lazy<Arc<Mutex<types::TransmissionStatus>>> = 
    Lazy::new(|| Arc::new(Mutex::new(types::TransmissionStatus::Disconnected)));

// Flag indicating if collector is running
static COLLECTOR_RUNNING: Lazy<AtomicBool> = 
    Lazy::new(|| AtomicBool::new(false));

/**
 * Update the global connection status
 * Called by background task when status changes
 */
fn update_connection_status(status: types::TransmissionStatus) {
    let mut global_status = CONNECTION_STATUS.lock().unwrap();
    *global_status = status;
}

/**
 * Get the current connection status
 * Thread-safe read from global state
 */
fn get_connection_status() -> types::TransmissionStatus {
    CONNECTION_STATUS.lock().unwrap().clone()
}

/**
 * Background transmission task
 * Runs continuously until shutdown signal
 * Updates global CONNECTION_STATUS for UI queries
 */
async fn transmission_background_task(_app_handle: AppHandle, initial_config: config::CollectorConfig) {
    log::info!("Starting transmission background task");
    
    // Start with initial config, but we'll get fresh config (with token) from cache when needed
    let mut ws_client = client::CollectorClient::new(initial_config.clone());
    let mut retry_state = types::RetryState::default();
    let mut last_ping_time = std::time::Instant::now();
    let mut consecutive_failures = 0;
    
    // Get initial config for retry settings (these don't change)
    let get_config_for_retry = || {
        config::get_cached_config()
            .unwrap_or(initial_config.clone())
    };
    
    // Initial connection attempt
    update_connection_status(types::TransmissionStatus::Connecting);
    if let Err(e) = ws_client.connect().await {
        log::error!("Initial connection failed: {}", e);
        update_connection_status(types::TransmissionStatus::Error(e.clone()));
        let _ = batch::record_error(format!("Initial connection failed: {}", e));
    } else {
        update_connection_status(types::TransmissionStatus::Authenticating);
        if let Err(e) = ws_client.authenticate().await {
            log::error!("Initial authentication failed: {}", e);
            update_connection_status(types::TransmissionStatus::Error(e.clone()));
            let _ = batch::record_error(format!("Initial authentication failed: {}", e));
        } else {
            log::info!("Initial connection successful");
            update_connection_status(types::TransmissionStatus::Connected);
        }
    }

    // Main loop
    loop {
        // Check shutdown signal
        if SHUTDOWN_SIGNAL.load(Ordering::Relaxed) {
            log::info!("Shutdown signal received, exiting background task");
            break;
        }

        // Get client status and update global status
        let status = ws_client.get_status();
        update_connection_status(status.clone());
        
        // Get fresh config for this iteration
        let current_config = get_config_for_retry();
        
        if status != types::TransmissionStatus::Connected {
            // Attempt reconnection if auto_reconnect enabled
            if current_config.auto_reconnect && retry_state.should_retry_now() {
                log::info!("Attempting reconnection (attempt {})", retry_state.attempts + 1);
                update_connection_status(types::TransmissionStatus::Connecting);
                
                match ws_client.connect().await {
                    Ok(()) => {
                        update_connection_status(types::TransmissionStatus::Authenticating);
                        match ws_client.authenticate().await {
                            Ok(()) => {
                                log::info!("Reconnection successful");
                                update_connection_status(types::TransmissionStatus::Connected);
                                retry_state.reset();
                                consecutive_failures = 0;
                            }
                            Err(e) => {
                                log::error!("Reconnection auth failed: {}", e);
                                update_connection_status(types::TransmissionStatus::Error(e.clone()));
                                let _ = batch::record_error(format!("Auth failed: {}", e));
                                
                                let delay_ms = retry_state.calculate_next_delay(
                                    current_config.retry_backoff_base_ms,
                                    current_config.retry_backoff_multiplier,
                                    current_config.retry_max_delay_seconds,
                                );
                                retry_state.increment(delay_ms);
                                consecutive_failures += 1;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Reconnection failed: {}", e);
                        update_connection_status(types::TransmissionStatus::Error(e.clone()));
                        let _ = batch::record_error(format!("Connection failed: {}", e));
                        
                        // Calculate next retry delay with exponential backoff
                        let delay_ms = retry_state.calculate_next_delay(
                            current_config.retry_backoff_base_ms,
                            current_config.retry_backoff_multiplier,
                            current_config.retry_max_delay_seconds,
                        );
                        retry_state.increment(delay_ms);
                        
                        consecutive_failures += 1;
                    }
                }
                
                // If max attempts reached, wait longer before resetting
                if consecutive_failures >= current_config.retry_max_attempts {
                    log::warn!("Max retry attempts reached, backing off for 5 minutes");
                    tokio::time::sleep(Duration::from_secs(300)).await;
                    consecutive_failures = 0;
                    retry_state.reset();
                }
            }
        } else {
            // Connected - handle normal operations
            
            // Check if batch should be flushed
            if let Ok(should_flush) = batch::should_flush() {
                if should_flush {
                    log::debug!("Batch flush triggered");
                    
                    if let Ok(Some(batch_to_send)) = batch::flush() {
                        // Attempt to send batch
                        match ws_client.send_batch(&batch_to_send).await {
                            Ok(events_sent) => {
                                log::info!("Batch sent successfully: {} events", events_sent);
                                let _ = batch::record_success(events_sent);
                                retry_state.reset();
                            }
                            Err(e) => {
                                log::error!("Batch send failed: {}", e);
                                let _ = batch::record_error(format!("Send failed: {}", e));
                                
                                // Enqueue batch for retry
                                if let Err(enqueue_err) = batch::enqueue_batch(batch_to_send) {
                                    log::error!("Failed to enqueue batch: {}", enqueue_err);
                                }
                            }
                        }
                    }
                }
            }

            // Try to drain offline queue if not empty
            if let Ok(Some(queued_batch)) = batch::dequeue_batch() {
                log::info!("Attempting to send queued batch: {}", queued_batch.batch_id);
                
                match ws_client.send_batch(&queued_batch).await {
                    Ok(events_sent) => {
                        log::info!("Queued batch sent successfully: {} events", events_sent);
                        let _ = batch::record_success(events_sent);
                        let _ = batch::increment_retry_attempts();
                    }
                    Err(e) => {
                        log::error!("Queued batch send failed: {}", e);
                        let _ = batch::record_error(format!("Queued send failed: {}", e));
                        
                        // Re-enqueue at back of queue
                        if let Err(enqueue_err) = batch::enqueue_batch(queued_batch) {
                            log::error!("Failed to re-enqueue batch: {}", enqueue_err);
                        }
                        
                        let _ = batch::increment_retry_attempts();
                        
                        // Back off on queue processing if failing
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }

            // Send keepalive ping if interval exceeded
            let elapsed_since_ping = last_ping_time.elapsed();
            if elapsed_since_ping.as_secs() >= current_config.websocket_keepalive_seconds {
                // Get current batch status for logging
                let pending_events = batch::get_statistics()
                    .map(|s| s.pending_events)
                    .unwrap_or(0);
                
                log::debug!("Sending keepalive ping: connection_uptime={}s, pending_events={}", 
                    elapsed_since_ping.as_secs(), pending_events);
                
                match ws_client.ping().await {
                    Ok(()) => {
                        log::debug!("Keepalive ping successful");
                    }
                    Err(e) => {
                        log::warn!("Keepalive ping failed: {}, triggering reconnection", e);
                        let _ = batch::record_error(format!("Keepalive ping failed: {}", e));
                        // Ping failure indicates connection problem, will trigger reconnection
                    }
                }
                last_ping_time = std::time::Instant::now();
            }
        }

        // Sleep before next iteration
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Cleanup on shutdown
    log::info!("Background task shutting down");
    update_connection_status(types::TransmissionStatus::Disconnected);
    
    // Flush any pending batch
    if let Ok(Some(final_batch)) = batch::flush() {
        log::info!("Flushing final batch on shutdown");
        if let Err(e) = ws_client.send_batch(&final_batch).await {
            log::warn!("Failed to send final batch: {}", e);
            // Enqueue for next session
            let _ = batch::enqueue_batch(final_batch);
        }
    }
    
    // Disconnect gracefully
    ws_client.disconnect().await;
}

/**
 * Background task to poll ActivityWatch for new window events
 * Polls every 2 seconds and sends new events immediately to collector
 */
async fn activitywatch_polling_task(app_handle: AppHandle) {
    let poll_interval = Duration::from_secs(2); // Poll every 2 seconds
    
    loop {
        // Check shutdown signal
        if SHUTDOWN_SIGNAL.load(Ordering::Relaxed) {
            break;
        }
        
        // Check if collector is enabled
        if !config::is_enabled() {
            tokio::time::sleep(poll_interval).await;
            continue;
        }
        
        // Get ActivityWatch base URL
        let base_url_opt = {
            aw_manager::AW_BASE_URL.lock().unwrap().clone()
        };
        
        let base_url = match base_url_opt {
            Some(url) => url,
            None => {
                tokio::time::sleep(poll_interval).await;
                continue;
            }
        };
        
        // Check if server is healthy
        let healthy = aw_manager::wait_healthy(&base_url, Duration::from_secs(1)).await;
        if !healthy {
            tokio::time::sleep(poll_interval).await;
            continue;
        }
        
        // Determine time range: from last event timestamp (or 5 minutes ago if first run) to now
        let now = Utc::now();
        let start_time = {
            let last_ts = LAST_EVENT_TIMESTAMP.lock().unwrap();
            if let Some(ts) = *last_ts {
                // Start from last event timestamp (exclusive) to avoid duplicates
                ts
            } else {
                // First run: look back 5 minutes to catch recent events
                now - chrono::Duration::minutes(5)
            }
        };
        
        // Fetch new events since last poll
        match aw_client::fetch_events_by_range(&base_url, start_time, now).await {
            Ok(events) => {
                let window_count = events.window_events.len();
                let afk_count = events.afk_events.len();
                
                if window_count > 0 || afk_count > 0 {
                    // Track the maximum end time across all events to avoid missing events
                    let mut max_end_time: Option<DateTime<Utc>> = None;
                    
                    // Send window events to collector
                    for event in &events.window_events {
                        if bridge::collect_window_event(&app_handle, event).is_err() {
                            // Silently continue on error
                        } else {
                            // Calculate event end time (timestamp + duration) to avoid missing overlapping events
                            if let Ok(ts) = DateTime::parse_from_rfc3339(&event.timestamp) {
                                let event_start = ts.with_timezone(&Utc);
                                let event_end = event_start + chrono::Duration::seconds(event.duration as i64);
                                
                                if max_end_time.is_none() || event_end > max_end_time.unwrap() {
                                    max_end_time = Some(event_end);
                                }
                            }
                        }
                    }
                    
                    // Send AFK events to collector
                    for event in &events.afk_events {
                        if bridge::collect_afk_event(&app_handle, event).is_err() {
                            // Silently continue on error
                        } else {
                            // Calculate event end time (timestamp + duration) to avoid missing overlapping events
                            if let Ok(ts) = DateTime::parse_from_rfc3339(&event.timestamp) {
                                let event_start = ts.with_timezone(&Utc);
                                let event_end = event_start + chrono::Duration::seconds(event.duration as i64);
                                
                                if max_end_time.is_none() || event_end > max_end_time.unwrap() {
                                    max_end_time = Some(event_end);
                                }
                            }
                        }
                    }
                    
                    // Update last event timestamp to the maximum end time (add 1ms buffer to avoid edge cases)
                    if let Some(end_time) = max_end_time {
                        let mut last_ts = LAST_EVENT_TIMESTAMP.lock().unwrap();
                        let new_ts = end_time + chrono::Duration::milliseconds(1);
                        if last_ts.is_none() || new_ts > last_ts.unwrap() {
                            *last_ts = Some(new_ts);
                        }
                    }
                }
            }
            Err(_e) => {
                // Silently continue on error
            }
        }
        
        // Sleep before next poll
        tokio::time::sleep(poll_interval).await;
    }
}

/**
 * Background task to collect daily metrics periodically
 * Collects metrics for the current day every hour
 */
async fn daily_metrics_collection_task(app_handle: AppHandle) {
    let collection_interval = Duration::from_secs(3600); // Collect every hour
    
    log::info!("[DAILY_METRICS_TASK] Starting daily metrics collection task");
    
    // Initial delay to allow ActivityWatch to be ready (reduced from 60s to 5s)
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    loop {
        // Check shutdown signal
        if SHUTDOWN_SIGNAL.load(Ordering::Relaxed) {
            break;
        }
        
        // Check if collector is enabled
        if !config::is_enabled() {
            log::debug!("[DAILY_METRICS_TASK] Collector disabled, skipping collection");
            tokio::time::sleep(collection_interval).await;
            continue;
        }
        
        // Get ActivityWatch base URL
        let base_url_opt = {
            aw_manager::AW_BASE_URL.lock().unwrap().clone()
        };
        
        let base_url = match base_url_opt {
            Some(url) => url,
            None => {
                log::debug!("[DAILY_METRICS_TASK] ActivityWatch base URL not available, skipping collection");
                tokio::time::sleep(collection_interval).await;
                continue;
            }
        };
        
        // Check if server is healthy
        let healthy = aw_manager::wait_healthy(&base_url, Duration::from_secs(1)).await;
        if !healthy {
            log::debug!("[DAILY_METRICS_TASK] ActivityWatch server not healthy, skipping collection");
            tokio::time::sleep(collection_interval).await;
            continue;
        }
        
        log::info!("[DAILY_METRICS_TASK] Starting collection cycle");
        
        // Get current date (today)
        let today = Utc::now().date_naive();
        let date_str = today.format("%Y-%m-%d").to_string();
        
        // Calculate start and end times for today (00:00:00 to 23:59:59)
        let start_time = match today.and_hms_opt(0, 0, 0)
            .and_then(|dt| dt.and_local_timezone(Utc).single())
        {
            Some(dt) => dt,
            None => {
                log::warn!("Failed to create start time for {}, skipping collection", date_str);
                tokio::time::sleep(collection_interval).await;
                continue;
            }
        };
        
        let end_time = match today.and_hms_opt(23, 59, 59)
            .and_then(|dt| dt.and_local_timezone(Utc).single())
        {
            Some(dt) => dt,
            None => {
                log::warn!("Failed to create end time for {}, skipping collection", date_str);
                tokio::time::sleep(collection_interval).await;
                continue;
            }
        };
        
        // Calculate daily metrics for today
        log::info!("[DAILY_METRICS_TASK] Calculating daily metrics for {}", date_str);
        match aw_client::calculate_daily_metrics(&base_url, today).await {
            Ok(metrics) => {
                log::info!("[DAILY_METRICS_TASK] Calculated daily metrics: active={}s, idle={}s, afk={}s, utilization={}, switches={}",
                    metrics.total_active_seconds, metrics.total_idle_seconds, 
                    metrics.total_afk_seconds, metrics.utilization_ratio, metrics.app_switches);
                // Send metrics to collector
                if let Err(e) = bridge::collect_daily_metrics(&app_handle, &metrics) {
                    log::error!("[DAILY_METRICS_TASK] Failed to collect daily metrics for {}: {}", date_str, e);
                } else {
                    log::info!("[DAILY_METRICS_TASK] Successfully collected daily metrics for {}", date_str);
                }
            }
            Err(e) => {
                log::error!("[DAILY_METRICS_TASK] Failed to calculate daily metrics for {}: {}", date_str, e);
            }
        }
        
        // Collect app usage for today
        log::info!("[DAILY_METRICS_TASK] Aggregating app usage for {}", date_str);
        match aw_client::aggregate_app_usage(&base_url, start_time, end_time).await {
            Ok(app_usage) => {
                log::info!("[DAILY_METRICS_TASK] Aggregated {} apps", app_usage.len());
                // Format end time as ISO 8601 string for the collector
                let end_time_str = end_time.to_rfc3339();
                // Send app usage to collector
                if let Err(e) = bridge::collect_app_usage(&app_handle, &app_usage, &end_time_str) {
                    log::error!("[DAILY_METRICS_TASK] Failed to collect app usage for {}: {}", date_str, e);
                } else {
                    log::info!("[DAILY_METRICS_TASK] Successfully collected app usage for {} ({} apps)", date_str, app_usage.len());
                }
            }
            Err(e) => {
                log::error!("[DAILY_METRICS_TASK] Failed to aggregate app usage for {}: {}", date_str, e);
            }
        }
        
        log::info!("[DAILY_METRICS_TASK] Collection cycle complete, flushing batch to ensure data is sent");
        
        // Flush the batch to ensure daily metrics and app usage are sent immediately
        if let Ok(Some(_batch)) = batch::flush() {
            log::info!("[DAILY_METRICS_TASK] Batch flushed successfully");
        } else {
            log::warn!("[DAILY_METRICS_TASK] Failed to flush batch");
        }
        
        log::info!("[DAILY_METRICS_TASK] Sleeping for {} seconds until next collection", collection_interval.as_secs());
        // Sleep before next collection
        tokio::time::sleep(collection_interval).await;
    }
}

/**
 * ============================================================================
 * TAURI COMMANDS
 * ============================================================================
 * 
 * Public API for frontend to control collector
 */

/**
 * Start collector with given configuration
 * Initializes batch manager and starts background transmission task
 */
#[tauri::command]
pub async fn start_collector(
    app: AppHandle,
    config: config::CollectorConfig,
) -> Result<(), String> {
    log::info!("[START_COLLECTOR] Starting collector");
    
    // Log whether app JWT token is provided (optional for mock-auth)
    if let Some(ref token) = config.app_jwt_token {
        log::info!("[START_COLLECTOR] App JWT token provided: length={}", token.len());
    } else {
        log::debug!("[START_COLLECTOR] No app JWT token in config - using mock-auth (JWT optional)");
    }

    // Validate configuration
    config.validate()?;

    // Save configuration (note: app_jwt_token is not persisted to disk for security)
    config::save_config(&app, &config)?;

    // Initialize config cache (this preserves the token in memory)
    config::init_cache(config.clone());

    // Check if already running
    if COLLECTOR_RUNNING.load(Ordering::Relaxed) {
        return Err("Collector already running".to_string());
    }

    // Initialize batch manager
    batch::init(&app, config.clone())?;

    // Reset shutdown signal and set running flag
    SHUTDOWN_SIGNAL.store(false, Ordering::Relaxed);
    COLLECTOR_RUNNING.store(true, Ordering::Relaxed);

    // Spawn background transmission task
    let app_clone = app.clone();
    let config_clone = config.clone();
    let handle = tokio::spawn(async move {
        transmission_background_task(app_clone, config_clone).await;
    });

    // Store transmission task handle
    {
        let mut bg_handle = BACKGROUND_HANDLE.lock().unwrap();
        *bg_handle = Some(handle);
    }

    // Spawn ActivityWatch polling task
    let app_clone2 = app.clone();
    let aw_handle = tokio::spawn(async move {
        activitywatch_polling_task(app_clone2).await;
    });

    // Store ActivityWatch polling task handle
    {
        let mut aw_handle_guard = AW_POLLING_HANDLE.lock().unwrap();
        *aw_handle_guard = Some(aw_handle);
    }

    // Spawn daily metrics collection task
    let app_clone3 = app.clone();
    let metrics_handle = tokio::spawn(async move {
        daily_metrics_collection_task(app_clone3).await;
    });

    // Store daily metrics task handle
    {
        let mut metrics_handle_guard = DAILY_METRICS_HANDLE.lock().unwrap();
        *metrics_handle_guard = Some(metrics_handle);
    }

    log::info!("Collector started successfully");
    Ok(())
}

/**
 * Update app JWT token in collector config cache
 * This allows updating the token without restarting the collector
 * Useful when token is refreshed after expiration
 */
#[tauri::command]
pub async fn update_collector_app_jwt_token(token: Option<String>) -> Result<(), String> {
    log::info!("Updating app JWT token in collector config cache");
    
    if let Some(ref t) = token {
        log::info!("New token provided: length={}", t.len());
    } else {
        log::info!("Clearing app JWT token from cache");
    }
    
    config::update_app_jwt_token(token);
    Ok(())
}

/**
 * Manually trigger daily metrics and app usage collection
 * Useful for testing or immediate data collection
 */
#[tauri::command]
pub async fn trigger_daily_collection(app: AppHandle) -> Result<String, String> {
    log::info!("[TRIGGER_COLLECTION] Manual daily collection triggered");
    
    // Check if collector is enabled
    if !config::is_enabled() {
        return Err("Collector is not enabled".to_string());
    }
    
    // Get ActivityWatch base URL
    let base_url_opt = {
        aw_manager::AW_BASE_URL.lock().unwrap().clone()
    };
    
    let base_url = match base_url_opt {
        Some(url) => url,
        None => {
            return Err("ActivityWatch base URL not available".to_string());
        }
    };
    
    // Check if server is healthy
    let healthy = aw_manager::wait_healthy(&base_url, Duration::from_secs(5)).await;
    if !healthy {
        return Err("ActivityWatch server is not healthy".to_string());
    }
    
    // Get current date (today)
    let today = Utc::now().date_naive();
    let date_str = today.format("%Y-%m-%d").to_string();
    
    // Calculate start and end times for today (00:00:00 to 23:59:59)
    let start_time = match today.and_hms_opt(0, 0, 0)
        .and_then(|dt| dt.and_local_timezone(Utc).single())
    {
        Some(dt) => dt,
        None => {
            return Err(format!("Failed to create start time for {}", date_str));
        }
    };
    
    let end_time = match today.and_hms_opt(23, 59, 59)
        .and_then(|dt| dt.and_local_timezone(Utc).single())
    {
        Some(dt) => dt,
        None => {
            return Err(format!("Failed to create end time for {}", date_str));
        }
    };
    
    let mut results = Vec::new();
    
    // Calculate daily metrics for today
    match aw_client::calculate_daily_metrics(&base_url, today).await {
        Ok(metrics) => {
            if let Err(e) = bridge::collect_daily_metrics(&app, &metrics) {
                results.push(format!("Failed to collect daily metrics: {}", e));
            } else {
                results.push(format!("Collected daily metrics: active={}s, idle={}s, afk={}s", 
                    metrics.total_active_seconds, metrics.total_idle_seconds, metrics.total_afk_seconds));
            }
        }
        Err(e) => {
            results.push(format!("Failed to calculate daily metrics: {}", e));
        }
    }
    
    // Collect app usage for today
    match aw_client::aggregate_app_usage(&base_url, start_time, end_time).await {
        Ok(app_usage) => {
            let end_time_str = end_time.to_rfc3339();
            if let Err(e) = bridge::collect_app_usage(&app, &app_usage, &end_time_str) {
                results.push(format!("Failed to collect app usage: {}", e));
            } else {
                results.push(format!("Collected app usage: {} apps", app_usage.len()));
            }
        }
        Err(e) => {
            results.push(format!("Failed to aggregate app usage: {}", e));
        }
    }
    
    // Flush the batch to ensure data is sent immediately
    if let Ok(Some(_batch)) = batch::flush() {
        results.push("Batch flushed successfully".to_string());
    } else {
        results.push("Failed to flush batch".to_string());
    }
    
    Ok(results.join("; "))
}

/**
 * Stop collector
 * Signals background task to shutdown and waits for completion
 */
#[tauri::command]
pub async fn stop_collector() -> Result<(), String> {
    log::info!("Stopping collector");

    // Set shutdown signal
    SHUTDOWN_SIGNAL.store(true, Ordering::Relaxed);

    // Wait for ActivityWatch polling task to complete
    let aw_handle_opt = {
        let mut aw_handle_guard = AW_POLLING_HANDLE.lock().unwrap();
        aw_handle_guard.take()
    };

    if let Some(aw_handle) = aw_handle_opt {
        let timeout_duration = Duration::from_secs(5);
        match tokio::time::timeout(timeout_duration, aw_handle).await {
            Ok(Ok(())) => {
                log::info!("ActivityWatch polling task stopped cleanly");
            }
            Ok(Err(e)) => {
                log::error!("ActivityWatch polling task panicked: {:?}", e);
            }
            Err(_) => {
                log::warn!("ActivityWatch polling task stop timeout");
            }
        }
    }

    // Wait for daily metrics collection task to complete
    let metrics_handle_opt = {
        let mut metrics_handle_guard = DAILY_METRICS_HANDLE.lock().unwrap();
        metrics_handle_guard.take()
    };

    if let Some(metrics_handle) = metrics_handle_opt {
        let timeout_duration = Duration::from_secs(5);
        match tokio::time::timeout(timeout_duration, metrics_handle).await {
            Ok(Ok(())) => {
                log::info!("Daily metrics collection task stopped cleanly");
            }
            Ok(Err(e)) => {
                log::error!("Daily metrics collection task panicked: {:?}", e);
            }
            Err(_) => {
                log::warn!("Daily metrics collection task stop timeout");
            }
        }
    }

    // Wait for background transmission task to complete (with timeout)
    let handle_opt = {
        let mut bg_handle = BACKGROUND_HANDLE.lock().unwrap();
        bg_handle.take()
    };

    if let Some(handle) = handle_opt {
        let timeout_duration = Duration::from_secs(10);
        match tokio::time::timeout(timeout_duration, handle).await {
            Ok(Ok(())) => {
                log::info!("Background transmission task stopped cleanly");
            }
            Ok(Err(e)) => {
                log::error!("Background transmission task panicked: {:?}", e);
            }
            Err(_) => {
                log::warn!("Background transmission task stop timeout");
            }
        }
    }

    // Clear running flag and reset connection status
    COLLECTOR_RUNNING.store(false, Ordering::Relaxed);
    update_connection_status(types::TransmissionStatus::Disconnected);

    // Shutdown batch manager
    batch::shutdown();

    // Clear config cache
    config::clear_cache();

    log::info!("Collector stopped");
    Ok(())
}

/**
 * Get current collector status and statistics
 * Returns error if collector is not configured (empty user_id/org_id)
 * Returns statistics with current connection status if configured
 */
#[tauri::command]
pub async fn get_collector_status(app: AppHandle) -> Result<types::SyncStatistics, String> {
    // Load config to check if collector has been configured
    let config = config::load_config(&app)?;
    
    // If required fields are empty, collector hasn't been configured
    if config.user_name.is_empty() || config.user_id.is_empty() || 
       config.org_name.is_empty() || config.org_id.is_empty() || 
       config.account_id.is_empty() {
        return Err("Collector not configured".to_string());
    }
    
    // Get connection status from shared global state
    let connection_status = get_connection_status();
    
    // Try to get batch statistics (may fail if collector not initialized)
    let mut stats = match batch::get_statistics() {
        Ok(s) => s,
        Err(_) => {
            // Collector not running, return default stats
            types::SyncStatistics::default()
        }
    };

    // Always use the shared connection status
    stats.connection_status = connection_status;
    Ok(stats)
}

/**
 * Update collector configuration
 * Uses config diffing to avoid unnecessary restarts
 * 
 * Restart required when: enabled, server_url, auth_url, user_id, or org_id change
 * Hot-update allowed for: batch settings, retry settings, keepalive, etc.
 */
#[tauri::command]
pub async fn update_collector_config(
    app: AppHandle,
    mut config: config::CollectorConfig,
) -> Result<(), String> {
    log::info!("Updating collector configuration");

    // Always enforce default values for user/org fields
    config.user_name = "Local".to_string();
    config.user_id = "0".to_string();
    config.org_name = "Local".to_string();
    config.org_id = "0".to_string();
    config.account_id = "0".to_string();

    // Validate configuration
    config.validate()?;

    // Get current cached config and running state
    let current_config = config::get_cached_config();
    let is_running = COLLECTOR_RUNNING.load(Ordering::Relaxed);

    // Check if config actually changed
    if let Some(ref current) = current_config {
        if current == &config {
            log::info!("Configuration unchanged, skipping update");
            return Ok(());
        }
    }

    // Determine if restart is needed
    let needs_restart = match &current_config {
        Some(current) => current.needs_restart(&config),
        None => true, // No cached config means we need to start fresh
    };

    // Save new configuration to disk (always)
    config::save_config(&app, &config)?;

    if !is_running {
        // Not running - just update cache and optionally start
        config::update_cache(config.clone());
        
        if config.enabled {
            log::info!("Collector not running, starting with new configuration");
            start_collector(app, config).await?;
        }
    } else if needs_restart {
        // Running and needs restart - stop, update, potentially restart
        log::info!("Configuration change requires restart");
        stop_collector().await?;
        config::update_cache(config.clone());
        
        if config.enabled {
            log::info!("Restarting collector with new configuration");
            start_collector(app, config).await?;
        }
    } else {
        // Running but can hot-update - just update cache
        log::info!("Hot-updating configuration (no restart needed)");
        
        // Log token state
        if let Some(ref token) = config.app_jwt_token {
            log::info!("[UPDATE_CONFIG] Updating cache with app_jwt_token: length={}", token.len());
        } else {
            log::warn!("[UPDATE_CONFIG] No app_jwt_token in config being cached");
        }
        
        config::update_cache(config);
    }

    log::info!("Configuration updated successfully");
    Ok(())
}

/**
 * Get current collector configuration
 */
#[tauri::command]
pub async fn get_collector_config(app: AppHandle) -> Result<config::CollectorConfig, String> {
    config::load_config(&app)
}

/**
 * Test connection to collector server
 * Creates temporary client, attempts connection and authentication
 */
#[tauri::command]
pub async fn test_collector_connection(
    config: config::CollectorConfig,
) -> Result<String, String> {
    log::info!("[TEST_CONNECTION] Testing collector connection");
    log::info!("[TEST_CONNECTION] Config received - user_id: {}, org_id: {}", config.user_id, config.org_id);
    
    // Log whether app JWT token is provided (optional for mock-auth)
    if let Some(ref token) = config.app_jwt_token {
        log::info!("[TEST_CONNECTION] App JWT token provided: length={}, first_20_chars={}", 
            token.len(), 
            if token.len() >= 20 { &token[..20] } else { token });
    } else {
        log::debug!("[TEST_CONNECTION] No app JWT token in config - using mock-auth (JWT optional)");
    }

    // Validate configuration first
    config.validate()?;

    // Create temporary client
    log::info!("[TEST_CONNECTION] Creating CollectorClient with config");
    let mut test_client = client::CollectorClient::new(config);

    // Attempt connection
    test_client.connect().await?;

    // Attempt authentication
    test_client.authenticate().await?;

    // Disconnect
    test_client.disconnect().await;

    Ok("Connection test successful".to_string())
}

