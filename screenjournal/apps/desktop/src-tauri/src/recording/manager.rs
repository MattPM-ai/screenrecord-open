/**
 * ============================================================================
 * RECORDING MANAGER MODULE
 * ============================================================================
 * 
 * PURPOSE: Lifecycle management and Tauri commands for multi-display MP4 recording
 * 
 * RESPONSIBILITIES:
 * - Start/stop screen recording on all displays simultaneously
 * - Manage recording segments with automatic rotation (60s default)
 * - Provide Tauri commands for frontend control
 * - Handle configuration persistence
 * 
 * RECORDING FLOW:
 * 1. Start recording -> spawn capture thread per display
 * 2. Each capture thread pipes frames to its own FFmpeg for H.264 MP4 encoding
 * 3. On segment duration reached -> finalize all, start new segment
 * 4. Save combined metadata JSON sidecar for the segment
 * 
 * ============================================================================
 */

use crate::recording::{capture, config, gemini, storage, transcription, types::*, upload};
use crate::recording::capture::{AudioCaptureRole, SharedAudioPaths};
use chrono::Utc;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;

// Global recording configuration
static RECORDING_CONFIG: Lazy<Mutex<RecordingConfig>> =
    Lazy::new(|| Mutex::new(RecordingConfig::default()));

// Global audio feature configuration
static AUDIO_FEATURE_CONFIG: Lazy<Mutex<AudioFeatureConfig>> =
    Lazy::new(|| Mutex::new(AudioFeatureConfig::default()));

// Global recording state
static RECORDING_STATE: Lazy<Mutex<RecordingStateHolder>> =
    Lazy::new(|| Mutex::new(RecordingStateHolder::Idle));

// Global statistics
static RECORDING_STATS: Lazy<Mutex<RecordingStats>> =
    Lazy::new(|| Mutex::new(RecordingStats::default()));

// Shutdown signal for recording threads
static SHUTDOWN_SIGNAL: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

// Generation counter for rotation loops - incremented on each start to invalidate old loops
static ROTATION_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Default)]
struct RecordingStats {
    total_segments: u64,
}

// Capture thread handle with display index
struct CaptureThread {
    display_index: u32,
    handle: std::thread::JoinHandle<Result<capture::CaptureResult, String>>,
}

// Recording state holder
enum RecordingStateHolder {
    Idle,
    Recording {
        segment_id: String,
        start_time: chrono::DateTime<Utc>,
        capture_threads: Vec<CaptureThread>,
        display_count: u32,
    },
}

impl std::fmt::Debug for RecordingStateHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordingStateHolder::Idle => write!(f, "Idle"),
            RecordingStateHolder::Recording { segment_id, display_count, .. } => {
                write!(f, "Recording({}, {} displays)", segment_id, display_count)
            }
        }
    }
}

// Initialize configuration on app startup
pub fn init_config(config: RecordingConfig) {
    *RECORDING_CONFIG.lock().unwrap() = config;
}

// Initialize audio feature configuration on app startup
pub fn init_audio_config(app: &AppHandle) -> Result<(), String> {
    let audio_config = config::load_audio_feature_config(app)?;
    *AUDIO_FEATURE_CONFIG.lock().unwrap() = audio_config;
    
    // Initialize transcription queue with loaded config
    let config_clone = AUDIO_FEATURE_CONFIG.lock().unwrap().clone();
    transcription::update_config_from_audio_feature(&config_clone);
    
    Ok(())
}

// Start screen recording
#[tauri::command]
pub async fn start_recording(app: AppHandle) -> Result<(), String> {
    let config = RECORDING_CONFIG.lock().unwrap().clone();
    
    if !config.enabled {
        return Err("Screen recording is disabled in config".to_string());
    }
    
    // Check if already recording
    {
        let state = RECORDING_STATE.lock().unwrap();
        if matches!(*state, RecordingStateHolder::Recording { .. }) {
            return Err("Already recording".to_string());
        }
    }
    
    log::info!("Starting multi-display screen recording system");
    
    // Reset shutdown signal
    SHUTDOWN_SIGNAL.store(false, Ordering::SeqCst);
    
    // Increment generation to invalidate any old rotation loops
    let generation = ROTATION_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    
    // Start first segment
    start_new_segment(&app, &config)?;
    
    // Spawn segment rotation task with current generation
    let app_clone = app.clone();
    let segment_duration = Duration::from_secs(config.segment_duration_seconds);
    
    tokio::spawn(async move {
        segment_rotation_loop(app_clone, segment_duration, generation).await;
    });
    
    log::info!("Screen recording started (generation {})", generation);
    Ok(())
}

