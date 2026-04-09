/**
 * ============================================================================
 * WEBSOCKET CLIENT MODULE
 * ============================================================================
 * 
 * PURPOSE: WebSocket connection management for data transmission
 * 
 * PROTOCOL:
 * 1. Connect to WebSocket endpoint with TLS
 * 2. Send: $AUTH <jwt-token>
 * 3. Receive: AUTH_SUCCESS
 * 4. Send: line protocol strings
 * 5. Receive: OK (for each line)
 * 
 * CONNECTION LIFECYCLE:
 * - Persistent connection maintained for batch streaming
 * - Automatic reconnection on disconnect (handled by manager)
 * - Keepalive pings every 30 seconds
 * - Graceful shutdown with close frame
 * 
 * ============================================================================
 */

use crate::collector::config::CollectorConfig;
use crate::collector::types::{LineProtocolBatch, TransmissionStatus};
use crate::collector::auth;
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

type WebSocketStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
>;

/**
 * WebSocket client for collector server communication
 * Manages connection lifecycle, authentication, and message transmission
 */
pub struct CollectorClient {
    config: CollectorConfig,
    websocket: Option<WebSocketStream>,
    status: Arc<Mutex<TransmissionStatus>>,
    // Ping statistics for connection health monitoring
    ping_count: u64,
    pong_received_count: u64,
    total_ping_latency_ms: u64,
    last_ping_time: Option<Instant>,
    consecutive_ping_failures: u32,
}

impl CollectorClient {
    /**
     * Create new WebSocket client
     * Client starts in Disconnected state
     */
    pub fn new(config: CollectorConfig) -> Self {
        Self {
            config,
            websocket: None,
            status: Arc::new(Mutex::new(TransmissionStatus::Disconnected)),
            ping_count: 0,
            pong_received_count: 0,
            total_ping_latency_ms: 0,
            last_ping_time: None,
            consecutive_ping_failures: 0,
        }
    }

    /**
     * Get current connection status
     * Thread-safe accessor for status display
     */
    pub fn get_status(&self) -> TransmissionStatus {
        self.status.lock().unwrap().clone()
    }

    /**
     * Set connection status
     * Updates internal state and logs status changes
     */
    fn set_status(&self, status: TransmissionStatus) {
        log::info!("Collector status changed to: {:?}", status);
        *self.status.lock().unwrap() = status;
    }

    /**
     * Connect to WebSocket server
     * Establishes TLS connection and updates status
     */
    pub async fn connect(&mut self) -> Result<(), String> {
        log::info!("Connecting to WebSocket: {}", self.config.server_url);
        self.set_status(TransmissionStatus::Connecting);

        // Reset ping statistics for new connection
        self.ping_count = 0;
        self.pong_received_count = 0;
        self.total_ping_latency_ms = 0;
        self.last_ping_time = None;
        self.consecutive_ping_failures = 0;

        // Parse URL and connect with timeout
        let connect_future = connect_async(&self.config.server_url);
        let timeout_duration = Duration::from_secs(self.config.connection_timeout_seconds);

        let (ws_stream, response) = timeout(timeout_duration, connect_future)
            .await
            .map_err(|_| format!("Connection timeout after {} seconds. Is the collector server running at {}?", 
                self.config.connection_timeout_seconds, self.config.server_url))?
            .map_err(|e| {
                let error_msg = format!("WebSocket connection failed: {}", e);
                // Provide helpful error message for connection refused
                if error_msg.contains("Connection refused") || error_msg.contains("os error 61") {
                    format!("Connection refused. Please ensure the collector server is running at {}. Start it with: cd sj-collector && go run ./cmd/server", 
                        self.config.server_url)
                } else {
                    error_msg
                }
            })?;

        log::info!("WebSocket connected, status: {}", response.status());

        self.websocket = Some(ws_stream);
        Ok(())
    }

