/**
 * ============================================================================
 * COLLECTOR BRIDGE MODULE
 * ============================================================================
 * 
 * PURPOSE: Bridge between ActivityWatch data and collector transmission
 * 
 * FUNCTIONALITY:
 * - Convert ActivityWatch events to line protocol
 * - Send formatted events to batch manager
 * - Handle hostname resolution
 * - Check if collector is enabled before transmission
 * 
 * ============================================================================
 */

use crate::collector::{batch, config, formatter};
use crate::activitywatch::types::{AppUsage, DailyMetrics, EventInfo};

/**
 * Get system hostname for tagging
 * Returns hostname or "unknown" if unavailable
 */
fn get_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

/**
 * Send window activity event to collector
 * Formats event as line protocol and adds to batch
 */
pub fn collect_window_event(
    _app_handle: &tauri::AppHandle,
    event: &EventInfo,
) -> Result<(), String> {
    if !config::is_enabled() {
        return Ok(()); // Silently skip if disabled
    }

    let hostname = get_hostname();

    // Extract app and title from event data
    // Use "unknown" if app is missing or empty (InfluxDB 2.0 doesn't allow empty tag values)
    let app = event.data.get("app")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown");
    
    let title = event.data.get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Format to line protocol
    let line_protocol = formatter::format_window_event(
        &event.timestamp,
        event.duration,
        app,
        title,
        &hostname,
    )?;

    // Add to batch manager
    batch::add_event(line_protocol)?;

    Ok(())
}

/**
 * Send AFK status event to collector
 * Formats event as line protocol and adds to batch
 * Also triggers flush if user went AFK and flush_on_afk is enabled
 */
pub fn collect_afk_event(
    _app_handle: &tauri::AppHandle,
    event: &EventInfo,
) -> Result<(), String> {
    if !config::is_enabled() {
        return Ok(());
    }

    let hostname = get_hostname();

    // Extract status from event data
    let status = event.data.get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Format to line protocol
    let line_protocol = formatter::format_afk_event(
        &event.timestamp,
        event.duration,
        status,
        &hostname,
    )?;

    // Add to batch manager
    batch::add_event(line_protocol)?;

    // Check if we should flush on AFK
    if (status == "afk" || status == "idle") && config::should_flush_on_afk() {
        log::info!("User went AFK, flushing batch (flush_on_afk enabled)");
        let _ = batch::flush(); // Best effort flush
    }

    Ok(())
}

/**
 * Send daily metrics to collector
 * Formats metrics as line protocol and adds to batch
 */
pub fn collect_daily_metrics(
    _app_handle: &tauri::AppHandle,
    metrics: &DailyMetrics,
) -> Result<(), String> {
    if !config::is_enabled() {
        return Ok(());
    }

    let hostname = get_hostname();

    // Format to line protocol
    let line_protocol = formatter::format_daily_metrics(
        &metrics.date,
        metrics.total_active_seconds,
        metrics.total_idle_seconds,
        metrics.total_afk_seconds,
        metrics.utilization_ratio,
        metrics.app_switches,
        &hostname,
    )?;

    // Add to batch manager
    batch::add_event(line_protocol)?;

    Ok(())
}

/**
 * Send app usage statistics to collector
 * Formats each app's usage as line protocol and adds to batch
 */
pub fn collect_app_usage(
    _app_handle: &tauri::AppHandle,
    app_usage: &[AppUsage],
    timestamp: &str,
) -> Result<(), String> {
    if !config::is_enabled() {
        return Ok(());
    }

    let hostname = get_hostname();

    for app in app_usage {
        // Use "unknown" if app_name is empty (InfluxDB 2.0 doesn't allow empty tag values)
        let app_name = if app.app_name.is_empty() {
            "unknown"
        } else {
            &app.app_name
        };
        
        // Format to line protocol
        let line_protocol = formatter::format_app_usage(
            timestamp,
            app_name,
            app.total_seconds,
            app.event_count,
            app.category.as_deref(),
            &hostname,
        )?;

        // Add to batch manager
        batch::add_event(line_protocol)?;
    }

    Ok(())
}

/**
 * Trigger manual flush when user goes AFK
 * Used to ensure data is sent before potential system sleep
 */
pub fn flush_on_afk() -> Result<(), String> {
    if let Ok(Some(_batch)) = batch::flush() {
        log::info!("Flushed batch on AFK trigger");
        // Note: The batch is created but will be picked up by the background task
    }
    Ok(())
}

