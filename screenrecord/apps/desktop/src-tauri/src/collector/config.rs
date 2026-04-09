/**
 * ============================================================================
 * COLLECTOR CONFIGURATION MODULE
 * ============================================================================
 * 
 * PURPOSE: Configuration schema, persistence, and validation
 * 
 * STORAGE: Configuration stored as JSON in app data directory
 * FILE PATH: {app_data_dir}/collector_config.json
 * 
 * FUNCTIONALITY:
 * - Define configuration schema with production defaults
 * - Validate configuration values
 * - Load configuration from disk
 * - Save configuration atomically
 * 
 * ============================================================================
 */

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use once_cell::sync::Lazy;
use tauri::{AppHandle, Manager};

/**
 * Global cached configuration instance
 * Initialized when collector starts, cleared when stopped
 */
static CACHED_CONFIG: Lazy<RwLock<Option<CollectorConfig>>> = Lazy::new(|| RwLock::new(None));

/**
 * Complete collector configuration
 * All transmission behavior is controlled through these settings
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollectorConfig {
    // Master enable/disable switch
    pub enabled: bool,

    // WebSocket endpoint (e.g., ws://localhost:8080/time-series)
    pub server_url: String,

    // Authentication endpoint (e.g., http://localhost:8080/mock-auth)
    pub auth_url: String,

    // User name for JWT claims and data tagging
    pub user_name: String,

    // User identifier for JWT claims and data tagging
    pub user_id: String,

    // Organization name for JWT claims and data tagging
    pub org_name: String,

    // Organization identifier for JWT claims and data tagging
    pub org_id: String,

    // Account identifier for JWT claims and data tagging
    pub account_id: String,

    // Maximum events per batch before forcing send
    pub batch_max_size: usize,

    // Maximum seconds between batch sends
    pub batch_max_interval_seconds: u64,

    // Maximum retry attempts for failed transmissions
    pub retry_max_attempts: u32,

    // Base backoff delay in milliseconds
    pub retry_backoff_base_ms: u64,

    // Exponential backoff multiplier
    pub retry_backoff_multiplier: f64,

    // Maximum backoff delay in seconds
    pub retry_max_delay_seconds: u64,

    // Maximum batches to store in offline queue
    pub offline_queue_max_batches: usize,

    // WebSocket keepalive ping interval in seconds
    pub websocket_keepalive_seconds: u64,

    // Connection timeout in seconds
    pub connection_timeout_seconds: u64,

    // Flush batch when user goes AFK
    pub flush_on_afk: bool,

    // Automatically reconnect on disconnect
    pub auto_reconnect: bool,

    // Optional JWT token from main app authentication
    // If provided, will be sent to collector auth endpoint for authorization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_jwt_token: Option<String>,
}

impl Default for CollectorConfig {
    /**
     * Production-ready default configuration
     * Collector disabled by default, requires user configuration
     */
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "ws://localhost:8080/time-series".to_string(),
            auth_url: "http://localhost:8080/mock-auth".to_string(),
            user_name: "Local".to_string(),
            user_id: "0".to_string(),
            org_name: "Local".to_string(),
            org_id: "0".to_string(),
            account_id: "0".to_string(),
            batch_max_size: 100,
            batch_max_interval_seconds: 60,
            retry_max_attempts: 5,
            retry_backoff_base_ms: 1000,
            retry_backoff_multiplier: 2.0,
            retry_max_delay_seconds: 60,
            offline_queue_max_batches: 1000,
            websocket_keepalive_seconds: 30,
            connection_timeout_seconds: 10,
            flush_on_afk: true,
            auto_reconnect: true,
            app_jwt_token: None,
        }
    }
}

impl CollectorConfig {
    /**
     * Check if config changes require a collector restart
     * 
     * Fields that require restart (affect connection/auth):
     * - enabled, server_url, auth_url, user_name, user_id, org_name, org_id, account_id
     * 
     * Fields that can be hot-updated (runtime behavior only):
     * - batch_max_size, batch_max_interval_seconds, retry settings, etc.
     */
    pub fn needs_restart(&self, other: &CollectorConfig) -> bool {
        // Connection/authentication fields require restart
        self.enabled != other.enabled ||
        self.server_url != other.server_url ||
        self.auth_url != other.auth_url ||
        self.user_name != other.user_name ||
        self.user_id != other.user_id ||
        self.org_name != other.org_name ||
        self.org_id != other.org_id ||
        self.account_id != other.account_id ||
        self.app_jwt_token != other.app_jwt_token
    }

