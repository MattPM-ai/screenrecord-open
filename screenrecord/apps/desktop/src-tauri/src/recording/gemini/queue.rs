/**
 * ============================================================================
 * GEMINI QUEUE MODULE
 * ============================================================================
 * 
 * PURPOSE: Async job queue for Gemini video analysis
 * 
 * FEATURES:
 * - Background processing (non-blocking)
 * - Exponential backoff retry on failures
 * - Persistent queue for crash recovery
 * - Graceful shutdown handling
 * 
 * ARCHITECTURE:
 * - MPSC channel for job submission
 * - Single background task processes jobs sequentially
 * - Jobs persisted to disk for recovery
 * 
 * ============================================================================
 */

use crate::collector::{batch, config as collector_config};
use crate::recording::gemini::{
    client,
    formatter,
    types::{GeminiConfig, GeminiError, GeminiJob},
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

// =============================================================================
// Global State
// =============================================================================

/// Job submission channel
static JOB_SENDER: Lazy<Mutex<Option<mpsc::Sender<GeminiJob>>>> = Lazy::new(|| Mutex::new(None));

/// Shutdown signal
static SHUTDOWN: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// Current Gemini config
static GEMINI_CONFIG: Lazy<Mutex<GeminiConfig>> = Lazy::new(|| Mutex::new(GeminiConfig::default()));

/// Queue statistics
static QUEUE_STATS: Lazy<Mutex<QueueStats>> = Lazy::new(|| Mutex::new(QueueStats::default()));

/// App handle for storage paths
static APP_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));

// =============================================================================
// Types
// =============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueStats {
    pub jobs_submitted: u64,
    pub jobs_completed: u64,
    pub jobs_failed: u64,
    pub jobs_pending: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub running: bool,
    pub stats: QueueStats,
    pub config: GeminiConfig,
}

/// Persisted queue for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedQueue {
    jobs: Vec<GeminiJob>,
}

// =============================================================================
// Public API
// =============================================================================

/**
 * Initialize the Gemini processing queue
 * Must be called on app startup before submitting jobs
 */
pub fn init_queue(app: &AppHandle, config: GeminiConfig) {
    log::info!("Initializing Gemini queue (enabled: {})", config.enabled);

    // Store app handle and config
    *APP_HANDLE.lock().unwrap() = Some(app.clone());
    *GEMINI_CONFIG.lock().unwrap() = config.clone();

    // Reset shutdown flag
    SHUTDOWN.store(false, Ordering::SeqCst);

    // Create channel
    let (tx, rx) = mpsc::channel::<GeminiJob>(100);
    *JOB_SENDER.lock().unwrap() = Some(tx);

    // Load persisted jobs
    let persisted_jobs = load_persisted_queue(app);
    let job_count = persisted_jobs.len();

    // Spawn background processor using Tauri's async runtime
    // (required because init_queue is called from synchronous setup)
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        queue_processor(rx, app_clone, persisted_jobs).await;
    });

    if job_count > 0 {
        log::info!("Loaded {} persisted Gemini jobs from previous session", job_count);
    }

    log::info!("Gemini queue initialized");
}

/**
 * Submit a job for Gemini analysis
 * Non-blocking: returns immediately after queuing
 */
pub fn submit_job(job: GeminiJob) -> Result<(), String> {
    let config = GEMINI_CONFIG.lock().unwrap().clone();
    
    if !config.enabled {
        log::info!(
            "[GEMINI-QUEUE] ⏭ Skipping job (Gemini disabled): segment={} display={}",
            job.segment_id,
            job.display_index
        );
        return Ok(());
    }

    // Check if API key exists (check user-provided key first if AppHandle is available)
    let has_key = if let Some(app_handle) = APP_HANDLE.lock().unwrap().as_ref() {
        crate::recording::gemini::has_api_key_with_app(app_handle)
    } else {
        crate::recording::gemini::has_api_key()
    };
    
    if !has_key {
        log::warn!(
            "[GEMINI-QUEUE] ⚠ No API key configured, skipping: segment={} display={}",
            job.segment_id,
            job.display_index
        );
        return Ok(());
    }

    let sender = JOB_SENDER.lock().unwrap();
    let tx = sender.as_ref().ok_or_else(|| "Gemini queue not initialized".to_string())?;

    tx.try_send(job.clone())
        .map_err(|e| format!("Failed to queue Gemini job: {}", e))?;

    // Update stats
    {
        let mut stats = QUEUE_STATS.lock().unwrap();
        stats.jobs_submitted += 1;
        stats.jobs_pending += 1;
    }

    // Persist queue
    if let Some(app) = APP_HANDLE.lock().unwrap().as_ref() {
        persist_job(app, &job);
    }

    log::info!(
        "Queued Gemini analysis job for segment {} display {}",
        job.segment_id,
        job.display_index
    );

    Ok(())
}

/**
 * Shutdown the queue gracefully
 * Waits for current job to complete, persists remaining jobs
 */
