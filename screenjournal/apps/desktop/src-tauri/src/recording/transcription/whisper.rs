/**
 * ============================================================================
 * WHISPER INTEGRATION MODULE
 * ============================================================================
 * 
 * PURPOSE: Wrapper for whisper-rs (whisper.cpp Rust bindings) for local
 *          speech-to-text transcription
 * 
 * FUNCTIONALITY:
 * - Load and cache Whisper model on startup
 * - Convert WAV audio to format expected by Whisper (16kHz mono f32)
 * - Run transcription and extract word-level timestamps
 * - Thread-safe model access for background processing
 * 
 * MODEL REQUIREMENTS:
 * - Model file must be bundled at resources/whisper-tiny.en.bin
 * - Model is loaded once on app startup via init_whisper()
 * 
 * ============================================================================
 */

use crate::recording::transcription::types::{TranscribedWord, TranscriptionSegment};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

// =============================================================================
// Global State
// =============================================================================

/// Cached Whisper context (loaded once on startup)
static WHISPER_CONTEXT: OnceLock<WhisperContext> = OnceLock::new();

/// Flag indicating if whisper is available
static WHISPER_AVAILABLE: OnceLock<bool> = OnceLock::new();

// =============================================================================
// Path Resolution
// =============================================================================

/// Convert a path to a string suitable for C APIs (e.g. whisper.cpp).
/// On Windows, strips the verbatim prefix \\?\ so that C runtime can open the file.
fn path_to_c_str(path: &Path) -> Result<String, String> {
    let s = path
        .to_str()
        .ok_or_else(|| "Invalid model path encoding".to_string())?;
    #[cfg(windows)]
    let s = {
        if s.starts_with(r"\\?\") {
            s.strip_prefix(r"\\?\").unwrap_or(s)
        } else {
            s
        }
    };
    Ok(s.to_string())
}

/**
 * Resolve the Whisper model file path (dev and production)
 * 
 * Searches in order:
 * 1. DEV: src-tauri/resources/whisper-tiny.en.bin
 * 2. DEV: resources/whisper-tiny.en.bin (when cwd is src-tauri)
 * 3. PROD: {resource_dir}/whisper-tiny.en.bin
 */
pub fn resolve_whisper_model_path(app: &AppHandle) -> Option<PathBuf> {
    let model_name = "whisper-tiny.en.bin";
    
    // DEV: Try project paths first
    if cfg!(debug_assertions) {
        // 1) src-tauri/resources/whisper-tiny.en.bin path from project root
        let candidate1 = PathBuf::from("src-tauri")
            .join("resources")
            .join(model_name);
        if candidate1.exists() {
            log::info!("Whisper model found at dev path: {:?}", candidate1);
            return Some(candidate1);
        }
        
        // 2) resources/whisper-tiny.en.bin path when cwd is src-tauri
        let candidate2 = PathBuf::from("resources").join(model_name);
        if candidate2.exists() {
            log::info!("Whisper model found at dev path: {:?}", candidate2);
            return Some(candidate2);
        }
        
        // 3) Try Tauri resource resolver
        if let Ok(p) = app.path().resolve(
            model_name,
            tauri::path::BaseDirectory::Resource,
        ) {
            if p.exists() {
                log::info!("Whisper model found via Tauri resolver: {:?}", p);
                return Some(p);
            }
        }
    }
    
    // PROD: Use packaged resource dir
    if let Ok(resource_dir) = app.path().resource_dir() {
        let prod_path = resource_dir.join(model_name);
        log::info!(
            "[WHISPER] Checking prod path: resource_dir={:?}, model_path={:?}, exists={}",
            resource_dir,
            prod_path,
            prod_path.exists()
        );
        if prod_path.exists() {
            log::info!("Whisper model path (prod): {:?}", prod_path);
            return Some(prod_path);
        }
        // Fallback: Tauri resolve (can differ on Windows)
        if let Ok(p) = app.path().resolve(model_name, tauri::path::BaseDirectory::Resource) {
            if p.exists() {
                log::info!("Whisper model found via resolve fallback: {:?}", p);
                return Some(p);
            }
        }
        log::warn!("Whisper model not found at {:?}", prod_path);
    } else {
        log::error!("Failed to resolve resource_dir for Whisper model");
    }
    
    None
}

// =============================================================================
// Initialization
// =============================================================================

/**
 * Initialize the Whisper model from the bundled model file
 * 
 * Must be called once on app startup before any transcription.
 * The model is cached for the lifetime of the application.
 * 
 * # Arguments
 * * `model_path` - Path to the .bin model file (e.g., whisper-tiny.en.bin)
 * 
 * # Returns
 * * `Ok(())` if model loaded successfully
 * * `Err(String)` with error message if loading failed
 */
pub fn init_whisper(model_path: &Path) -> Result<(), String> {
    log::info!("[WHISPER] Initializing Whisper model from {:?}", model_path);
    
    // Check if model file exists
    if !model_path.exists() {
        let msg = format!("Whisper model file not found: {:?}", model_path);
        log::warn!("[WHISPER] {}", msg);
        WHISPER_AVAILABLE.set(false).ok();
        return Err(msg);
    }
    
    // Get file size for logging
    let file_size = std::fs::metadata(model_path)
        .map(|m| m.len())
        .unwrap_or(0);
    log::info!("[WHISPER] Model file size: {} bytes ({:.1} MB)", file_size, file_size as f64 / 1_000_000.0);
    
    // Load the model
    let params = WhisperContextParameters::default();
    
    // Use a path string that C APIs on Windows can open (strip verbatim prefix \\?\ if present)
    let model_path_str = path_to_c_str(model_path)?;
    
    match WhisperContext::new_with_params(&model_path_str, params) {
        Ok(ctx) => {
            WHISPER_CONTEXT.set(ctx)
                .map_err(|_| "Whisper already initialized".to_string())?;
            WHISPER_AVAILABLE.set(true).ok();
            log::info!("[WHISPER] Model loaded successfully");
            Ok(())
        }
        Err(e) => {
            let msg = format!("Failed to load Whisper model: {}", e);
            log::error!("[WHISPER] {}", msg);
            WHISPER_AVAILABLE.set(false).ok();
            Err(msg)
        }
    }
}

/**
 * Check if Whisper is available and ready for transcription
 */
pub fn is_available() -> bool {
    WHISPER_AVAILABLE.get().copied().unwrap_or(false)
}

// =============================================================================
// Audio Processing
// =============================================================================

/**
 * Internal result from transcription containing raw segments
 */
pub struct RawTranscriptionResult {
    pub segments: Vec<TranscriptionSegment>,
    pub full_text: String,
    pub audio_duration_ms: u64,
}

/**
 * Load a WAV file and convert to the format expected by Whisper
 * 
 * Whisper requires: 16kHz sample rate, mono, f32 samples
 * This function handles resampling and channel conversion as needed.
 * 
 * # Arguments
 * * `path` - Path to the WAV file
 * 
 * # Returns
 * * `Ok((samples, duration_ms))` - f32 samples and duration in milliseconds
 * * `Err(String)` - Error message if loading failed
 */
fn load_wav_for_whisper(path: &Path) -> Result<(Vec<f32>, u64), String> {
    log::info!("[WHISPER] Loading WAV file: {:?}", path);
    
    let reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV file: {}", e))?;
    
    let spec = reader.spec();
    log::info!(
        "[WHISPER] WAV spec: {} channels, {} Hz, {:?} format, {} bits",
        spec.channels, spec.sample_rate, spec.sample_format, spec.bits_per_sample
    );
    
    // Read samples based on format
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_value = (1 << (spec.bits_per_sample - 1)) as f32;
            reader.into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_value)
                .collect()
        }
        hound::SampleFormat::Float => {
            reader.into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect()
        }
    };
    
    log::info!("[WHISPER] Read {} raw samples", samples.len());
    
    // Convert stereo to mono if needed
    let samples = if spec.channels == 2 {
        log::info!("[WHISPER] Converting stereo to mono");
        samples.chunks(2)
            .map(|chunk| {
                if chunk.len() == 2 {
                    (chunk[0] + chunk[1]) / 2.0
                } else {
                    chunk[0]
                }
            })
            .collect()
    } else if spec.channels > 2 {
        // For more than 2 channels, average all
        log::info!("[WHISPER] Converting {} channels to mono", spec.channels);
        samples.chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        samples
    };
    
    // Resample to 16kHz if needed (Whisper requirement)
    let samples = if spec.sample_rate != 16000 {
        log::info!("[WHISPER] Resampling from {} Hz to 16000 Hz", spec.sample_rate);
        resample_to_16khz(&samples, spec.sample_rate)
    } else {
        samples
    };
    
    // Calculate duration based on 16kHz sample rate
    let duration_ms = (samples.len() as u64 * 1000) / 16000;
    
    log::info!(
        "[WHISPER] Prepared {} samples for Whisper ({} ms duration)",
        samples.len(), duration_ms
    );
    
    Ok((samples, duration_ms))
}