    /**
     * Validate configuration values
     * Returns Ok(()) if valid, Err(String) with validation message if invalid
     */
    pub fn validate(&self) -> Result<(), String> {
        // If enabled, all user and org fields are required
        if self.enabled {
            if self.user_name.is_empty() {
                return Err("user_name is required when collector is enabled".to_string());
            }
            if self.user_id.is_empty() {
                return Err("user_id is required when collector is enabled".to_string());
            }
            if self.org_name.is_empty() {
                return Err("org_name is required when collector is enabled".to_string());
            }
            if self.org_id.is_empty() {
                return Err("org_id is required when collector is enabled".to_string());
            }
            if self.account_id.is_empty() {
                return Err("account_id is required when collector is enabled".to_string());
            }
            if self.user_name.len() > 64 {
                return Err("user_name must be 64 characters or less".to_string());
            }
            if self.user_id.len() > 64 {
                return Err("user_id must be 64 characters or less".to_string());
            }
            if self.org_name.len() > 64 {
                return Err("org_name must be 64 characters or less".to_string());
            }
            if self.org_id.len() > 64 {
                return Err("org_id must be 64 characters or less".to_string());
            }
            if self.account_id.len() > 64 {
                return Err("account_id must be 64 characters or less".to_string());
            }
        }

        // Validate URLs
        if !self.server_url.starts_with("ws://") && !self.server_url.starts_with("wss://") {
            return Err("server_url must start with ws:// or wss://".to_string());
        }
        if !self.auth_url.starts_with("http://") && !self.auth_url.starts_with("https://") {
            return Err("auth_url must start with http:// or https://".to_string());
        }

        // Validate numeric ranges
        if self.batch_max_size < 10 || self.batch_max_size > 10000 {
            return Err("batch_max_size must be between 10 and 10000".to_string());
        }
        if self.batch_max_interval_seconds < 10 || self.batch_max_interval_seconds > 3600 {
            return Err("batch_max_interval_seconds must be between 10 and 3600".to_string());
        }
        if self.retry_max_attempts < 1 || self.retry_max_attempts > 20 {
            return Err("retry_max_attempts must be between 1 and 20".to_string());
        }
        if self.retry_backoff_multiplier < 1.0 || self.retry_backoff_multiplier > 10.0 {
            return Err("retry_backoff_multiplier must be between 1.0 and 10.0".to_string());
        }

        Ok(())
    }
}

/**
 * Get path to configuration file
 * Returns {app_data_dir}/collector_config.json
 */
fn get_config_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data directory: {}", e))?;

    Ok(app_data_dir.join("collector_config.json"))
}

/**
 * Load configuration from disk
 * Returns default configuration if file doesn't exist or is invalid
 */
pub fn load_config(app_handle: &AppHandle) -> Result<CollectorConfig, String> {
    let config_path = get_config_path(app_handle)?;

    // If file doesn't exist, return default
    if !config_path.exists() {
        log::info!("Collector config not found, using defaults");
        return Ok(CollectorConfig::default());
    }

    // Read and parse JSON
    let json_str = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let mut config: CollectorConfig = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse config JSON: {}", e))?;

    // Always enforce default values for user/org fields
    config.user_name = "Local".to_string();
    config.user_id = "0".to_string();
    config.org_name = "Local".to_string();
    config.org_id = "0".to_string();
    config.account_id = "0".to_string();

    // Validate
    config.validate()?;

    log::info!("Loaded collector config from {} (with enforced defaults)", config_path.display());
    Ok(config)
}

/**
 * Save configuration to disk atomically
 * Uses temporary file + rename to prevent corruption
 */
pub fn save_config(app_handle: &AppHandle, config: &CollectorConfig) -> Result<(), String> {
    // Create a copy with enforced defaults for user/org fields
    let mut config_to_save = config.clone();
    config_to_save.user_name = "Local".to_string();
    config_to_save.user_id = "0".to_string();
    config_to_save.org_name = "Local".to_string();
    config_to_save.org_id = "0".to_string();
    config_to_save.account_id = "0".to_string();
    
    // Validate before saving
    config_to_save.validate()?;

    let config_path = get_config_path(app_handle)?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Serialize to pretty JSON (using config with enforced defaults)
    let json_str = serde_json::to_string_pretty(&config_to_save)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    // Write to temporary file
    let temp_path = config_path.with_extension("json.tmp");
    fs::write(&temp_path, json_str)
        .map_err(|e| format!("Failed to write temporary config file: {}", e))?;

    // Atomic rename
    fs::rename(&temp_path, &config_path)
        .map_err(|e| format!("Failed to save config file: {}", e))?;

    log::info!("Saved collector config to {}", config_path.display());
    Ok(())
}

/**
 * Initialize config cache
 * Stores configuration in memory for fast access during event collection
 * Always enforces default values for user/org fields
 */
