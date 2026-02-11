/**
 * ============================================================================
 * TRANSCRIPTION QUEUE MODULE
 * ============================================================================
 * 
 * PURPOSE: Background job queue for async transcription processing
 * 
 * FEATURES:
 * - Non-blocking job submission
 * - Sequential processing to manage CPU load
 * - Exponential backoff retry on failures
 * - Persistent queue for crash recovery
 * - Graceful shutdown handling
 * 
 * ARCHITECTURE:
 * - MPSC channel for job submission
 * - Single background task processes jobs sequentially
 * - Jobs persisted to disk for recovery across restarts
 * 
 * ============================================================================
 */

use crate::collector::{batch, config as collector_config};
use crate::recording::transcription::{
    formatter,
    storage,
    types::{QueueStats, TranscriptionConfig, TranscriptionJob, TranscriptionResult},
    whisper,
};
use chrono::Utc;
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
static JOB_SENDER: Lazy<Mutex<Option<mpsc::Sender<TranscriptionJob>>>> =
    Lazy::new(|| Mutex::new(None));

/// Shutdown signal
static SHUTDOWN: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// Current transcription config
static TRANSCRIPTION_CONFIG: Lazy<Mutex<TranscriptionConfig>> =
    Lazy::new(|| Mutex::new(TranscriptionConfig::default()));

/// Queue statistics
static QUEUE_STATS: Lazy<Mutex<QueueStats>> = Lazy::new(|| Mutex::new(QueueStats::default()));

/// App handle for storage paths
static APP_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));

// =============================================================================
// Public Types
// =============================================================================

/**
 * Current status of the transcription queue
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    /// Whether the queue is running
    pub running: bool,
    
    /// Whether Whisper is available
    pub whisper_available: bool,
    
    /// Queue statistics
    pub stats: QueueStats,
    
    /// Current configuration
    pub config: TranscriptionConfig,
}

/// Persisted queue for recovery across restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedQueue {
    jobs: Vec<TranscriptionJob>,
}

// =============================================================================
// Public API
// =============================================================================

/**
 * Initialize the transcription queue
 * 
 * Must be called on app startup after init_whisper().
 * Sets up the background processor and loads any persisted jobs.
 * 
 * # Arguments
 * * `app` - Tauri app handle for storage paths
 * * `config` - Transcription configuration
 */
pub fn init_queue(app: &AppHandle, config: TranscriptionConfig) {
    log::info!(
        "[TRANSCRIPTION-QUEUE] Initializing queue (enabled: {}, whisper available: {})",
        config.enabled,
        whisper::is_available()
    );

    // Store app handle and config
    *APP_HANDLE.lock().unwrap() = Some(app.clone());
    *TRANSCRIPTION_CONFIG.lock().unwrap() = config.clone();

    // Reset shutdown flag
    SHUTDOWN.store(false, Ordering::SeqCst);

    // Create channel
    let (tx, rx) = mpsc::channel::<TranscriptionJob>(100);
    *JOB_SENDER.lock().unwrap() = Some(tx);

    // Load persisted jobs
    let persisted_jobs = load_persisted_queue(app);
    let job_count = persisted_jobs.len();

    // Spawn background processor
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        queue_processor(rx, app_clone, persisted_jobs).await;
    });

    if job_count > 0 {
        log::info!(
            "[TRANSCRIPTION-QUEUE] Loaded {} persisted jobs from previous session",
            job_count
        );
    }

    log::info!("[TRANSCRIPTION-QUEUE] Queue initialized");
}

/**
 * Submit a transcription job
 * 
 * Non-blocking: returns immediately after queuing.
 * Jobs are processed sequentially in the background.
 * 
 * # Arguments
 * * `job` - The transcription job to submit
 * 
 * # Returns
 * * `Ok(())` - Job queued successfully
 * * `Err(String)` - Queue not initialized or full
 */
pub fn submit_job(job: TranscriptionJob) -> Result<(), String> {
    let config = TRANSCRIPTION_CONFIG.lock().unwrap().clone();

    // Check if transcription is enabled
    if !config.enabled {
        log::info!(
            "[TRANSCRIPTION-QUEUE] ⏭ Skipping job (transcription disabled): segment={} display={} source={}",
            job.segment_id,
            job.display_index,
            job.source
        );
        return Ok(());
    }

    // Check if Whisper is available
    if !whisper::is_available() {
        log::warn!(
            "[TRANSCRIPTION-QUEUE] ⚠ Whisper not available, skipping: segment={} display={} source={}",
            job.segment_id,
            job.display_index,
            job.source
        );
        return Ok(());
    }

    let sender = JOB_SENDER.lock().unwrap();
    let tx = sender
        .as_ref()
        .ok_or_else(|| "Transcription queue not initialized".to_string())?;

    tx.try_send(job.clone())
        .map_err(|e| format!("Failed to queue transcription job: {}", e))?;

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
        "[TRANSCRIPTION-QUEUE] ✓ Queued job: segment={} display={} source={}",
        job.segment_id,
        job.display_index,
        job.source
    );

    Ok(())
}