// Stop screen recording
#[tauri::command]
pub async fn stop_recording(app: AppHandle) -> Result<(), String> {
    log::info!("Stopping screen recording system");
    
    // Signal shutdown
    SHUTDOWN_SIGNAL.store(true, Ordering::SeqCst);
    
    // Finalize current segment
    finalize_current_segment(&app)?;
    
    // Set state to idle
    {
        let mut state = RECORDING_STATE.lock().unwrap();
        *state = RecordingStateHolder::Idle;
    }
    
    log::info!("Screen recording stopped");
    Ok(())
}

// Start a new recording segment for all displays
fn start_new_segment(app: &AppHandle, config: &RecordingConfig) -> Result<(), String> {
    log::info!("Starting new multi-display recording segment");
    
    // Get all display count
    let display_count = capture::get_display_count();
    if display_count == 0 {
        return Err("No displays available for capture".to_string());
    }
    
    log::info!("Found {} display(s) to record", display_count);
    
    // Generate segment ID and ensure directory exists
    let segment_id = storage::generate_segment_id();
    let date = Utc::now().date_naive();
    storage::ensure_recording_dir(app, &date)?;
    
    // Spawn capture thread for each display
    let shutdown = SHUTDOWN_SIGNAL.clone();
    let fps = config.framerate;
    let output_width = config.output_width;
    let crf = config.crf;
    let preset = config.preset.clone();
    // Safety timeout: segment duration + 2 minutes buffer
    // Primary timing is controlled by rotation loop's shutdown signal
    let safety_timeout = Duration::from_secs(config.segment_duration_seconds + 120);
    
    // Audio capture setup: Primary display (0) captures audio, others use shared audio
    let primary_display_index: u32 = 0;
    let audio_ready_signal = Arc::new(AtomicBool::new(false));
    let audio_failed_signal = Arc::new(AtomicBool::new(false));
    
    // Paths for shared audio files (from primary display)
    // These match the format expected by transcription::storage::get_audio_path
    let shared_audio_paths = SharedAudioPaths {
        system_audio_path: transcription::storage::get_audio_path(
            app, &date, &segment_id, primary_display_index, transcription::AudioSource::SystemAudio
        ),
        mic_audio_path: transcription::storage::get_audio_path(
            app, &date, &segment_id, primary_display_index, transcription::AudioSource::Microphone
        ),
    };
    
    // Get audio feature config
    let audio_config = AUDIO_FEATURE_CONFIG.lock().unwrap().clone();
    let capture_audio = audio_config.enabled;
    
    let mut capture_threads = Vec::new();
    
    for display_idx in 0..display_count {
        let output_path = storage::get_video_path(app, &date, &segment_id, display_idx as u32);
        log::info!("Display {}: Output path {:?}", display_idx, output_path);
        
        let shutdown_clone = shutdown.clone();
        let display_index = display_idx as u32;
        let preset_clone = preset.clone();
        
        // Determine audio role for this display
        let audio_role: AudioCaptureRole = if display_index == primary_display_index {
            AudioCaptureRole::Primary {
                audio_ready_signal: audio_ready_signal.clone(),
                audio_failed_signal: audio_failed_signal.clone(),
            }
        } else {
            AudioCaptureRole::Secondary {
                shared_audio: shared_audio_paths.clone(),
                audio_ready_signal: audio_ready_signal.clone(),
                audio_failed_signal: audio_failed_signal.clone(),
            }
        };
        
        let handle = std::thread::spawn(move || {
            capture::capture_display_to_file(
                display_index,
                fps,
                output_width,
                crf,
                &preset_clone,
                &output_path,
                safety_timeout,
                shutdown_clone,
                audio_role,
                capture_audio,
            )
        });
        
        capture_threads.push(CaptureThread {
            display_index: display_index,
            handle,
        });
    }
    
    // Update state
    {
        let mut state = RECORDING_STATE.lock().unwrap();
        *state = RecordingStateHolder::Recording {
            segment_id,
            start_time: Utc::now(),
            capture_threads,
            display_count: display_count as u32,
        };
    }
    
    Ok(())
}