pub fn init_cache(config: CollectorConfig) {
    let mut cache = CACHED_CONFIG.write().unwrap();
    // Create a copy with enforced defaults for user/org fields
    let mut config_with_defaults = config.clone();
    config_with_defaults.user_name = "Local".to_string();
    config_with_defaults.user_id = "0".to_string();
    config_with_defaults.org_name = "Local".to_string();
    config_with_defaults.org_id = "0".to_string();
    config_with_defaults.account_id = "0".to_string();
    *cache = Some(config_with_defaults);
    log::info!("Collector config cache initialized (with enforced defaults)");
    if config.app_jwt_token.is_some() {
        log::info!("App JWT token included in cache (length: {})", config.app_jwt_token.as_ref().unwrap().len());
    } else {
        log::warn!("No app JWT token in cached config");
    }
}

/**
 * Update app JWT token in cached config
 * Called when token is refreshed or updated
 */
pub fn update_app_jwt_token(token: Option<String>) {
    let mut cache = CACHED_CONFIG.write().unwrap();
    if let Some(ref mut config) = *cache {
        let old_token = config.app_jwt_token.clone();
        config.app_jwt_token = token.clone();
        if token.is_some() {
            log::info!("[CACHE] Updated app JWT token in cache: length={}, was_none={}", 
                token.as_ref().unwrap().len(), 
                old_token.is_none());
        } else {
            log::info!("[CACHE] Cleared app JWT token from cache (was_none={})", old_token.is_none());
        }
    } else {
        log::warn!("[CACHE] Cannot update app JWT token - config cache not initialized");
    }
}

/**
 * Get cached configuration
 * Returns cloned config if cache is initialized, None otherwise
 */
pub fn get_cached_config() -> Option<CollectorConfig> {
    let cache = CACHED_CONFIG.read().unwrap();
    cache.clone()
}

/**
 * Clear config cache
 * Called when collector is stopped
 */
pub fn clear_cache() {
    let mut cache = CACHED_CONFIG.write().unwrap();
    *cache = None;
    log::info!("Collector config cache cleared");
}

/**
 * Update config cache without restart
 * Used for hot-updating runtime settings that don't require reconnection
 */
pub fn update_cache(config: CollectorConfig) {
    let mut cache = CACHED_CONFIG.write().unwrap();
    if config.app_jwt_token.is_some() {
        log::info!("Collector config cache updated (hot-update) with app JWT token (length: {})", 
            config.app_jwt_token.as_ref().unwrap().len());
    } else {
        log::info!("Collector config cache updated (hot-update) - no app JWT token");
    }
    *cache = Some(config);
}

/**
 * Check if collector is enabled
 * Returns false if cache is not initialized or collector is disabled
 */
pub fn is_enabled() -> bool {
    get_cached_config()
        .map(|config| config.enabled)
        .unwrap_or(false)
}

/**
 * Check if flush on AFK is enabled
 * Returns false if cache is not initialized, collector is disabled, or flush_on_afk is disabled
 */
pub fn should_flush_on_afk() -> bool {
    get_cached_config()
        .map(|config| config.enabled && config.flush_on_afk)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_valid() {
        let config = CollectorConfig::default();
        // Default config should be valid except for empty user_id/org_id when enabled
        assert!(!config.enabled); // Disabled by default, so validation passes
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_requires_user_org_when_enabled() {
        let mut config = CollectorConfig::default();
        config.enabled = true;
        // Should fail: all required fields are empty
        assert!(config.validate().is_err());

        config.user_name = "Test User".to_string();
        config.user_id = "test_user".to_string();
        config.org_name = "Test Org".to_string();
        config.org_id = "test_org".to_string();
        config.account_id = "test_account".to_string();
        // Should now pass
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_url_formats() {
        let mut config = CollectorConfig::default();
        config.enabled = true;
        config.user_name = "Test User".to_string();
        config.user_id = "test".to_string();
        config.org_name = "Test Org".to_string();
        config.org_id = "test".to_string();
        config.account_id = "test_account".to_string();

        // Invalid server URL
        config.server_url = "http://invalid".to_string();
        assert!(config.validate().is_err());

        config.server_url = "wss://valid.com/ws".to_string();
        assert!(config.validate().is_ok());

        // Invalid auth URL
        config.auth_url = "ws://invalid".to_string();
        assert!(config.validate().is_err());

        config.auth_url = "https://valid.com/auth".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_numeric_ranges() {
        let mut config = CollectorConfig::default();
        config.enabled = true;
        config.user_name = "Test User".to_string();
        config.user_id = "test".to_string();
        config.org_name = "Test Org".to_string();
        config.org_id = "test".to_string();
        config.account_id = "test_account".to_string();

        // Test batch_max_size
        config.batch_max_size = 5;
        assert!(config.validate().is_err());
        config.batch_max_size = 100;
        assert!(config.validate().is_ok());

        // Test batch_max_interval_seconds
        config.batch_max_interval_seconds = 5;
        assert!(config.validate().is_err());
        config.batch_max_interval_seconds = 60;
        assert!(config.validate().is_ok());
    }
}

