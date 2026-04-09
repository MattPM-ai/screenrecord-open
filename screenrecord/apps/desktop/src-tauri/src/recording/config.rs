/**
 * ============================================================================
 * RECORDING CONFIG MODULE
 * ============================================================================
 * 
 * PURPOSE: Configuration persistence for screen recording system
 * 
 * FUNCTIONALITY:
 * - Load/save recording configuration to disk
 * - Default configuration when none exists
 * - JSON-based storage in app data directory
 * 
 * ============================================================================
 */

use crate::recording::gemini::GeminiConfig;
use crate::recording::types::{RecordingConfig, AudioFeatureConfig};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

// Get config file path
fn config_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app_data_dir available")
        .join("recording_config.json")
}

// Load configuration from disk
pub fn load_config(app: &AppHandle) -> Result<RecordingConfig, String> {
    let path = config_path(app);
    
    if !path.exists() {
        log::info!("No recording config found, using defaults");
        return Ok(RecordingConfig::default());
    }
    
    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {}", e))?;
    
    let config: RecordingConfig =
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse config: {}", e))?;
    
    log::info!("Loaded recording config from {:?}", path);
    Ok(config)
}

// Save configuration to disk
pub fn save_config(app: &AppHandle, config: &RecordingConfig) -> Result<(), String> {
    let path = config_path(app);
    
    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    
    let contents = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    std::fs::write(&path, contents).map_err(|e| format!("Failed to write config: {}", e))?;
    
    log::info!("Saved recording config to {:?}", path);
    Ok(())
}

// =============================================================================
// Gemini Configuration
// =============================================================================

// Get Gemini config file path
fn gemini_config_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app_data_dir available")
        .join("gemini_config.json")
}

// Load Gemini configuration from disk
pub fn load_gemini_config(app: &AppHandle) -> Result<GeminiConfig, String> {
    let path = gemini_config_path(app);
    
    if !path.exists() {
        log::info!("No Gemini config found, using defaults");
        return Ok(GeminiConfig::default());
    }
    
    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read Gemini config: {}", e))?;
    
    let config: GeminiConfig =
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse Gemini config: {}", e))?;
    
    log::info!("Loaded Gemini config from {:?}", path);
    Ok(config)
}

// Save Gemini configuration to disk
pub fn save_gemini_config(app: &AppHandle, config: &GeminiConfig) -> Result<(), String> {
    let path = gemini_config_path(app);
    
    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    
    let contents = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize Gemini config: {}", e))?;
    
    std::fs::write(&path, contents).map_err(|e| format!("Failed to write Gemini config: {}", e))?;
    
    log::info!("Saved Gemini config to {:?}", path);
    Ok(())
}

// =============================================================================
// Gemini API Key Storage (Separate from config for security)
// =============================================================================

// Get Gemini API key file path
fn gemini_api_key_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app_data_dir available")
        .join("gemini_api_key.txt")
}

// Load Gemini API key from secure storage
pub fn load_gemini_api_key(app: &AppHandle) -> Result<Option<String>, String> {
    let path = gemini_api_key_path(app);
    
    if !path.exists() {
        log::debug!("No Gemini API key file found");
        return Ok(None);
    }
    
    let key = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read API key file: {}", e))?;
    
    let trimmed_key = key.trim().to_string();
    
    if trimmed_key.is_empty() {
        log::debug!("Gemini API key file is empty");
        return Ok(None);
    }
    
    log::debug!("Loaded Gemini API key from secure storage");
    Ok(Some(trimmed_key))
}

// Save Gemini API key to secure storage
pub fn save_gemini_api_key(app: &AppHandle, api_key: &str) -> Result<(), String> {
    let path = gemini_api_key_path(app);
    
    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    
    // Set restrictive file permissions (Unix-like systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path.parent().unwrap())
            .map_err(|_| "Failed to get parent dir metadata")?
            .permissions();
        perms.set_mode(0o700); // rwx------
        std::fs::set_permissions(&path.parent().unwrap(), perms)
            .map_err(|_| "Failed to set directory permissions")?;
    }
    
    std::fs::write(&path, api_key.trim())
        .map_err(|e| format!("Failed to write API key file: {}", e))?;
    
    // Set restrictive file permissions on the key file itself
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)
            .map_err(|_| "Failed to get file metadata")?
            .permissions();
        perms.set_mode(0o600); // rw-------
        std::fs::set_permissions(&path, perms)
            .map_err(|_| "Failed to set file permissions")?;
    }
    
    log::info!("Saved Gemini API key to secure storage");
    Ok(())
}

// Delete Gemini API key from secure storage
pub fn delete_gemini_api_key(app: &AppHandle) -> Result<(), String> {
    let path = gemini_api_key_path(app);
    
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete API key file: {}", e))?;
        log::info!("Deleted Gemini API key from secure storage");
    }
    
    Ok(())
}

// =============================================================================
// Audio Feature Configuration
// =============================================================================

// Get audio feature config file path
fn audio_feature_config_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app_data_dir available")
        .join("audio_feature_config.json")
}

// Load audio feature configuration from disk
/// 
/// Returns default configuration if file doesn't exist.
pub fn load_audio_feature_config(app: &AppHandle) -> Result<AudioFeatureConfig, String> {
    let path = audio_feature_config_path(app);
    
    if !path.exists() {
        log::info!("No audio feature config found, using defaults");
        return Ok(AudioFeatureConfig::default());
    }
    
    let contents = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read audio feature config: {}", e))?;
    
    let config: AudioFeatureConfig = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse audio feature config: {}", e))?;
    
    log::info!("Loaded audio feature config from {:?}", path);
    Ok(config)
}

// Save audio feature configuration to disk
pub fn save_audio_feature_config(app: &AppHandle, config: &AudioFeatureConfig) -> Result<(), String> {
    let path = audio_feature_config_path(app);
    
    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    
    let contents = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize audio feature config: {}", e))?;
    
    std::fs::write(&path, contents)
        .map_err(|e| format!("Failed to write audio feature config: {}", e))?;
    
    log::info!("Saved audio feature config to {:?}", path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = RecordingConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: RecordingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
