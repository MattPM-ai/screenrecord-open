/**
 * ============================================================================
 * UPLOAD TYPES MODULE
 * ============================================================================
 * 
 * PURPOSE: Configuration and error types for audio upload/mixing
 * 
 * ============================================================================
 */

use serde::{Deserialize, Serialize};

/**
 * Upload configuration
 * 
 * Note: S3 upload has been removed - files are stored locally only.
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadConfig {
    /// Enable audio mixing and storage
    pub enabled: bool,
    
    /// Audio bitrate in kbps (e.g., 128)
    pub audio_bitrate_kbps: u32,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            audio_bitrate_kbps: 128,
        }
    }
}

/**
 * Upload-related errors
 */
#[derive(Debug)]
pub enum UploadError {
    /// File not found
    FileNotFound(String),
    
    /// File read error
    FileReadError(String),
    
    /// FFmpeg processing error
    ProcessingError(String),
    
    /// General upload error
    UploadError(String),
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadError::FileNotFound(path) => write!(f, "File not found: {}", path),
            UploadError::FileReadError(msg) => write!(f, "File read error: {}", msg),
            UploadError::ProcessingError(msg) => write!(f, "Processing error: {}", msg),
            UploadError::UploadError(msg) => write!(f, "Upload error: {}", msg),
        }
    }
}

impl std::error::Error for UploadError {}