/**
 * Shutdown the queue gracefully
 * 
 * Signals shutdown and waits for current job to complete.
 * Remaining jobs are persisted for next startup.
 */
pub async fn shutdown_queue() {
    log::info!("[TRANSCRIPTION-QUEUE] Shutting down...");

    SHUTDOWN.store(true, Ordering::SeqCst);

    // Drop the sender to close the channel
    *JOB_SENDER.lock().unwrap() = None;

    // Give processor time to finish current job
    sleep(Duration::from_secs(1)).await;

    log::info!("[TRANSCRIPTION-QUEUE] Shutdown complete");
}

/**
 * Get current queue status
 */
pub fn get_queue_status() -> QueueStatus {
    QueueStatus {
        running: JOB_SENDER.lock().unwrap().is_some(),
        whisper_available: whisper::is_available(),
        stats: QUEUE_STATS.lock().unwrap().clone(),
        config: TRANSCRIPTION_CONFIG.lock().unwrap().clone(),
    }
}

/**
 * Update transcription configuration
 */
pub fn update_config(config: TranscriptionConfig) {
    log::info!(
        "[TRANSCRIPTION-QUEUE] Updating config (enabled: {})",
        config.enabled
    );
    *TRANSCRIPTION_CONFIG.lock().unwrap() = config;
}

/**
 * Update transcription configuration from AudioFeatureConfig
 * 
 * Extracts transcription-related settings from the audio feature config.
 */
pub fn update_config_from_audio_feature(audio_config: &crate::recording::types::AudioFeatureConfig) {
    let config = TranscriptionConfig {
        enabled: audio_config.transcription_enabled,
        model: audio_config.transcription_model.clone(),
        max_retries: audio_config.transcription_max_retries,
        retry_delay_seconds: audio_config.transcription_retry_delay_seconds,
        processing_delay_seconds: audio_config.transcription_processing_delay_seconds,
    };
    log::info!(
        "[TRANSCRIPTION-QUEUE] Updating config from AudioFeatureConfig (enabled: {}, model: {})",
        config.enabled,
        config.model
    );
    *TRANSCRIPTION_CONFIG.lock().unwrap() = config;
}

// =============================================================================
// Background Processor
// =============================================================================

/**
 * Main queue processor loop
 * 
 * Processes persisted jobs first, then new jobs from the channel.
 * Jobs are processed sequentially to manage CPU load.
 */
async fn queue_processor(
    mut rx: mpsc::Receiver<TranscriptionJob>,
    app: AppHandle,
    persisted_jobs: Vec<TranscriptionJob>,
) {
    log::info!("[TRANSCRIPTION-QUEUE] Processor started");

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

    log::info!("[TRANSCRIPTION-QUEUE] Processor stopped");
}

/**
 * Process a single transcription job.
 * Uses a loop for retries instead of recursion to prevent stack growth.
 */