pub async fn shutdown_queue() {
    log::info!("Shutting down Gemini queue...");
    
    SHUTDOWN.store(true, Ordering::SeqCst);
    
    // Drop the sender to close the channel
    *JOB_SENDER.lock().unwrap() = None;
    
    // Give processor time to finish current job
    sleep(Duration::from_secs(1)).await;
    
    log::info!("Gemini queue shutdown complete");
}

/**
 * Get current queue status
 */
pub fn get_queue_status() -> QueueStatus {
    QueueStatus {
        running: JOB_SENDER.lock().unwrap().is_some(),
        stats: QUEUE_STATS.lock().unwrap().clone(),
        config: GEMINI_CONFIG.lock().unwrap().clone(),
    }
}

/**
 * Update Gemini configuration
 */
pub fn update_config(config: GeminiConfig) {
    log::info!("Updating Gemini config (enabled: {})", config.enabled);
    *GEMINI_CONFIG.lock().unwrap() = config;
}

// =============================================================================
// Background Processor
// =============================================================================

async fn queue_processor(
    mut rx: mpsc::Receiver<GeminiJob>,
    app: AppHandle,
    persisted_jobs: Vec<GeminiJob>,
) {
    log::info!("Gemini queue processor started");

    // Process persisted jobs first
    for job in persisted_jobs {
        if SHUTDOWN.load(Ordering::SeqCst) {
            break;
        }
        process_job(&app, job).await;
    }

    // Process new jobs from channel
    while let Some(job) = rx.recv().await {
        if SHUTDOWN.load(Ordering::SeqCst) {
            // Persist remaining job for next startup
            persist_job(&app, &job);
            break;
        }
        
        process_job(&app, job).await;
    }

    log::info!("Gemini queue processor stopped");
}

async fn process_job(app: &AppHandle, mut job: GeminiJob) {
    let config = GEMINI_CONFIG.lock().unwrap().clone();
    
    log::info!(
        "[GEMINI-QUEUE] ▶ Processing job: segment={} display={} attempt={}/{} (rate_limit_waits={})",
        job.segment_id,
        job.display_index,
        job.retry_count + 1,
        config.max_retries,
        job.rate_limit_waits
    );
    log::info!(
        "[GEMINI-QUEUE]   Video: {:?} ({:.1}s duration)",
        job.video_path,
        job.metadata.duration_seconds
    );

    // Check if video file still exists
    if !job.video_path.exists() {
        log::warn!(
            "Video file not found for segment {} display {}: {:?}",
            job.segment_id,
            job.display_index,
            job.video_path
        );
        mark_job_failed("Video file not found");
        remove_persisted_job(app, &job);
        return;
    }

    // Call Gemini API (pass app handle to check user-provided API key)
    let result = client::analyze_video(
        &job.video_path,
        &job.segment_id,
        job.display_index,
        job.metadata.duration_seconds,
        &job.metadata.start_time,
        &config,
        Some(app),
    )
    .await;

    match result {
        Ok(analysis) => {
            log::info!(
                "Gemini analysis successful for segment {} display {}: {} entries",
                job.segment_id,
                job.display_index,
                analysis.timeline.len()
            );

            // Send to collector
            if let Err(e) = send_to_collector(&analysis) {
                log::error!("Failed to send timeline to collector: {}", e);
            }

            // Mark complete
            {
                let mut stats = QUEUE_STATS.lock().unwrap();
                stats.jobs_completed += 1;
                stats.jobs_pending = stats.jobs_pending.saturating_sub(1);
            }

            // Remove from persisted queue
            remove_persisted_job(app, &job);
        }
        Err(e) => {
            log::error!(
                "Gemini analysis failed for segment {} display {}: {}",
                job.segment_id,
                job.display_index,
                e
            );

            // Handle error based on type
            match &e {
                GeminiError::RateLimited { retry_after, .. } => {
                    // Rate limits don't count against max_retries
                    job.rate_limit_waits += 1;
                    
                    if job.rate_limit_waits >= config.rate_limit_max_waits {
                        log::error!(
                            "Max rate limit waits ({}) exceeded for segment {} display {}",
                            config.rate_limit_max_waits,
                            job.segment_id,
                            job.display_index
                        );
                        mark_job_failed(&e.to_string());
                        remove_persisted_job(app, &job);
                        return;
                    }

                    // Use API-provided delay or fallback to config
                    let delay = retry_after.unwrap_or_else(|| {
                        Duration::from_secs(config.retry_delay_seconds * (2_u64.pow(job.rate_limit_waits - 1)))
                    });
                    
                    log::info!(
                        "[GEMINI-QUEUE] Rate limited. Waiting {:.1}s (API-specified delay). Rate limit wait {}/{}",
                        delay.as_secs_f64(),
                        job.rate_limit_waits,
                        config.rate_limit_max_waits
                    );
                    
                    sleep(delay).await;
                    
                    // Re-process (retry_count NOT incremented)
                    Box::pin(process_job(app, job)).await;
                }
                GeminiError::ServiceUnavailable { retry_after, .. } => {
                    // Service unavailable counts against retries
                    job.retry_count += 1;

                    if job.retry_count >= config.max_retries {
                        log::error!(
                            "Max retries exceeded for segment {} display {}",
                            job.segment_id,
                            job.display_index
                        );
                        mark_job_failed(&e.to_string());
                        remove_persisted_job(app, &job);
                        return;
                    }

                    // Use API-provided delay or exponential backoff
                    let delay = retry_after.unwrap_or_else(|| {
                        Duration::from_secs(config.retry_delay_seconds * (2_u64.pow(job.retry_count - 1)))
                    });
                    
                    log::info!(
                        "[GEMINI-QUEUE] Service unavailable. Retrying in {:.1}s (attempt {}/{})",
                        delay.as_secs_f64(),
                        job.retry_count + 1,
                        config.max_retries
                    );
                    
                    sleep(delay).await;
                    
                    // Re-process
                    Box::pin(process_job(app, job)).await;
                }
                GeminiError::Permanent { .. } => {
                    // Permanent errors - no retry
                    log::error!(
                        "[GEMINI-QUEUE] Permanent error for segment {} display {}: {}",
                        job.segment_id,
                        job.display_index,
                        e
                    );
                    log::error!(
                        "[GEMINI-QUEUE] Screen timeline will not retry. Check Settings: Gemini API key and model availability (e.g. model not found or invalid key)."
                    );
                    mark_job_failed(&e.to_string());
                    remove_persisted_job(app, &job);
                }
            }
        }
    }
}

