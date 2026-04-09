/**
 * ============================================================================
 * COLLECTOR MODULE
 * ============================================================================
 * 
 * PURPOSE: Data transmission system for sending ActivityWatch events to the
 * sj-collector server via WebSocket with InfluxDB line protocol.
 * 
 * ARCHITECTURE:
 * - config: Configuration management and persistence
 * - types: Data structures and models
 * - formatter: Line protocol formatting
 * - auth: JWT authentication client
 * - client: WebSocket client implementation
 * - queue: Persistent offline queue
 * - batch: Batch management and accumulation
 * - manager: High-level orchestration and Tauri commands
 * 
 * AUTHOR: ScreenRecord Development Team
 * CREATED: 2025-01-19
 * ============================================================================
 */

pub mod config;
pub mod types;
pub mod formatter;
pub mod auth;
pub mod client;
pub mod queue;
pub mod batch;
pub mod manager;
pub mod bridge;