// Finalize the current recording segment
fn finalize_current_segment(app: &AppHandle) -> Result<Option<RecordingMetadata>, String> {
    log::info!("Finalizing current segment");
    
    let (segment_id, start_time, capture_threads, display_count) = {
        let mut state = RECORDING_STATE.lock().unwrap();
        
        match std::mem::replace(&mut *state, RecordingStateHolder::Idle) {
            RecordingStateHolder::Recording {
                segment_id,
                start_time,
                capture_threads,
                display_count,
            } => (segment_id, start_time, capture_threads, display_count),
            RecordingStateHolder::Idle => {
                return Ok(None);
            }
        }
    };
    
    // Wait for all capture threads to finish and collect results
    let mut display_recordings = Vec::new();
    let mut total_file_size: u64 = 0;
    let date = start_time.date_naive();
    
    for ct in capture_threads {
        match ct.handle.join() {
            Ok(Ok(result)) => {
                log::info!(
                    "Display {}: Capture finished - {}x{}, {} frames, {} bytes",
                    result.display_index, result.width, result.height, 
                    result.frame_count, result.file_size
                );
                
                let filename = format!("{}_d{}.mp4", segment_id, result.display_index);
                total_file_size += result.file_size;
                
                display_recordings.push(DisplayRecording {
                    display_index: result.display_index,
                    width: result.width,
                    height: result.height,
                    frame_count: result.frame_count,
                    file_size_bytes: result.file_size,
                    filename,
                });
            }
            Ok(Err(e)) => {
                log::error!("Display {}: Capture thread error: {}", ct.display_index, e);
            }
            Err(_) => {
                log::error!("Display {}: Capture thread panicked", ct.display_index);
            }
        }
    }
    
    // Only create metadata if we got at least one successful capture
    if display_recordings.is_empty() {
        log::warn!("No successful captures for segment {}", segment_id);
        return Ok(None);
    }
    
    let end_time = Utc::now();
    let duration_seconds = (end_time - start_time).num_milliseconds() as f64 / 1000.0;
    // Clone config immediately to release lock - prevents deadlock
    let config = RECORDING_CONFIG.lock().unwrap().clone();
    
    let metadata = RecordingMetadata {
        id: segment_id.clone(),
        format: "mp4".to_string(),
        codec: "h264".to_string(),
        framerate: config.framerate,
        start_time: start_time.to_rfc3339(),
        end_time: end_time.to_rfc3339(),
        duration_seconds,
        total_file_size_bytes: total_file_size,
        display_count,
        displays: display_recordings,
    };
    
    // Save metadata JSON
    storage::save_metadata(app, &date, &metadata)?;
    
    // Update statistics
    {
        let mut stats = RECORDING_STATS.lock().unwrap();
        stats.total_segments += 1;
    }
    
    log::info!(
        "Segment finalized: {} ({:.1}s, {} displays, {} bytes total)",
        segment_id,
        duration_seconds,
        display_count,
        total_file_size
    );
    
    // Run cleanup - reuse already cloned config
    storage::cleanup_old_recordings(app, &config).ok();
    storage::cleanup_by_quota(app, &config).ok();
    
    // Submit Gemini analysis jobs for each display
    for display in &metadata.displays {
        let video_path = storage::get_video_path(app, &date, &metadata.id, display.display_index);
        
        let job = gemini::GeminiJob {
            segment_id: metadata.id.clone(),
            display_index: display.display_index,
            video_path,
            metadata: metadata.clone(),
            retry_count: 0,
            rate_limit_waits: 0,
            created_at: chrono::Utc::now(),
        };
        
        if let Err(e) = gemini::submit_job(job) {
            log::warn!("Failed to queue Gemini analysis job: {}", e);
        }
    }
    
    // Audio mixing and transcription (if audio files exist)
    // Note: Audio capture integration into capture.rs is still pending
    // This code will work once audio files are created by the capture module
    let primary_display_index: u32 = 0;
    
    // Get audio file paths (these may not exist yet if audio capture isn't integrated)
    let mic_path = transcription::storage::get_audio_path(
        app, &date, &metadata.id, primary_display_index, transcription::AudioSource::Microphone
    );
    let system_audio_path = transcription::storage::get_audio_path(
        app, &date, &metadata.id, primary_display_index, transcription::AudioSource::SystemAudio
    );
    
    // Mix and save audio locally (if audio files exist)
    let upload_config = upload::UploadConfig::default();
    let audio_output_path = storage::get_audio_path(app, &date, &metadata.id);
    
    // Ensure audio directory exists
    storage::ensure_audio_dir(app, &date).ok();
    
    let audio_path_result = upload::mix_and_save_audio(
        &metadata.id,
        &system_audio_path,
        if mic_path.exists() { Some(mic_path.as_path()) } else { None },
        &audio_output_path,
        &upload_config,
    );
    
    // Submit transcription jobs if audio files exist and transcription is enabled
    // Note: This requires AudioFeatureConfig to be added to manager state
    // For now, we'll check if transcription queue is initialized
    let transcription_enabled = {
        // Check if transcription queue is running (indicates it's enabled)
        let status = transcription::get_queue_status();
        status.running && status.config.enabled
    };
    
    if transcription_enabled {
        // Submit microphone transcription job
        if mic_path.exists() {
            let job = transcription::TranscriptionJob {
                segment_id: metadata.id.clone(),
                display_index: primary_display_index,
                source: transcription::AudioSource::Microphone,
                audio_path: mic_path,
                segment_start_time: metadata.start_time.clone(),
                retry_count: 0,
                created_at: chrono::Utc::now(),
                audio_path_local: audio_path_result.clone(),
            };
            if let Err(e) = transcription::submit_job(job) {
                log::warn!("Failed to queue mic transcription job: {}", e);
            }
        }
        
        // Submit system audio transcription job
        if system_audio_path.exists() {
            let job = transcription::TranscriptionJob {
                segment_id: metadata.id.clone(),
                display_index: primary_display_index,
                source: transcription::AudioSource::SystemAudio,
                audio_path: system_audio_path,
                segment_start_time: metadata.start_time.clone(),
                retry_count: 0,
                created_at: chrono::Utc::now(),
                audio_path_local: audio_path_result.clone(),
            };
            if let Err(e) = transcription::submit_job(job) {
                log::warn!("Failed to queue system audio transcription job: {}", e);
            }
        }
    }
    
    Ok(Some(metadata))
}