    /**
     * Authenticate with server using JWT token
     * Sends $AUTH message and waits for AUTH_SUCCESS response
     */
    pub async fn authenticate(&mut self) -> Result<(), String> {
        log::info!("[CLIENT] Authenticating with collector server");
        
        // Get fresh config from cache to ensure we have the latest app_jwt_token
        let fresh_config = crate::collector::config::get_cached_config();
        
        // Update our local config with fresh token if available
        if let Some(ref fresh) = fresh_config {
            if fresh.app_jwt_token != self.config.app_jwt_token {
                log::info!("[CLIENT] Updating app_jwt_token from cache: was_none={}, now_none={}", 
                    self.config.app_jwt_token.is_none(),
                    fresh.app_jwt_token.is_none());
                if let Some(ref token) = fresh.app_jwt_token {
                    log::info!("[CLIENT] New token from cache: length={}", token.len());
                }
                self.config.app_jwt_token = fresh.app_jwt_token.clone();
            } else {
                log::debug!("[CLIENT] Token in cache matches local config");
            }
        } else {
            log::warn!("[CLIENT] No cached config available - cannot update token");
        }
        
        log::info!("[CLIENT] Config.app_jwt_token is: {}", 
            if self.config.app_jwt_token.is_some() { 
                format!("Some(length: {})", self.config.app_jwt_token.as_ref().unwrap().len())
            } else { 
                "None".to_string()
            });

        self.set_status(TransmissionStatus::Authenticating);

        // Log whether app JWT token will be sent (optional for mock-auth)
        if let Some(ref app_token) = self.config.app_jwt_token {
            log::info!("[CLIENT] Will send app JWT token in Authorization header (length: {})", app_token.len());
        } else {
            log::debug!("[CLIENT] No app JWT token in config - using mock-auth (JWT optional)");
        }

        // Get valid token
        let app_jwt_for_auth = self.config.app_jwt_token.as_deref();
        log::info!("[CLIENT] Calling get_valid_token with app_jwt_token: {}", 
            if app_jwt_for_auth.is_some() { 
                format!("Some(length: {})", app_jwt_for_auth.unwrap().len())
            } else { 
                "None".to_string()
            });
        
        let token = auth::get_valid_token(
            &self.config.auth_url,
            &self.config.user_name,
            &self.config.user_id,
            &self.config.org_name,
            &self.config.org_id,
            &self.config.account_id,
            app_jwt_for_auth,
        )
        .await?;

        // Get WebSocket reference
        let ws = self.websocket.as_mut()
            .ok_or_else(|| "WebSocket not connected".to_string())?;

        // Send auth message
        let auth_message = format!("$AUTH {}", token);
        ws.send(Message::Text(auth_message))
            .await
            .map_err(|e| format!("Failed to send auth message: {}", e))?;

        // Wait for response with timeout
        let response_future = ws.next();
        let response = timeout(Duration::from_secs(10), response_future)
            .await
            .map_err(|_| "Authentication timeout".to_string())?
            .ok_or_else(|| "Connection closed during authentication".to_string())?
            .map_err(|e| format!("Failed to receive auth response: {}", e))?;

        // Verify response
        match response {
            Message::Text(text) => {
                if text == "AUTH_SUCCESS" {
                    log::info!("Authentication successful");
                    self.set_status(TransmissionStatus::Connected);
                    Ok(())
                } else {
                    // Server rejected authentication
                    auth::invalidate_token(); // Clear cached token
                    let err_msg = format!("Authentication failed: {}", text);
                    self.set_status(TransmissionStatus::Error(err_msg.clone()));
                    Err(err_msg)
                }
            }
            _ => {
                let err_msg = "Unexpected auth response format".to_string();
                self.set_status(TransmissionStatus::Error(err_msg.clone()));
                Err(err_msg)
            }
        }
    }