async fn process_job(app: &AppHandle, mut job: TranscriptionJob) {
    // Use a loop instead of recursion to prevent stack growth on repeated retries
    loop {
        let config = TRANSCRIPTION_CONFIG.lock().unwrap().clone();

        log::info!(
            "[TRANSCRIPTION-QUEUE] ▶ Processing: segment={} display={} source={} attempt={}/{}",
            job.segment_id,
            job.display_index,
            job.source,
            job.retry_count + 1,
            config.max_retries
        );

        // Add processing delay if configured
        if config.processing_delay_seconds > 0 && job.retry_count == 0 {
            log::info!(
                "[TRANSCRIPTION-QUEUE] Waiting {}s before processing...",
                config.processing_delay_seconds
            );
            sleep(Duration::from_secs(config.processing_delay_seconds)).await;
        }

        // Check if audio file exists
        if !job.audio_path.exists() {
            log::warn!(
                "[TRANSCRIPTION-QUEUE] Audio file not found: {:?}",
                job.audio_path
            );
            mark_job_failed("Audio file not found");
            remove_persisted_job(app, &job);
            return;
        }

        // Run transcription in blocking task (CPU-intensive)
        let audio_path = job.audio_path.clone();
        let start_time = std::time::Instant::now();

        let result = tokio::task::spawn_blocking(move || whisper::transcribe_audio(&audio_path)).await;

        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(raw_result)) => {
                log::info!(
                    "[TRANSCRIPTION-QUEUE] ✓ Transcription complete: {} segments, {} ms processing time",
                    raw_result.segments.len(),
                    processing_time_ms
                );

                // Build full TranscriptionResult
                let transcription = TranscriptionResult {
                    segment_id: job.segment_id.clone(),
                    display_index: job.display_index,
                    source: job.source,
                    speaker_label: job.source.speaker_label().to_string(),
                    model: config.model.clone(),
                    language: "en".to_string(),
                    transcribed_at: Utc::now().to_rfc3339(),
                    audio_duration_ms: raw_result.audio_duration_ms,
                    processing_time_ms,
                    segments: raw_result.segments,
                    full_text: raw_result.full_text,
                };

                // Save transcript
                if let Err(e) = storage::save_transcript(app, &transcription) {
                    log::error!("[TRANSCRIPTION-QUEUE] Failed to save transcript: {}", e);
                    match handle_retry(app, job, &e, &config).await {
                        Some(retry_job) => { job = retry_job; continue; }
                        None => return,
                    }
                }

                // Send to collector (non-blocking, errors logged but don't fail the job)
                // Convert audio_path_local (PathBuf) to string for formatter
                let audio_path_str = job.audio_path_local
                    .as_ref()
                    .and_then(|p| p.to_str())
                    .map(|s| s.to_string());
                
                if let Err(e) = send_to_collector(&transcription, &job.segment_start_time, audio_path_str.as_deref()) {
                    log::error!("[TRANSCRIPTION-QUEUE] Failed to send transcript to collector: {}", e);
                    // Don't retry - transcript is already saved to disk
                }

                // Update stats
                {
                    let mut stats = QUEUE_STATS.lock().unwrap();
                    stats.jobs_completed += 1;
                    stats.jobs_pending = stats.jobs_pending.saturating_sub(1);
                    stats.total_audio_processed_ms += raw_result.audio_duration_ms;
                    stats.total_processing_time_ms += processing_time_ms;
                }

                // Remove from persisted queue
                remove_persisted_job(app, &job);
                
                // Note: Audio files are kept (not deleted) for local storage

                log::info!(
                    "[TRANSCRIPTION-QUEUE] ✓ Saved transcript for segment={} display={} source={}",
                    job.segment_id,
                    job.display_index,
                    job.source
                );
                return;
            }
            Ok(Err(e)) => {
                log::error!("[TRANSCRIPTION-QUEUE] Transcription error: {}", e);
                match handle_retry(app, job, &e, &config).await {
                    Some(retry_job) => { job = retry_job; continue; }
                    None => return,
                }
            }
            Err(e) => {
                log::error!("[TRANSCRIPTION-QUEUE] Task panic: {}", e);
                match handle_retry(app, job, &e.to_string(), &config).await {
                    Some(retry_job) => { job = retry_job; continue; }
                    None => return,
                }
            }
        }
    }
}

/**
 * Handle job retry with exponential backoff
 * Returns Some(job) if a retry should be attempted, None if max retries exceeded.
 * The caller (process_job loop) is responsible for actually retrying.
 */
async fn handle_retry(app: &AppHandle, mut job: TranscriptionJob, error: &str, config: &TranscriptionConfig) -> Option<TranscriptionJob> {
    job.retry_count += 1;

    if job.retry_count >= config.max_retries {
        log::error!(
            "[TRANSCRIPTION-QUEUE] Max retries ({}) exceeded for segment={} display={} source={}",
            config.max_retries,
            job.segment_id,
            job.display_index,
            job.source
        );
        mark_job_failed(error);
        remove_persisted_job(app, &job);
        return None;
    }

    // Calculate backoff delay
    let delay_secs = config.retry_delay_seconds * (2_u64.pow(job.retry_count - 1));

    log::info!(
        "[TRANSCRIPTION-QUEUE] Retrying in {}s (attempt {}/{})",
        delay_secs,
        job.retry_count + 1,
        config.max_retries
    );

    sleep(Duration::from_secs(delay_secs)).await;

    // Return the job for retry via loop instead of recursion
    Some(job)
}

/**
 * Mark a job as failed in stats
 */
fn mark_job_failed(error: &str) {
    let mut stats = QUEUE_STATS.lock().unwrap();
    stats.jobs_failed += 1;
    stats.jobs_pending = stats.jobs_pending.saturating_sub(1);
    stats.last_error = Some(error.to_string());
}

