/**
 * ============================================================================
 * BATCH MANAGER MODULE
 * ============================================================================
 * 
 * PURPOSE: Accumulate events and trigger batch creation based on thresholds
 * 
 * BATCHING STRATEGY:
 * - Time-based: Flush after N seconds since last flush
 * - Size-based: Flush when batch reaches N events
 * - Trigger-based: Manual flush on demand (AFK, shutdown, etc.)
 * 
 * THREAD SAFETY:
 * - Global singleton with Mutex protection
 * - Safe concurrent access from multiple threads
 * 
 * ============================================================================
 */

use crate::collector::config::CollectorConfig;
use crate::collector::queue::OfflineQueue;
use crate::collector::types::{LineProtocolBatch, SyncStatistics};
use chrono::Utc;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::Instant;
use uuid::Uuid;

/**
 * Global batch manager instance
 * Access through BATCH_MANAGER static
 */
static BATCH_MANAGER: Lazy<Mutex<Option<BatchManager>>> = Lazy::new(|| Mutex::new(None));

/**
 * Batch manager state
 * Tracks current batch, timing, and statistics
 */
pub struct BatchManager {
    current_batch: Vec<String>,
    last_flush_time: Instant,
    config: CollectorConfig,
    offline_queue: OfflineQueue,
    statistics: SyncStatistics,
}

impl BatchManager {
    /**
     * Create new batch manager
     * Private constructor, use init() to create global instance
     */
    fn new(config: CollectorConfig, offline_queue: OfflineQueue) -> Self {
        Self {
            current_batch: Vec::new(),
            last_flush_time: Instant::now(),
            config,
            offline_queue,
            statistics: SyncStatistics::default(),
        }
    }

    /**
     * Add event to current batch
     * Checks if flush should be triggered
     */
    pub fn add_event(&mut self, line_protocol: String) {
        self.current_batch.push(line_protocol);
    }

    /**
     * Check if batch should be flushed
     * Returns true if size or time threshold is met
     */
    pub fn should_flush(&self) -> bool {
        // Size threshold
        if self.current_batch.len() >= self.config.batch_max_size {
            log::debug!("Batch size threshold reached ({} events)", self.current_batch.len());
            return true;
        }

        // Time threshold
        let elapsed = self.last_flush_time.elapsed();
        if elapsed.as_secs() >= self.config.batch_max_interval_seconds {
            if !self.current_batch.is_empty() {
                log::debug!("Batch time threshold reached ({} seconds)", elapsed.as_secs());
                return true;
            }
        }

        false
    }

    /**
     * Flush current batch and create LineProtocolBatch
     * Returns Some(batch) if batch is not empty, None if empty
     */
    pub fn flush(&mut self) -> Option<LineProtocolBatch> {
        if self.current_batch.is_empty() {
            log::debug!("Flush requested but batch is empty");
            return None;
        }

        let batch_id = Uuid::new_v4().to_string();
        let batch = LineProtocolBatch {
            batch_id: batch_id.clone(),
            created_at: Utc::now(),
            events: self.current_batch.drain(..).collect(),
        };

        let event_count = batch.events.len();
        self.last_flush_time = Instant::now();

        log::info!("Flushed batch {} with {} events", batch_id, event_count);
        Some(batch)
    }

    /**
     * Enqueue batch to offline queue
     * Called when transmission fails
     */
    pub fn enqueue_batch(&mut self, batch: LineProtocolBatch) -> Result<(), String> {
        self.offline_queue.enqueue(batch)
    }

    /**
     * Dequeue oldest batch from offline queue
     * Used for retry logic
     */
    pub fn dequeue_batch(&mut self) -> Result<Option<LineProtocolBatch>, String> {
        self.offline_queue.dequeue()
    }

    /**
     * Get number of pending events
     * Includes current batch + offline queue
     */
    pub fn pending_events(&self) -> usize {
        let current_batch_size = self.current_batch.len();
        let queued_batches = self.offline_queue.len();
        // Estimate: assume average batch size for queued batches
        current_batch_size + (queued_batches * self.config.batch_max_size / 2)
    }

    /**
     * Update statistics after successful send
     */
    pub fn record_success(&mut self, events_sent: usize) {
        self.statistics.total_events_sent += events_sent as u64;
        self.statistics.total_batches_sent += 1;
        self.statistics.last_sync_time = Some(Utc::now());
        self.statistics.retry_attempts = 0; // Reset retry counter on success
        self.statistics.last_error = None;
        log::debug!("Updated statistics: {} events sent, {} batches total", 
            events_sent, self.statistics.total_batches_sent);
    }