// Segment rotation loop
async fn segment_rotation_loop(app: AppHandle, segment_duration: Duration, generation: u64) {
    log::info!("Segment rotation loop started (interval: {:?}, generation: {})", segment_duration, generation);
    
    loop {
        // Wait for segment duration
        tokio::time::sleep(segment_duration).await;
        
        // Check if this loop has been superseded by a newer one (config restart)
        if ROTATION_GENERATION.load(Ordering::SeqCst) != generation {
            log::info!("Segment rotation loop: superseded by newer loop (gen {}), exiting", generation);
            break;
        }
        
        // Check shutdown
        if SHUTDOWN_SIGNAL.load(Ordering::SeqCst) {
            log::info!("Segment rotation loop: shutdown signal received");
            break;
        }
        
        // Check if we're still recording
        let is_recording = {
            let state = RECORDING_STATE.lock().unwrap();
            matches!(*state, RecordingStateHolder::Recording { .. })
        };
        
        if !is_recording {
            log::info!("Segment rotation loop: not recording, exiting");
            break;
        }
        
        log::info!("Rotating segment...");
        
        // Signal current capture to stop
        SHUTDOWN_SIGNAL.store(true, Ordering::SeqCst);
        
        // Give capture threads time to finish
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Finalize current segment
        if let Err(e) = finalize_current_segment(&app) {
            log::error!("Failed to finalize segment: {}", e);
        }
        
        // Reset shutdown signal for new segment
        SHUTDOWN_SIGNAL.store(false, Ordering::SeqCst);
        
        // Start new segment
        let config = RECORDING_CONFIG.lock().unwrap().clone();
        if let Err(e) = start_new_segment(&app, &config) {
            log::error!("Failed to start new segment: {}", e);
            break;
        }
    }
    
    log::info!("Segment rotation loop ended (generation: {})", generation);
}

