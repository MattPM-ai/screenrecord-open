/**
 * ============================================================================
 * RECORDING STORAGE MODULE
 * ============================================================================
 * 
 * PURPOSE: Manage storage of MP4 recording segment files
 * 
 * FUNCTIONALITY:
 * - Generate paths for recording segments
 * - Calculate total storage usage
 * - Cleanup old recordings by age
 * - Cleanup by storage quota
 * 
 * FILE STRUCTURE:
 * ~/.screenjournal/recordings/
 * ├── 2025-01-15/
 * │   ├── segment_1736956800_abc123.mp4   # H.264 encoded video
 * │   ├── segment_1736956800_abc123.json  # Metadata sidecar
 * │   └── ...
 * └── 2025-01-16/
 *     └── ...
 * ~/.screenjournal/audio/
 * ├── 2025-01-15/
 * │   ├── segment_1736956800_abc123.mp4   # Mixed audio (AAC in MP4)
 * │   └── ...
 * └── 2025-01-16/
 *     └── ...
 * 
 * ============================================================================
 */

use crate::recording::types::{RecordingConfig, RecordingMetadata};
use chrono::{NaiveDate, Utc};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use walkdir::WalkDir;

// Get the base recordings directory
pub fn get_recordings_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app_data_dir available")
        .join("recordings")
}

// Get the base audio directory (separate from recordings)
pub fn get_audio_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app_data_dir available")
        .join("audio")
}

// Get the path for a mixed audio file (MP4 format)
pub fn get_audio_path(app: &AppHandle, date: &NaiveDate, segment_id: &str) -> PathBuf {
    let base_dir = get_audio_dir(app);
    let date_dir = base_dir.join(date.format("%Y-%m-%d").to_string());
    date_dir.join(format!("{}.mp4", segment_id))
}

// Ensure the audio directory exists for a date
pub fn ensure_audio_dir(app: &AppHandle, date: &NaiveDate) -> Result<PathBuf, String> {
    let base_dir = get_audio_dir(app);
    let date_dir = base_dir.join(date.format("%Y-%m-%d").to_string());
    
    std::fs::create_dir_all(&date_dir)
        .map_err(|e| format!("Failed to create audio directory: {}", e))?;
    
    Ok(date_dir)
}

// Get the path for an MP4 recording segment file for a specific display
pub fn get_video_path(app: &AppHandle, date: &NaiveDate, segment_id: &str, display_index: u32) -> PathBuf {
    let base_dir = get_recordings_dir(app);
    let date_dir = base_dir.join(date.format("%Y-%m-%d").to_string());
    date_dir.join(format!("{}_d{}.mp4", segment_id, display_index))
}

// Get the path for a metadata JSON file
pub fn get_metadata_path(app: &AppHandle, date: &NaiveDate, segment_id: &str) -> PathBuf {
    let base_dir = get_recordings_dir(app);
    let date_dir = base_dir.join(date.format("%Y-%m-%d").to_string());
    date_dir.join(format!("{}.json", segment_id))
}

// Ensure the recordings directory exists for a date
pub fn ensure_recording_dir(app: &AppHandle, date: &NaiveDate) -> Result<PathBuf, String> {
    let base_dir = get_recordings_dir(app);
    let date_dir = base_dir.join(date.format("%Y-%m-%d").to_string());
    
    std::fs::create_dir_all(&date_dir)
        .map_err(|e| format!("Failed to create recordings directory: {}", e))?;
    
    Ok(date_dir)
}

// Generate a unique segment ID
pub fn generate_segment_id() -> String {
    let timestamp = Utc::now().timestamp();
    let random_suffix: String = (0..6)
        .map(|_| {
            let byte: u8 = rand::random();
            format!("{:x}", byte)
        })
        .collect();
    format!("segment_{}_{}", timestamp, random_suffix)
}

// Save recording metadata to JSON file
pub fn save_metadata(app: &AppHandle, date: &NaiveDate, metadata: &RecordingMetadata) -> Result<(), String> {
    let path = get_metadata_path(app, date, &metadata.id);
    
    let contents = serde_json::to_string_pretty(metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
    
    std::fs::write(&path, contents)
        .map_err(|e| format!("Failed to write metadata: {}", e))?;
    
    log::info!("Saved metadata to {:?}", path);
    Ok(())
}

// Load recording metadata from JSON file
pub fn load_metadata(path: &PathBuf) -> Result<RecordingMetadata, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read metadata: {}", e))?;
    
    serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse metadata: {}", e))
}

// Calculate total storage used by recordings
pub fn calculate_total_storage(app: &AppHandle) -> Result<u64, String> {
    let recordings_dir = get_recordings_dir(app);
    
    if !recordings_dir.exists() {
        return Ok(0);
    }
    
    let mut total_size: u64 = 0;
    
    for entry in WalkDir::new(&recordings_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                total_size += metadata.len();
            }
        }
    }
    
    Ok(total_size)
}

// Count total segments (by counting .mp4 files)
pub fn count_segments(app: &AppHandle) -> u64 {
    let recordings_dir = get_recordings_dir(app);
    
    if !recordings_dir.exists() {
        return 0;
    }
    
    let mut count: u64 = 0;
    
    for entry in WalkDir::new(&recordings_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "mp4" {
                    count += 1;
                }
            }
        }
    }
    
    count
}

