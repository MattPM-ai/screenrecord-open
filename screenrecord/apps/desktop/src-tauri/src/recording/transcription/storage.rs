/**
 * ============================================================================
 * TRANSCRIPTION STORAGE MODULE
 * ============================================================================
 * 
 * PURPOSE: Transcript file I/O and path management
 * 
 * ============================================================================
 */

use crate::recording::storage;
use crate::recording::transcription::types::{AudioSource, TranscriptionResult};
use chrono::NaiveDate;
use std::path::PathBuf;
use tauri::AppHandle;

/**
 * Get the path for a transcript JSON file
 */
pub fn get_transcript_path(
    app: &AppHandle,
    date: &NaiveDate,
    segment_id: &str,
    display_index: u32,
    source: AudioSource,
) -> PathBuf {
    let base_dir = storage::get_recordings_dir(app);
    let date_dir = base_dir.join(date.format("%Y-%m-%d").to_string());
    date_dir.join(format!("{}_d{}.{}.transcript.json", segment_id, display_index, source.file_suffix()))
}

/**
 * Get the path for an audio WAV file
 */
pub fn get_audio_path(
    app: &AppHandle,
    date: &NaiveDate,
    segment_id: &str,
    display_index: u32,
    source: AudioSource,
) -> PathBuf {
    let base_dir = storage::get_recordings_dir(app);
    let date_dir = base_dir.join(date.format("%Y-%m-%d").to_string());
    date_dir.join(format!("{}_d{}.{}.wav", segment_id, display_index, source.file_suffix()))
}

/**
 * Save a transcription result to JSON file
 */
pub fn save_transcript(
    app: &AppHandle,
    transcription: &TranscriptionResult,
) -> Result<(), String> {
    // Parse date from segment start time
    let date = chrono::DateTime::parse_from_rfc3339(&transcription.transcribed_at)
        .map_err(|e| format!("Invalid timestamp in transcription: {}", e))?
        .date_naive();
    
    let path = get_transcript_path(
        app,
        &date,
        &transcription.segment_id,
        transcription.display_index,
        transcription.source,
    );
    
    let contents = serde_json::to_string_pretty(transcription)
        .map_err(|e| format!("Failed to serialize transcript: {}", e))?;
    
    std::fs::write(&path, contents)
        .map_err(|e| format!("Failed to write transcript: {}", e))?;
    
    log::info!("Saved transcript to {:?}", path);
    Ok(())
}
