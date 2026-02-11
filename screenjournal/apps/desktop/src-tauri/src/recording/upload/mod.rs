/**
 * ============================================================================
 * UPLOAD MODULE
 * ============================================================================
 * 
 * PURPOSE: Mix and save audio files locally (no S3 upload)
 * 
 * FEATURES:
 * - Mix mic + system audio into single MP4 (AAC in MP4 container)
 * - Save to local audio directory
 * - Return absolute path to saved file
 * - Graceful error handling (continues without path on failure)
 * 
 * FLOW:
 * 1. Mix audio files (FFmpeg) → MP4
 * 2. Save to audio directory → get absolute path
 * 3. Return path (or None on failure)
 * 
 * ============================================================================
 */

pub mod mixer;
pub mod types;

pub use types::{UploadConfig, UploadError};

use std::path::Path;

// =============================================================================
// Public API
// =============================================================================

/**
 * Mix audio files and save locally
 * 
 * Orchestrates the complete audio mixing process:
 * 1. Mixes mic and system audio into MP4
 * 2. Saves to local audio directory
 * 3. Returns absolute path
 * 
 * On any failure, logs error and returns None (graceful degradation).
 * 
 * # Arguments
 * * `segment_id` - Recording segment identifier
 * * `system_audio_path` - Path to system audio WAV file
 * * `mic_audio_path` - Optional path to microphone WAV file
 * * `output_path` - Full path where MP4 should be saved (must be absolute)
 * * `config` - Upload configuration
 * 
 * # Returns
 * * `Option<PathBuf>` - Absolute path to saved MP4 if successful, None if any step fails
 */
pub fn mix_and_save_audio(
    segment_id: &str,
    system_audio_path: &Path,
    mic_audio_path: Option<&Path>,
    output_path: &Path,
    config: &UploadConfig,
) -> Option<std::path::PathBuf> {
    if !config.enabled {
        log::info!("[UPLOAD] Audio mixing disabled in config, skipping");
        return None;
    }

    // Check if any audio exists
    let has_system = system_audio_path.exists();
    let has_mic = mic_audio_path.map(|p| p.exists()).unwrap_or(false);
    
    if !has_system && !has_mic {
        log::info!("[UPLOAD] No audio files found for segment {}, skipping", segment_id);
        return None;
    }

    log::info!(
        "[UPLOAD] Starting audio mix and save for segment {} (system: {}, mic: {})",
        segment_id,
        has_system,
        has_mic
    );

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::error!("[UPLOAD] Failed to create output directory {:?}: {}", parent, e);
            return None;
        }
    }

    // Mix audio to MP4
    let mix_result = mixer::mix_audio_to_mp4(
        system_audio_path,
        mic_audio_path,
        output_path,
        config.audio_bitrate_kbps,
    );

    match mix_result {
        Ok(path) => {
            // Convert to absolute path
            let absolute_path = match path.canonicalize() {
                Ok(canonical) => canonical,
                Err(_) => {
                    // If canonicalization fails, use the path as-is (should already be absolute)
                    path
                }
            };
            
            log::info!(
                "[UPLOAD] ✓ Audio saved locally for segment {}: {}",
                segment_id,
                absolute_path.display()
            );
            Some(absolute_path)
        }
        Err(e) => {
            log::error!("[UPLOAD] Failed to mix audio for segment {}: {}", segment_id, e);
            None
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_config_default() {
        let config = UploadConfig::default();
        assert!(config.enabled);
        assert_eq!(config.audio_bitrate_kbps, 128);
    }

    #[test]
    fn test_mix_and_save_disabled() {
        let config = UploadConfig {
            enabled: false,
            ..Default::default()
        };
        
        let result = mix_and_save_audio(
            "test_segment",
            Path::new("/nonexistent/system.wav"),
            None,
            Path::new("/tmp/output.mp4"),
            &config,
        );
        
        assert!(result.is_none());
    }

    #[test]
    fn test_mix_and_save_no_audio_files() {
        let config = UploadConfig::default();
        
        let result = mix_and_save_audio(
            "test_segment",
            Path::new("/nonexistent/system.wav"),
            Some(Path::new("/nonexistent/mic.wav")),
            Path::new("/tmp/output.mp4"),
            &config,
        );
        
        assert!(result.is_none());
    }
}