// =============================================================================
// Collector Integration
// =============================================================================

/**
 * Send transcription segments to the collector server
 * 
 * Formats each transcription segment as InfluxDB line protocol and queues
 * for transmission via the collector batch system.
 * 
 * # Arguments
 * * `transcription` - The completed transcription result
 * * `segment_start_time` - ISO 8601 timestamp when the recording segment started
 * * `audio_path` - Optional absolute local path to mixed audio file
 * 
 * # Returns
 * * `Ok(())` - All segments queued successfully (or collector disabled)
 * * `Err(String)` - Error formatting or queuing segments
 */
fn send_to_collector(
    transcription: &TranscriptionResult,
    segment_start_time: &str,
    audio_path: Option<&str>,
) -> Result<(), String> {
    if !collector_config::is_enabled() {
        log::info!("[TRANSCRIPTION-COLLECTOR] Collector disabled, transcript data NOT sent");
        return Ok(());
    }

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    log::info!(
        "[TRANSCRIPTION-COLLECTOR] Sending {} transcript segments to collector (segment={} speaker={})",
        transcription.segments.len(),
        transcription.segment_id,
        transcription.speaker_label
    );

    // Convert transcription segments to line protocol and queue
    for (i, segment) in transcription.segments.iter().enumerate() {
        let line_protocol = formatter::format_transcription_segment(
            segment,
            segment_start_time,
            &transcription.speaker_label,
            &hostname,
            audio_path,
        )?;

        log::info!(
            "[TRANSCRIPTION-COLLECTOR] LineProtocol[{}]: {}",
            i + 1,
            &line_protocol[..line_protocol.len().min(200)]
        );

        batch::add_event(line_protocol)?;
    }

    log::info!(
        "[TRANSCRIPTION-COLLECTOR] ✓ Queued {} transcript segments for transmission",
        transcription.segments.len()
    );

    Ok(())
}

// =============================================================================
// Persistence
// =============================================================================

/**
 * Get path to persisted queue file
 */
fn get_queue_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|e| {
            log::error!("Failed to resolve app_data_dir for transcription queue: {}", e);
            std::env::temp_dir().join("screenjournal")
        })
        .join("transcription_queue.json")
}

/**
 * Add a job to the persisted queue
 */
fn persist_job(app: &AppHandle, job: &TranscriptionJob) {
    let path = get_queue_path(app);

    // Load existing queue
    let mut queue = load_persisted_queue_raw(app);

    // Add job if not already present
    if !queue.jobs.iter().any(|j| {
        j.segment_id == job.segment_id
            && j.display_index == job.display_index
            && j.source == job.source
    }) {
        queue.jobs.push(job.clone());
    }

    // Save
    if let Ok(json) = serde_json::to_string_pretty(&queue) {
        if let Err(e) = std::fs::write(&path, json) {
            log::warn!("[TRANSCRIPTION-QUEUE] Failed to persist queue: {}", e);
        }
    }
}

/**
 * Remove a job from the persisted queue
 */
fn remove_persisted_job(app: &AppHandle, job: &TranscriptionJob) {
    let path = get_queue_path(app);

    // Load existing queue
    let mut queue = load_persisted_queue_raw(app);

    // Remove job
    queue.jobs.retain(|j| {
        !(j.segment_id == job.segment_id
            && j.display_index == job.display_index
            && j.source == job.source)
    });

    // Save
    if let Ok(json) = serde_json::to_string_pretty(&queue) {
        if let Err(e) = std::fs::write(&path, json) {
            log::warn!("[TRANSCRIPTION-QUEUE] Failed to update queue: {}", e);
        }
    }
}

/**
 * Load persisted jobs
 */
fn load_persisted_queue(app: &AppHandle) -> Vec<TranscriptionJob> {
    load_persisted_queue_raw(app).jobs
}

/**
 * Load raw persisted queue structure
 */
fn load_persisted_queue_raw(app: &AppHandle) -> PersistedQueue {
    let path = get_queue_path(app);

    if !path.exists() {
        return PersistedQueue { jobs: Vec::new() };
    }

    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
            log::warn!("[TRANSCRIPTION-QUEUE] Failed to parse persisted queue JSON, starting fresh: {}", e);
            PersistedQueue { jobs: Vec::new() }
        }),
        Err(e) => {
            log::warn!("[TRANSCRIPTION-QUEUE] Failed to read persisted queue file: {}", e);
            PersistedQueue { jobs: Vec::new() }
        },
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
