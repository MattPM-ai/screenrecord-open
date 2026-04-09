/**
 * ============================================================================
 * COLLECTOR TYPES MODULE
 * ============================================================================
 * 
 * PURPOSE: Define all data structures used across the collector system
 * 
 * TYPES DEFINED:
 * - AuthCredentials: User and organization identifiers
 * - AuthToken: JWT token with expiration tracking
 * - LineProtocolBatch: Collection of formatted events ready for transmission
 * - TransmissionStatus: Connection state enum
 * - SyncStatistics: Metrics for monitoring transmission health
 * - EventData: Enum representing different event types
 * 
 * ============================================================================
 */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/**
 * Authentication credentials for obtaining JWT tokens
 * Used when calling the /mock-auth endpoint
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCredentials {
    pub user: String,
    pub org: String,
}

/**
 * JWT authentication token with lifecycle metadata
 * Tokens are cached and automatically refreshed when expired
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl AuthToken {
    /**
     * Check if token has expired or is about to expire (within 1 minute)
     * Returns true if token should be refreshed
     */
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        // Add 1-minute safety margin to avoid edge cases
        self.expires_at <= now + chrono::Duration::minutes(1)
    }
}

/**
 * A batch of line protocol strings ready for transmission
 * Batches are created when size or time thresholds are met
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineProtocolBatch {
    pub events: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub batch_id: String, // UUID for tracking
}

/**
 * Current connection status of the WebSocket client
 * Used for UI display and reconnection logic
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "message")]
pub enum TransmissionStatus {
    Disconnected,
    Connecting,
    Authenticating,
    Connected,
    Error(String),
}

/**
 * Synchronization statistics for monitoring and UI display
 * Tracks transmission health and queue status
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatistics {
    pub total_events_sent: u64,
    pub total_batches_sent: u64,
    pub last_sync_time: Option<DateTime<Utc>>,
    pub pending_events: usize,
    pub connection_status: TransmissionStatus,
    pub last_error: Option<String>,
    pub retry_attempts: u32,
}

impl Default for SyncStatistics {
    fn default() -> Self {
        Self {
            total_events_sent: 0,
            total_batches_sent: 0,
            last_sync_time: None,
            pending_events: 0,
            connection_status: TransmissionStatus::Disconnected,
            last_error: None,
            retry_attempts: 0,
        }
    }
}

/**
 * Retry state tracking for exponential backoff
 * Manages retry attempts and delay calculations
 */
#[derive(Debug, Clone)]
pub struct RetryState {
    pub attempts: u32,
    pub next_retry_at: Option<std::time::Instant>,
    pub current_delay_ms: u64,
}

impl Default for RetryState {
    fn default() -> Self {
        Self {
            attempts: 0,
            next_retry_at: None,
            current_delay_ms: 0,
        }
    }
}

impl RetryState {
    /**
     * Calculate next retry delay using exponential backoff
     * delay = base_ms * multiplier^attempts, capped at max_delay_seconds
     */
    pub fn calculate_next_delay(
        &self,
        base_ms: u64,
        multiplier: f64,
        max_delay_seconds: u64,
    ) -> u64 {
        let delay_ms = (base_ms as f64 * multiplier.powi(self.attempts as i32)) as u64;
        let max_delay_ms = max_delay_seconds * 1000;
        delay_ms.min(max_delay_ms)
    }

    /**
     * Increment retry attempts and schedule next retry
     */
    pub fn increment(&mut self, delay_ms: u64) {
        self.attempts += 1;
        self.current_delay_ms = delay_ms;
        self.next_retry_at = Some(std::time::Instant::now() + std::time::Duration::from_millis(delay_ms));
    }

    /**
     * Reset retry state after successful transmission
     */
    pub fn reset(&mut self) {
        self.attempts = 0;
        self.next_retry_at = None;
        self.current_delay_ms = 0;
    }

    /**
     * Check if it's time to retry
     * Returns true if next_retry_at has passed
     */
    pub fn should_retry_now(&self) -> bool {
        if let Some(next_retry) = self.next_retry_at {
            std::time::Instant::now() >= next_retry
        } else {
            true // No retry scheduled, can retry immediately
        }
    }
}