// Get recording status
#[tauri::command]
pub async fn get_recording_status(app: AppHandle) -> Result<RecordingStatus, String> {
    let config = RECORDING_CONFIG.lock().unwrap().clone();
    let stats = RECORDING_STATS.lock().unwrap();
    let state = RECORDING_STATE.lock().unwrap();
    
    let (is_recording, current_segment_id, current_segment_start, current_segment_duration, display_count) = match &*state {
        RecordingStateHolder::Idle => (false, None, None, None, 0),
        RecordingStateHolder::Recording { segment_id, start_time, display_count, .. } => {
            let duration = (Utc::now() - *start_time).num_milliseconds() as f64 / 1000.0;
            (
                true,
                Some(segment_id.clone()),
                Some(start_time.to_rfc3339()),
                Some(duration),
                *display_count,
            )
        }
    };
    
    let total_storage = storage::calculate_total_storage(&app).unwrap_or(0);
    let total_segments = storage::count_segments(&app);
    
    Ok(RecordingStatus {
        enabled: config.enabled,
        is_recording,
        current_segment_id,
        current_segment_start,
        current_segment_duration_seconds: current_segment_duration,
        display_count,
        total_segments: total_segments.max(stats.total_segments),
        total_storage_bytes: total_storage,
    })
}

// Get current recording configuration
#[tauri::command]
pub async fn get_recording_config() -> Result<RecordingConfig, String> {
    Ok(RECORDING_CONFIG.lock().unwrap().clone())
}

// Valid FFmpeg presets for encoding
const VALID_PRESETS: &[&str] = &[
    "ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow", "slower", "veryslow"
];

// Update recording configuration
#[tauri::command]
pub async fn update_recording_config(
    app: AppHandle,
    new_config: RecordingConfig,
) -> Result<(), String> {
    // Validate config
    if new_config.framerate < 1 || new_config.framerate > 30 {
        return Err("Framerate must be between 1 and 30".to_string());
    }
    
    if new_config.segment_duration_seconds < 10 {
        return Err("Segment duration must be at least 10 seconds".to_string());
    }
    
    // Validate output_width (reasonable range for video encoding)
    if new_config.output_width < 640 || new_config.output_width > 3840 {
        return Err("Output width must be between 640 and 3840 pixels".to_string());
    }
    
    // Validate CRF (FFmpeg valid range is 0-51)
    if new_config.crf > 51 {
        return Err("CRF must be between 0 and 51".to_string());
    }
    
    // Validate preset
    if !VALID_PRESETS.contains(&new_config.preset.as_str()) {
        return Err(format!(
            "Invalid preset '{}'. Must be one of: {}",
            new_config.preset,
            VALID_PRESETS.join(", ")
        ));
    }
    
    let current_config = RECORDING_CONFIG.lock().unwrap().clone();
    
    if current_config == new_config {
        log::info!("Recording configuration unchanged");
        return Ok(());
    }
    
    let needs_restart = current_config.needs_recording_restart(&new_config);
    let is_recording = {
        let state = RECORDING_STATE.lock().unwrap();
        matches!(*state, RecordingStateHolder::Recording { .. })
    };
    
    // Store new config
    *RECORDING_CONFIG.lock().unwrap() = new_config.clone();
    config::save_config(&app, &new_config)?;
    
    // Handle restart if needed
    if is_recording && needs_restart {
        log::info!("Recording config change requires restart");
        stop_recording(app.clone()).await?;
        
        if new_config.enabled {
            start_recording(app).await?;
        }
    } else if !is_recording && new_config.enabled {
        log::info!("Starting recording (was not running)");
        start_recording(app).await?;
    } else if is_recording && !new_config.enabled {
        log::info!("Stopping recording (disabled in config)");
        stop_recording(app).await?;
    }
    
    Ok(())
}

// Get number of available displays
#[tauri::command]
pub async fn get_display_count() -> Result<u32, String> {
    Ok(capture::get_display_count() as u32)
}

// Get recordings within a date range
#[tauri::command]
pub async fn get_recordings_by_date_range(
    app: AppHandle,
    start_time: String,
    end_time: String,
) -> Result<RecordingsResponse, String> {
    // Parse the ISO 8601 timestamps
    let start = chrono::DateTime::parse_from_rfc3339(&start_time)
        .map_err(|e| format!("Invalid start_time format: {}", e))?
        .with_timezone(&Utc);
    
    let end = chrono::DateTime::parse_from_rfc3339(&end_time)
        .map_err(|e| format!("Invalid end_time format: {}", e))?
        .with_timezone(&Utc);
    
    let recordings = storage::get_recordings_in_range(&app, &start, &end)?;
    let total_count = recordings.len() as u64;
    
    Ok(RecordingsResponse {
        recordings,
        total_count,
    })
}