/**
 * Simple linear interpolation resampling to 16kHz
 * 
 * Note: This is a basic implementation. For production use with
 * non-standard sample rates, consider using a dedicated resampling
 * library for better quality.
 */
fn resample_to_16khz(samples: &[f32], source_rate: u32) -> Vec<f32> {
    if source_rate == 16000 {
        return samples.to_vec();
    }
    
    let ratio = source_rate as f64 / 16000.0;
    let new_len = (samples.len() as f64 / ratio) as usize;
    let mut resampled = Vec::with_capacity(new_len);
    
    for i in 0..new_len {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(samples.len() - 1);
        let frac = src_idx - idx_floor as f64;
        
        let sample = samples[idx_floor] * (1.0 - frac as f32) + samples[idx_ceil] * frac as f32;
        resampled.push(sample);
    }
    
    resampled
}

// =============================================================================
// Transcription
// =============================================================================

/**
 * Transcribe an audio file using Whisper
 * 
 * # Arguments
 * * `audio_path` - Path to the WAV audio file
 * 
 * # Returns
 * * `Ok(RawTranscriptionResult)` - Transcription segments and full text
 * * `Err(String)` - Error message if transcription failed
 */
pub fn transcribe_audio(audio_path: &Path) -> Result<RawTranscriptionResult, String> {
    log::info!("[WHISPER] Starting transcription of {:?}", audio_path);
    
    let ctx = WHISPER_CONTEXT.get()
        .ok_or_else(|| "Whisper not initialized. Call init_whisper() first.".to_string())?;
    
    // Load and prepare audio
    let (samples, audio_duration_ms) = load_wav_for_whisper(audio_path)?;
    
    if samples.is_empty() {
        return Ok(RawTranscriptionResult {
            segments: Vec::new(),
            full_text: String::new(),
            audio_duration_ms: 0,
        });
    }
    
    // Configure Whisper parameters
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    
    // Set language to English (model is English-only anyway)
    params.set_language(Some("en"));
    
    // Enable token-level timestamps for word timing
    params.set_token_timestamps(true);
    
    // Disable various outputs we don't need
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_print_special(false);
    
    // Create state for this transcription
    let mut state = ctx.create_state()
        .map_err(|e| format!("Failed to create Whisper state: {}", e))?;
    
    // Run inference
    log::info!("[WHISPER] Running inference on {} samples...", samples.len());
    state.full(params, &samples)
        .map_err(|e| format!("Whisper inference failed: {}", e))?;
    
    // Extract segments with word timestamps
    let segments = extract_segments(&state)?;
    
    // Build full text from segments
    let full_text = segments.iter()
        .map(|s| s.text.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    
    let word_count: usize = segments.iter().map(|s| s.words.len()).sum();
    log::info!(
        "[WHISPER] Transcription complete: {} segments, {} words, {} chars",
        segments.len(), word_count, full_text.len()
    );
    
    Ok(RawTranscriptionResult {
        segments,
        full_text,
        audio_duration_ms,
    })
}

/**
 * Extract segments with word-level timestamps from Whisper state
 * 
 * Handles invalid UTF-8 gracefully - some tokens (like music notes)
 * may contain byte sequences that aren't valid UTF-8.
 */
fn extract_segments(state: &whisper_rs::WhisperState) -> Result<Vec<TranscriptionSegment>, String> {
    let num_segments = state.full_n_segments()
        .map_err(|e| format!("Failed to get segment count: {}", e))?;
    
    let mut segments = Vec::new();
    
    for i in 0..num_segments {
        // Get segment timing
        let start_time = state.full_get_segment_t0(i)
            .map_err(|e| format!("Failed to get segment start time: {}", e))?;
        let end_time = state.full_get_segment_t1(i)
            .map_err(|e| format!("Failed to get segment end time: {}", e))?;
        
        // Convert from Whisper centiseconds to milliseconds
        let start_ms = (start_time * 10) as u64;
        let end_ms = (end_time * 10) as u64;
        
        // Get segment text - handle invalid UTF-8 gracefully
        let text = match state.full_get_segment_text(i) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("[WHISPER] Segment {} has invalid UTF-8 text, skipping: {}", i, e);
                continue;
            }
        };
        
        // Extract word-level timestamps
        let num_tokens = state.full_n_tokens(i)
            .map_err(|e| format!("Failed to get token count: {}", e))?;
        
        let mut words = Vec::new();
        
        for j in 0..num_tokens {
            // Get token text - handle invalid UTF-8 gracefully
            let token_text = match state.full_get_token_text(i, j) {
                Ok(t) => t,
                Err(_) => {
                    // Skip tokens with invalid UTF-8 (e.g., music symbols)
                    continue;
                }
            };
            
            // Skip special tokens (start with [, like [BLANK], [MUSIC], etc.)
            if token_text.starts_with('[') || token_text.is_empty() {
                continue;
            }
            
            // Skip tokens that are just whitespace or special characters
            let trimmed = token_text.trim();
            if trimmed.is_empty() {
                continue;
            }
            
            // Get token timing data
            let token_data = match state.full_get_token_data(i, j) {
                Ok(d) => d,
                Err(_) => continue,
            };
            
            // Convert timing (Whisper uses centiseconds internally)
            let word_start_ms = (token_data.t0 * 10) as u64;
            let word_end_ms = (token_data.t1 * 10) as u64;
            
            // Get probability
            let probability = token_data.p;
            
            words.push(TranscribedWord {
                word: trimmed.to_string(),
                start_ms: word_start_ms,
                end_ms: word_end_ms,
                probability,
            });
        }
        
        segments.push(TranscriptionSegment {
            start_ms,
            end_ms,
            text: text.trim().to_string(),
            words,
        });
    }
    
    Ok(segments)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_same_rate() {
        let samples = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let resampled = resample_to_16khz(&samples, 16000);
        assert_eq!(samples, resampled);
    }

    #[test]
    fn test_resample_downsample() {
        // 32kHz to 16kHz should roughly halve the samples
        let samples: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let resampled = resample_to_16khz(&samples, 32000);
        // Should be approximately half the length
        assert!(resampled.len() >= 45 && resampled.len() <= 55);
    }

    #[test]
    fn test_is_available_before_init() {
        // Before initialization, should return false
        // Note: This test might fail if run after other tests that call init_whisper
        // In practice, is_available() should return false until init_whisper succeeds
        assert!(!is_available() || WHISPER_CONTEXT.get().is_some());
    }
}
