/**
 * ============================================================================
 * AUDIO MIXER MODULE
 * ============================================================================
 * 
 * PURPOSE: Mix microphone and system audio into a single MP4 file using FFmpeg
 * 
 * FEATURES:
 * - Combines mic.wav + audio.wav using FFmpeg amix filter
 * - Falls back to single source if only one exists
 * - Outputs AAC in MP4 container
 * 
 * ============================================================================
 */

use crate::recording::capture::get_ffmpeg_path;
use std::path::{Path, PathBuf};
use std::process::Command;

/**
 * Mix audio files into a single MP4 (AAC in MP4 container)
 * 
 * Combines system audio and microphone audio (if available) into one MP4 file.
 * Uses FFmpeg's amix filter for mixing.
 * 
 * Output is AAC codec in MP4 container, compatible with:
 * - QuickTime Player
 * - All major browsers and media players
 * 
 * # Arguments
 * * `system_audio_path` - Path to system audio WAV file
 * * `mic_audio_path` - Optional path to microphone WAV file
 * * `output_path` - Path for output MP4 file
 * * `bitrate_kbps` - Audio bitrate in kbps (e.g., 128)
 * 
 * # Returns
 * * `Ok(PathBuf)` - Path to created MP4 file
 * * `Err(String)` - Error message if mixing fails
 */
pub fn mix_audio_to_mp4(
    system_audio_path: &Path,
    mic_audio_path: Option<&Path>,
    output_path: &Path,
    bitrate_kbps: u32,
) -> Result<PathBuf, String> {
    let ffmpeg_path = get_ffmpeg_path()?;
    
    // Check which audio sources are available
    let has_system = system_audio_path.exists() 
        && std::fs::metadata(system_audio_path)
            .map(|m| m.len() > 44) // More than just WAV header
            .unwrap_or(false);
    
    let has_mic = mic_audio_path
        .map(|p| p.exists() && std::fs::metadata(p).map(|m| m.len() > 44).unwrap_or(false))
        .unwrap_or(false);
    
    if !has_system && !has_mic {
        return Err("No audio files available for mixing".to_string());
    }
    
    log::info!(
        "[AUDIO-MIX] Mixing audio: system={}, mic={} -> {:?}",
        has_system,
        has_mic,
        output_path
    );
    
    let bitrate_arg = format!("{}k", bitrate_kbps);
    
    let output = if has_system && has_mic {
        // Mix both audio sources
        let mic_p = mic_audio_path.unwrap();
        log::info!("[AUDIO-MIX] Using amix filter to combine system audio and mic audio");
        
        Command::new(&ffmpeg_path)
            .args([
                "-y",                                                   // Overwrite output
                "-i", system_audio_path.to_str().unwrap(),             // Input 0: System audio
                "-i", mic_p.to_str().unwrap(),                         // Input 1: Mic audio
                "-filter_complex", "[0:a][1:a]amix=inputs=2:duration=longest:dropout_transition=0[aout]",
                "-map", "[aout]",                                      // Mixed audio
                "-c:a", "aac",                                         // AAC codec
                "-b:a", &bitrate_arg,                                  // Bitrate
                "-movflags", "+faststart",                             // Enable streaming
            ])
            .arg(output_path)
            .output()
            .map_err(|e| format!("Failed to run FFmpeg mix: {}", e))?
    } else if has_system {
        // System audio only
        log::info!("[AUDIO-MIX] Converting system audio only to MP4");
        
        Command::new(&ffmpeg_path)
            .args([
                "-y",                                                   // Overwrite output
                "-i", system_audio_path.to_str().unwrap(),             // Input: System audio
                "-c:a", "aac",                                         // AAC codec
                "-b:a", &bitrate_arg,                                  // Bitrate
                "-movflags", "+faststart",                             // Enable streaming
            ])
            .arg(output_path)
            .output()
            .map_err(|e| format!("Failed to run FFmpeg convert: {}", e))?
    } else {
        // Mic audio only
        let mic_p = mic_audio_path.unwrap();
        log::info!("[AUDIO-MIX] Converting mic audio only to MP4");
        
        Command::new(&ffmpeg_path)
            .args([
                "-y",                                                   // Overwrite output
                "-i", mic_p.to_str().unwrap(),                         // Input: Mic audio
                "-c:a", "aac",                                         // AAC codec
                "-b:a", &bitrate_arg,                                  // Bitrate
                "-movflags", "+faststart",                             // Enable streaming
            ])
            .arg(output_path)
            .output()
            .map_err(|e| format!("Failed to run FFmpeg convert: {}", e))?
    };
    
    // Log FFmpeg output for debugging
    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_tail: String = stderr.chars().rev().take(300).collect::<String>().chars().rev().collect();
        log::debug!("[AUDIO-MIX] FFmpeg stderr (last 300 chars): {}", stderr_tail);
    }
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "FFmpeg audio mix failed with code {:?}: {}",
            output.status.code(),
            stderr.chars().take(500).collect::<String>()
        ));
    }
    
    // Verify output file was created
    if !output_path.exists() {
        return Err(format!("Output file was not created: {:?}", output_path));
    }
    
    let file_size = std::fs::metadata(output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    
    log::info!(
        "[AUDIO-MIX] ✓ Audio mixed successfully: {:?} ({} bytes)",
        output_path,
        file_size
    );
    
    Ok(output_path.to_path_buf())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_no_files_returns_error() {
        let result = mix_audio_to_mp4(
            Path::new("/nonexistent/system.wav"),
            Some(Path::new("/nonexistent/mic.wav")),
            Path::new("/tmp/output.mp4"),
            128,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No audio files available"));
    }
}