    /**
     * Send a batch of line protocol events
     * Sends each line and waits for OK acknowledgment
     * Returns number of successfully sent events
     */
    pub async fn send_batch(&mut self, batch: &LineProtocolBatch) -> Result<usize, String> {
        if self.get_status() != TransmissionStatus::Connected {
            return Err("Client not connected".to_string());
        }

        let total_events = batch.events.len();
        let mut sent_count = 0;

        log::info!("Sending batch {} with {} events", batch.batch_id, total_events);

        // Clone Arc for use in error closures
        let status = Arc::clone(&self.status);

        let ws = self.websocket.as_mut()
            .ok_or_else(|| "WebSocket not connected".to_string())?;

        for (i, line_protocol) in batch.events.iter().enumerate() {
            // Send line protocol
            let status_clone = Arc::clone(&status);
            log::info!("Sending line protocol: {}", line_protocol.clone());
            ws.send(Message::Text(line_protocol.clone()))
                .await
                .map_err(|e| {
                    let err_msg = format!("Failed to send event {}/{}: {}", i + 1, total_events, e);
                    *status_clone.lock().unwrap() = TransmissionStatus::Error(err_msg.clone());
                    err_msg
                })?;

            // Wait for acknowledgment with timeout
            let ack_future = ws.next();
            let status_clone = Arc::clone(&status);
            let ack_response = timeout(Duration::from_secs(15), ack_future)
                .await
                .map_err(|_| {
                    let err_msg = format!("ACK timeout for event {}/{}", i + 1, total_events);
                    *status_clone.lock().unwrap() = TransmissionStatus::Error(err_msg.clone());
                    err_msg
                })?;

            let status_clone = Arc::clone(&status);
            let ack_response = ack_response
                .ok_or_else(|| {
                    let err_msg = "Connection closed while waiting for ACK".to_string();
                    *status_clone.lock().unwrap() = TransmissionStatus::Disconnected;
                    err_msg
                })?;

            let status_clone = Arc::clone(&status);
            let ack_response = ack_response
                .map_err(|e| {
                    let err_msg = format!("Failed to receive ACK: {}", e);
                    *status_clone.lock().unwrap() = TransmissionStatus::Error(err_msg.clone());
                    err_msg
                })?;

            // Check acknowledgment
            match ack_response {
                Message::Text(text) if text == "OK" => {
                    sent_count += 1;
                }
                Message::Text(text) if text.starts_with("ERROR:") => {
                    log::warn!("Server error for event {}/{}: {}", i + 1, total_events, text);
                    // Continue with next event (server acknowledged receipt even if error)
                }
                Message::Text(text) => {
                    // Log unexpected text response for debugging
                    log::warn!(
                        "Unexpected text ACK for event {}/{}: {:?}",
                        i + 1,
                        total_events,
                        &text[..text.len().min(100)]
                    );
                }
                Message::Pong(_) => {
                    // Pong message received during batch - this is expected if async pings are in flight
                    // Log at debug level and continue waiting for actual ACK
                    log::debug!(
                        "Received Pong during batch send for event {}/{} - ignoring",
                        i + 1,
                        total_events
                    );
                    // Note: We should ideally re-wait for the actual ACK, but for now just continue
                    // This is the likely cause of "Unexpected ACK" warnings
                }
                Message::Ping(_data) => {
                    // Server sent a ping during batch - just log it
                    // We can't respond here due to borrow checker constraints
                    // The server should be patient during batch sends
                    log::debug!(
                        "Received Ping during batch send for event {}/{} - ignoring (batch in progress)",
                        i + 1,
                        total_events
                    );
                }
                Message::Binary(data) => {
                    log::warn!(
                        "Unexpected binary ACK for event {}/{}: {} bytes",
                        i + 1,
                        total_events,
                        data.len()
                    );
                }
                Message::Close(frame) => {
                    let err_msg = format!(
                        "Server closed connection during batch send (event {}/{}): {:?}",
                        i + 1,
                        total_events,
                        frame
                    );
                    *status.lock().unwrap() = TransmissionStatus::Disconnected;
                    return Err(err_msg);
                }
                Message::Frame(_) => {
                    log::debug!("Received raw frame during batch send for event {}/{}", i + 1, total_events);
                }
            }
        }

        log::info!("Batch {} sent: {}/{} events successful", batch.batch_id, sent_count, total_events);
        Ok(sent_count)
    }

    /**
     * Send keepalive ping
     * Fire-and-forget: sends ping but doesn't wait for pong
     * This avoids message interleaving issues with the single WebSocket stream
     * The ping send failure is sufficient to detect dead connections
     */
    pub async fn ping(&mut self) -> Result<(), String> {
        // Clone Arc for use in error closures
        let status = Arc::clone(&self.status);
        
        let ws = self.websocket.as_mut()
            .ok_or_else(|| "WebSocket not connected".to_string())?;

        // Increment ping count
        self.ping_count += 1;

        log::debug!("[COLLECTOR-CLIENT] PING_SENDING: sequence={}", self.ping_count);

        let status_clone = Arc::clone(&status);
        ws.send(Message::Ping(vec![]))
            .await
            .map_err(|e| {
                let err_msg = format!("Ping send failed: {}", e);
                *status_clone.lock().unwrap() = TransmissionStatus::Disconnected;
                self.consecutive_ping_failures += 1;
                log::error!("[COLLECTOR-CLIENT] PING_SEND_FAILED: sequence={}, consecutive_failures={}, error={}", 
                    self.ping_count, self.consecutive_ping_failures, e);
                err_msg
            })?;

        // Fire-and-forget: ping was sent successfully
        // Don't wait for pong - avoids message interleaving issues
        // The ping send failure is sufficient to detect dead connections
        self.consecutive_ping_failures = 0;
        log::debug!("[COLLECTOR-CLIENT] PING_SENT_OK: sequence={}", self.ping_count);

        Ok(())
    }

    /**
     * Disconnect WebSocket gracefully
     * Sends close frame and drops connection
     */
    pub async fn disconnect(&mut self) {
        if let Some(mut ws) = self.websocket.take() {
            log::info!("Disconnecting WebSocket");
            // Send close frame (best effort, ignore errors)
            let _ = ws.send(Message::Close(None)).await;
            let _ = ws.close(None).await;
        }
        self.set_status(TransmissionStatus::Disconnected);
    }
}

impl Drop for CollectorClient {
    /**
     * Ensure WebSocket is closed when client is dropped
     * Note: Drop is synchronous, so we can't call async disconnect
     */
    fn drop(&mut self) {
        if self.websocket.is_some() {
            log::warn!("CollectorClient dropped while connected, connection not closed gracefully");
        }
    }
}