    /**
     * Record error in statistics
     */
    pub fn record_error(&mut self, error: String) {
        self.statistics.last_error = Some(error.clone());
        log::warn!("Recorded error: {}", error);
    }

    /**
     * Increment retry attempt counter
     */
    pub fn increment_retry_attempts(&mut self) {
        self.statistics.retry_attempts += 1;
    }

    /**
     * Get current statistics snapshot
     */
    pub fn get_statistics(&self) -> SyncStatistics {
        let mut stats = self.statistics.clone();
        stats.pending_events = self.pending_events();
        stats
    }

    /**
     * Clear offline queue
     * Used when user manually clears queue
     */
    pub fn clear_queue(&mut self) -> Result<(), String> {
        self.offline_queue.clear()
    }

    /**
     * Get current batch size
     */
    pub fn current_batch_size(&self) -> usize {
        self.current_batch.len()
    }
}

/**
 * Initialize global batch manager
 * Must be called before using add_event or other functions
 */
pub fn init(app_handle: &tauri::AppHandle, config: CollectorConfig) -> Result<(), String> {
    let offline_queue = OfflineQueue::new(app_handle, config.offline_queue_max_batches)?;
    let manager = BatchManager::new(config, offline_queue);
    
    let mut global = BATCH_MANAGER.lock().unwrap();
    *global = Some(manager);
    
    log::info!("Batch manager initialized");
    Ok(())
}

/**
 * Add event to global batch manager
 * Thread-safe, can be called from any thread
 */
pub fn add_event(line_protocol: String) -> Result<(), String> {
    let mut global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_mut()
        .ok_or_else(|| {
            log::error!("Batch manager not initialized - cannot add event");
            "Batch manager not initialized".to_string()
        })?;
    
    manager.add_event(line_protocol);
    Ok(())
}

/**
 * Check if global batch manager should flush
 */
pub fn should_flush() -> Result<bool, String> {
    let global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_ref()
        .ok_or_else(|| "Batch manager not initialized".to_string())?;
    
    Ok(manager.should_flush())
}

/**
 * Flush current batch from global batch manager
 */
pub fn flush() -> Result<Option<LineProtocolBatch>, String> {
    let mut global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_mut()
        .ok_or_else(|| "Batch manager not initialized".to_string())?;
    
    Ok(manager.flush())
}

/**
 * Enqueue batch to offline queue
 */
pub fn enqueue_batch(batch: LineProtocolBatch) -> Result<(), String> {
    let mut global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_mut()
        .ok_or_else(|| "Batch manager not initialized".to_string())?;
    
    manager.enqueue_batch(batch)
}

/**
 * Dequeue batch from offline queue
 */
pub fn dequeue_batch() -> Result<Option<LineProtocolBatch>, String> {
    let mut global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_mut()
        .ok_or_else(|| "Batch manager not initialized".to_string())?;
    
    manager.dequeue_batch()
}

/**
 * Record successful transmission
 */
pub fn record_success(events_sent: usize) -> Result<(), String> {
    let mut global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_mut()
        .ok_or_else(|| "Batch manager not initialized".to_string())?;
    
    manager.record_success(events_sent);
    Ok(())
}

/**
 * Record error
 */
pub fn record_error(error: String) -> Result<(), String> {
    let mut global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_mut()
        .ok_or_else(|| "Batch manager not initialized".to_string())?;
    
    manager.record_error(error);
    Ok(())
}

/**
 * Increment retry attempts
 */
pub fn increment_retry_attempts() -> Result<(), String> {
    let mut global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_mut()
        .ok_or_else(|| "Batch manager not initialized".to_string())?;
    
    manager.increment_retry_attempts();
    Ok(())
}

/**
 * Get current statistics
 */
pub fn get_statistics() -> Result<SyncStatistics, String> {
    let global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_ref()
        .ok_or_else(|| "Batch manager not initialized".to_string())?;
    
    Ok(manager.get_statistics())
}

/**
 * Clear offline queue
 */
pub fn clear_queue() -> Result<(), String> {
    let mut global = BATCH_MANAGER.lock().unwrap();
    let manager = global.as_mut()
        .ok_or_else(|| "Batch manager not initialized".to_string())?;
    
    manager.clear_queue()
}

/**
 * Shutdown batch manager
 * Clears global instance
 */
pub fn shutdown() {
    let mut global = BATCH_MANAGER.lock().unwrap();
    *global = None;
    log::info!("Batch manager shutdown");
}