/**
 * Event data variants for different ActivityWatch event types
 * Each variant contains the specific fields for that event type
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum EventData {
    WindowActivity {
        timestamp: String,
        duration: f64,
        app: String,
        title: String,
        hostname: String,
    },
    AfkStatus {
        timestamp: String,
        duration: f64,
        status: String, // "active", "idle", "afk"
        hostname: String,
    },
    DailyMetrics {
        date: String,
        active_seconds: f64,
        idle_seconds: f64,
        afk_seconds: f64,
        utilization_ratio: f64,
        app_switches: i32,
        hostname: String,
    },
    AppUsage {
        timestamp: String,
        app_name: String,
        duration_seconds: f64,
        event_count: i32,
        category: Option<String>, // "productive", "neutral", "unproductive"
        hostname: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transmission_status_serialization() {
        // Test simple variants
        let disconnected = TransmissionStatus::Disconnected;
        let json = serde_json::to_string(&disconnected).unwrap();
        assert_eq!(json, r#"{"type":"Disconnected"}"#);

        let connecting = TransmissionStatus::Connecting;
        let json = serde_json::to_string(&connecting).unwrap();
        assert_eq!(json, r#"{"type":"Connecting"}"#);

        let connected = TransmissionStatus::Connected;
        let json = serde_json::to_string(&connected).unwrap();
        assert_eq!(json, r#"{"type":"Connected"}"#);

        // Test error variant with content
        let error = TransmissionStatus::Error("Test error message".to_string());
        let json = serde_json::to_string(&error).unwrap();
        assert_eq!(json, r#"{"type":"Error","message":"Test error message"}"#);
    }

    #[test]
    fn test_transmission_status_deserialization() {
        // Test simple variants
        let json = r#"{"type":"Disconnected"}"#;
        let status: TransmissionStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, TransmissionStatus::Disconnected);

        // Test error variant
        let json = r#"{"type":"Error","message":"Connection failed"}"#;
        let status: TransmissionStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, TransmissionStatus::Error("Connection failed".to_string()));
    }

    #[test]
    fn test_auth_token_expiration() {
        let now = Utc::now();
        
        // Token expiring in 2 hours should not be expired
        let valid_token = AuthToken {
            token: "test_token".to_string(),
            issued_at: now,
            expires_at: now + chrono::Duration::hours(2),
        };
        assert!(!valid_token.is_expired());

        // Token expiring in 30 seconds should be considered expired (due to 1-minute safety margin)
        let soon_expired_token = AuthToken {
            token: "test_token".to_string(),
            issued_at: now,
            expires_at: now + chrono::Duration::seconds(30),
        };
        assert!(soon_expired_token.is_expired());

        // Token expired 1 hour ago should be expired
        let expired_token = AuthToken {
            token: "test_token".to_string(),
            issued_at: now - chrono::Duration::hours(25),
            expires_at: now - chrono::Duration::hours(1),
        };
        assert!(expired_token.is_expired());
    }

    #[test]
    fn test_retry_state_logic() {
        let mut retry_state = RetryState::default();
        
        // Initially should retry immediately
        assert!(retry_state.should_retry_now());
        assert_eq!(retry_state.attempts, 0);

        // Calculate first delay
        let delay = retry_state.calculate_next_delay(1000, 2.0, 60);
        assert_eq!(delay, 1000); // First attempt: base delay

        // Increment retry state
        retry_state.increment(delay);
        assert_eq!(retry_state.attempts, 1);
        assert!(!retry_state.should_retry_now()); // Should not retry immediately after increment

        // Calculate second delay (exponential backoff)
        let delay2 = retry_state.calculate_next_delay(1000, 2.0, 60);
        assert_eq!(delay2, 2000); // Second attempt: 1000 * 2^1

        // Reset
        retry_state.reset();
        assert_eq!(retry_state.attempts, 0);
        assert!(retry_state.should_retry_now());
    }
}