// =============================================================================
// Gemini AI Integration Commands
// =============================================================================

// Check if Gemini API key is available (user-provided, env var, or embedded)
#[tauri::command]
pub async fn has_gemini_api_key(app: AppHandle) -> Result<bool, String> {
    Ok(gemini::has_api_key_with_app(&app))
}

// Get Gemini configuration
#[tauri::command]
pub async fn get_gemini_config() -> Result<gemini::GeminiConfig, String> {
    Ok(gemini::get_queue_status().config)
}

// Update Gemini configuration
#[tauri::command]
pub async fn update_gemini_config(
    app: AppHandle,
    new_config: gemini::GeminiConfig,
) -> Result<(), String> {
    // Save config to disk
    config::save_gemini_config(&app, &new_config)?;
    
    // Update running queue
    gemini::queue::update_config(new_config);
    
    log::info!("Gemini configuration updated");
    Ok(())
}

// Get Gemini queue status
#[tauri::command]
pub async fn get_gemini_queue_status() -> Result<gemini::QueueStatus, String> {
    Ok(gemini::get_queue_status())
}

// Set Gemini API key (user-provided from settings)
#[tauri::command]
pub async fn set_gemini_api_key(
    app: AppHandle,
    api_key: String,
) -> Result<(), String> {
    // Validate that key is not empty
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    
    // Save to secure storage
    config::save_gemini_api_key(&app, &api_key)?;
    
    log::info!("Gemini API key saved to secure storage");
    Ok(())
}

// Get Gemini API key status (returns whether a key is set, not the key itself)
#[tauri::command]
pub async fn get_gemini_api_key_status(app: AppHandle) -> Result<bool, String> {
    // Check if user-provided key exists
    if let Ok(Some(key)) = config::load_gemini_api_key(&app) {
        if !key.trim().is_empty() {
            return Ok(true);
        }
    }
    
    // Fall back to checking env var and embedded key
    Ok(gemini::has_api_key())
}

// Delete Gemini API key (remove user-provided key)
#[tauri::command]
pub async fn delete_gemini_api_key(
    app: AppHandle,
) -> Result<(), String> {
    config::delete_gemini_api_key(&app)?;
    log::info!("Gemini API key deleted from secure storage");
    Ok(())
}

// =============================================================================
// Audio Feature Config Commands
// =============================================================================

/// Get audio feature configuration
#[tauri::command]
pub async fn get_audio_feature_config() -> Result<AudioFeatureConfig, String> {
    Ok(AUDIO_FEATURE_CONFIG.lock().unwrap().clone())
}

/// Update audio feature configuration
#[tauri::command]
pub async fn update_audio_feature_config(
    app: AppHandle,
    new_config: AudioFeatureConfig,
) -> Result<(), String> {
    let current_config = AUDIO_FEATURE_CONFIG.lock().unwrap().clone();
    
    if current_config == new_config {
        log::info!("Audio feature configuration unchanged");
        return Ok(());
    }
    
    let needs_restart = current_config.needs_recording_restart(&new_config);
    let is_recording = {
        let state = RECORDING_STATE.lock().unwrap();
        matches!(*state, RecordingStateHolder::Recording { .. })
    };
    
    // Store new config
    *AUDIO_FEATURE_CONFIG.lock().unwrap() = new_config.clone();
    config::save_audio_feature_config(&app, &new_config)?;
    
    // Update transcription queue with new settings
    transcription::update_config_from_audio_feature(&new_config);
    
    // Handle restart if needed
    if is_recording && needs_restart {
        log::info!("Audio feature config change requires restart");
        stop_recording(app.clone()).await?;
        
        // Check if recording should restart (if enabled in new config)
        if new_config.enabled {
            let config = RECORDING_CONFIG.lock().unwrap().clone();
            if config.enabled {
                start_recording(app).await?;
            }
        }
    }
    
    Ok(())
}

// =============================================================================
// Transcription Commands
// =============================================================================

/// Check if Whisper is available
#[tauri::command]
pub async fn is_whisper_available() -> Result<bool, String> {
    Ok(transcription::whisper::is_available())
}

/// Get transcription queue status
#[tauri::command]
pub async fn get_transcription_queue_status() -> Result<transcription::QueueStatus, String> {
    Ok(transcription::get_queue_status())
}