// Get all recording files with metadata (for cleanup)
fn get_all_recording_files(app: &AppHandle) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let recordings_dir = get_recordings_dir(app);
    let mut files = Vec::new();
    
    if !recordings_dir.exists() {
        return files;
    }
    
    for entry in WalkDir::new(&recordings_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                // Include both .mp4 and .json files
                if ext == "mp4" || ext == "json" {
                    if let Ok(metadata) = entry.metadata() {
                        let modified = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        files.push((entry.path().to_path_buf(), metadata.len(), modified));
                    }
                }
            }
        }
    }
    
    // Sort by modification time (oldest first)
    files.sort_by(|a, b| a.2.cmp(&b.2));
    
    files
}

// Cleanup recordings older than retention period
pub fn cleanup_old_recordings(app: &AppHandle, config: &RecordingConfig) -> Result<u64, String> {
    let recordings_dir = get_recordings_dir(app);
    
    if !recordings_dir.exists() {
        return Ok(0);
    }
    
    let cutoff = Utc::now() - chrono::Duration::days(config.retention_days as i64);
    let cutoff_system_time = std::time::SystemTime::UNIX_EPOCH
        + std::time::Duration::from_secs(cutoff.timestamp() as u64);
    
    let mut bytes_deleted: u64 = 0;
    let mut files_deleted: u32 = 0;
    let mut dirs_to_check = Vec::new();
    
    for entry in WalkDir::new(&recordings_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                let modified = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                
                if modified < cutoff_system_time {
                    let file_size = metadata.len();
                    
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        log::warn!("Failed to delete old recording {:?}: {}", entry.path(), e);
                    } else {
                        bytes_deleted += file_size;
                        files_deleted += 1;
                        
                        // Track parent directory for potential cleanup
                        if let Some(parent) = entry.path().parent() {
                            if !dirs_to_check.contains(&parent.to_path_buf()) {
                                dirs_to_check.push(parent.to_path_buf());
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Clean up empty date directories
    for dir in dirs_to_check {
        if dir != recordings_dir {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                if entries.count() == 0 {
                    let _ = std::fs::remove_dir(&dir);
                }
            }
        }
    }
    
    if files_deleted > 0 {
        log::info!(
            "Cleaned up {} old files ({} bytes)",
            files_deleted,
            bytes_deleted
        );
    }
    
    Ok(bytes_deleted)
}

// Cleanup recordings to stay within storage quota
// Deletes oldest recordings first until under quota
pub fn cleanup_by_quota(app: &AppHandle, config: &RecordingConfig) -> Result<u64, String> {
    let current_size = calculate_total_storage(app)?;
    
    if current_size <= config.max_storage_bytes {
        return Ok(0);
    }
    
    let target_size = (config.max_storage_bytes as f64 * 0.9) as u64; // Target 90% of quota
    let bytes_to_delete = current_size - target_size;
    
    log::info!(
        "Storage quota exceeded ({} / {}), need to delete {} bytes",
        current_size,
        config.max_storage_bytes,
        bytes_to_delete
    );
    
    let files = get_all_recording_files(app);
    let mut bytes_deleted: u64 = 0;
    let mut files_deleted: u32 = 0;
    
    for (path, size, _) in files {
        if bytes_deleted >= bytes_to_delete {
            break;
        }
        
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("Failed to delete recording {:?}: {}", path, e);
        } else {
            bytes_deleted += size;
            files_deleted += 1;
            
            // Clean up empty parent directory
            if let Some(parent) = path.parent() {
                if let Ok(entries) = std::fs::read_dir(parent) {
                    if entries.count() == 0 {
                        let _ = std::fs::remove_dir(parent);
                    }
                }
            }
        }
    }
    
    log::info!(
        "Deleted {} files ({} bytes) for quota compliance",
        files_deleted,
        bytes_deleted
    );
    
    Ok(bytes_deleted)
}

// Get file size
pub fn get_file_size(path: &PathBuf) -> Result<u64, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?;
    Ok(metadata.len())
}

// Get all recording metadata files in a date range
pub fn get_recordings_in_range(
    app: &AppHandle,
    start_time: &chrono::DateTime<Utc>,
    end_time: &chrono::DateTime<Utc>,
) -> Result<Vec<RecordingMetadata>, String> {
    let recordings_dir = get_recordings_dir(app);
    
    if !recordings_dir.exists() {
        return Ok(Vec::new());
    }
    
    let mut recordings = Vec::new();
    
    // Walk through all .json metadata files
    for entry in WalkDir::new(&recordings_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "json" {
                    // Try to load the metadata
                    if let Ok(metadata) = load_metadata(&entry.path().to_path_buf()) {
                        // Parse the start time and check if it's in range
                        if let Ok(recording_start) = chrono::DateTime::parse_from_rfc3339(&metadata.start_time) {
                            let recording_start_utc = recording_start.with_timezone(&Utc);
                            
                            if recording_start_utc >= *start_time && recording_start_utc <= *end_time {
                                recordings.push(metadata);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Sort by start time (newest first)
    recordings.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    
    Ok(recordings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_segment_id() {
        let id1 = generate_segment_id();
        let id2 = generate_segment_id();
        
        assert!(id1.starts_with("segment_"));
        assert!(id2.starts_with("segment_"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_segment_id_format() {
        let id = generate_segment_id();
        let parts: Vec<&str> = id.split('_').collect();
        
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "segment");
        assert!(parts[1].parse::<i64>().is_ok());
        assert_eq!(parts[2].len(), 12);
    }
}