fn mark_job_failed(error: &str) {
    let mut stats = QUEUE_STATS.lock().unwrap();
    stats.jobs_failed += 1;
    stats.jobs_pending = stats.jobs_pending.saturating_sub(1);
    stats.last_error = Some(error.to_string());
}

// =============================================================================
// Collector Integration
// =============================================================================

fn send_to_collector(analysis: &crate::recording::gemini::types::TimelineAnalysis) -> Result<(), String> {
    if !collector_config::is_enabled() {
        log::info!("[GEMINI-COLLECTOR] Collector disabled, timeline data NOT sent");
        return Ok(());
    }

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    log::info!(
        "[GEMINI-COLLECTOR] Sending {} timeline entries to collector (segment={} display={})",
        analysis.timeline.len(),
        analysis.segment_id,
        analysis.display_index
    );

    // Convert timeline entries to line protocol and queue
    for (i, entry) in analysis.timeline.iter().enumerate() {
        let line_protocol = formatter::format_timeline_entry(
            analysis,
            entry,
            &hostname,
        )?;
        
        log::info!(
            "[GEMINI-COLLECTOR] LineProtocol[{}]: {}",
            i + 1,
            &line_protocol[..line_protocol.len().min(200)]
        );
        
        batch::add_event(line_protocol)?;
    }

    log::info!(
        "[GEMINI-COLLECTOR] ✓ Queued {} timeline events for transmission",
        analysis.timeline.len()
    );

    Ok(())
}

// =============================================================================
// Persistence
// =============================================================================

fn get_queue_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app_data_dir available")
        .join("gemini_queue.json")
}

fn persist_job(app: &AppHandle, job: &GeminiJob) {
    let path = get_queue_path(app);
    
    // Load existing queue
    let mut queue = load_persisted_queue_raw(app);
    
    // Add job if not already present
    if !queue.jobs.iter().any(|j| j.segment_id == job.segment_id && j.display_index == job.display_index) {
        queue.jobs.push(job.clone());
    }
    
    // Save
    if let Ok(json) = serde_json::to_string_pretty(&queue) {
        if let Err(e) = std::fs::write(&path, json) {
            log::warn!("Failed to persist Gemini queue: {}", e);
        }
    }
}

fn remove_persisted_job(app: &AppHandle, job: &GeminiJob) {
    let path = get_queue_path(app);
    
    // Load existing queue
    let mut queue = load_persisted_queue_raw(app);
    
    // Remove job
    queue.jobs.retain(|j| !(j.segment_id == job.segment_id && j.display_index == job.display_index));
    
    // Save
    if let Ok(json) = serde_json::to_string_pretty(&queue) {
        if let Err(e) = std::fs::write(&path, json) {
            log::warn!("Failed to update Gemini queue: {}", e);
        }
    }
}

fn load_persisted_queue(app: &AppHandle) -> Vec<GeminiJob> {
    load_persisted_queue_raw(app).jobs
}

fn load_persisted_queue_raw(app: &AppHandle) -> PersistedQueue {
    let path = get_queue_path(app);
    
    if !path.exists() {
        return PersistedQueue { jobs: Vec::new() };
    }
    
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or(PersistedQueue { jobs: Vec::new() }),
        Err(_) => PersistedQueue { jobs: Vec::new() },
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_stats_default() {
        let stats = QueueStats::default();
        assert_eq!(stats.jobs_submitted, 0);
        assert_eq!(stats.jobs_completed, 0);
        assert_eq!(stats.jobs_failed, 0);
        assert_eq!(stats.jobs_pending, 0);
        assert!(stats.last_error.is_none());
    }
}

