/**
 * ============================================================================
 * OFFLINE QUEUE MODULE
 * ============================================================================
 * 
 * PURPOSE: Persist unsent batches to disk for offline resilience
 * 
 * STORAGE STRUCTURE:
 * collector_queue/
 * ├── batch_<uuid1>.json
 * ├── batch_<uuid2>.json
 * └── queue_index.json  (contains ordered list of batch IDs)
 * 
 * QUEUE BEHAVIOR:
 * - FIFO (First In First Out) ordering
 * - Size-limited (oldest batches dropped when full)
 * - Atomic writes (temp file + rename)
 * - Automatic index rebuild on corruption
 * 
 * ============================================================================
 */

use crate::collector::types::LineProtocolBatch;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/**
 * Queue index metadata
 * Tracks ordered list of batch IDs and queue state
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueueIndex {
    batch_order: Vec<String>, // Ordered list of batch IDs (oldest first)
}

impl Default for QueueIndex {
    fn default() -> Self {
        Self {
            batch_order: Vec::new(),
        }
    }
}

/**
 * Persistent offline queue for batch storage
 * Maintains FIFO queue of batches on disk
 */
pub struct OfflineQueue {
    queue_dir: PathBuf,
    max_batches: usize,
    batch_order: Vec<String>,
}

impl OfflineQueue {
    /**
     * Create new offline queue
     * Initializes directory structure and loads existing queue state
     */
    pub fn new(app_handle: &AppHandle, max_batches: usize) -> Result<Self, String> {
        let queue_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to resolve app data directory: {}", e))?
            .join("collector_queue");

        // Create queue directory if it doesn't exist
        if !queue_dir.exists() {
            fs::create_dir_all(&queue_dir)
                .map_err(|e| format!("Failed to create queue directory: {}", e))?;
            log::info!("Created queue directory: {}", queue_dir.display());
        }

        // Load existing queue index
        let index_path = queue_dir.join("queue_index.json");
        let batch_order = if index_path.exists() {
            match Self::load_index(&index_path) {
                Ok(index) => {
                    log::info!("Loaded queue index with {} batches", index.batch_order.len());
                    index.batch_order
                }
                Err(e) => {
                    log::warn!("Failed to load queue index, rebuilding: {}", e);
                    Self::rebuild_index(&queue_dir)?
                }
            }
        } else {
            log::info!("No existing queue index, starting with empty queue");
            Vec::new()
        };

        Ok(Self {
            queue_dir,
            max_batches,
            batch_order,
        })
    }

    /**
     * Load queue index from disk
     */
    fn load_index(index_path: &PathBuf) -> Result<QueueIndex, String> {
        let json_str = fs::read_to_string(index_path)
            .map_err(|e| format!("Failed to read index file: {}", e))?;
        
        let index: QueueIndex = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse index JSON: {}", e))?;
        
        Ok(index)
    }

    /**
     * Save queue index to disk atomically
     */
    fn save_index(&self) -> Result<(), String> {
        let index_path = self.queue_dir.join("queue_index.json");
        let temp_path = index_path.with_extension("json.tmp");

        let index = QueueIndex {
            batch_order: self.batch_order.clone(),
        };

        let json_str = serde_json::to_string_pretty(&index)
            .map_err(|e| format!("Failed to serialize index: {}", e))?;

        fs::write(&temp_path, json_str)
            .map_err(|e| format!("Failed to write temp index: {}", e))?;

        fs::rename(&temp_path, &index_path)
            .map_err(|e| format!("Failed to save index: {}", e))?;

        Ok(())
    }

    /**
     * Rebuild queue index by scanning directory
     * Used when index file is corrupted or missing
     */
    fn rebuild_index(queue_dir: &PathBuf) -> Result<Vec<String>, String> {
        let mut batch_ids = Vec::new();

        let entries = fs::read_dir(queue_dir)
            .map_err(|e| format!("Failed to read queue directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            // Look for batch_*.json files
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("batch_") && filename.ends_with(".json") {
                    // Extract batch ID from filename
                    let batch_id = filename
                        .strip_prefix("batch_")
                        .and_then(|s| s.strip_suffix(".json"))
                        .unwrap_or("")
                        .to_string();
                    
                    if !batch_id.is_empty() {
                        batch_ids.push(batch_id);
                    }
                }
            }
        }

        // Sort by filename (approximation of creation order)
        batch_ids.sort();

        log::info!("Rebuilt queue index with {} batches", batch_ids.len());
        Ok(batch_ids)
    }

    /**
     * Enqueue a batch for offline storage
     * If queue is at max capacity, removes oldest batch first
     */
    pub fn enqueue(&mut self, batch: LineProtocolBatch) -> Result<(), String> {
        // Check if at capacity, remove oldest if needed
        if self.batch_order.len() >= self.max_batches {
            log::warn!("Queue at max capacity ({}), removing oldest batch", self.max_batches);
            if let Err(e) = self.dequeue() {
                log::error!("Failed to remove oldest batch: {}", e);
            }
        }

        // Serialize batch to JSON
        let batch_id = batch.batch_id.clone();
        let json_str = serde_json::to_string_pretty(&batch)
            .map_err(|e| format!("Failed to serialize batch: {}", e))?;

        // Write to file atomically
        let batch_path = self.queue_dir.join(format!("batch_{}.json", batch_id));
        let temp_path = batch_path.with_extension("json.tmp");

        fs::write(&temp_path, json_str)
            .map_err(|e| format!("Failed to write batch file: {}", e))?;

        fs::rename(&temp_path, &batch_path)
            .map_err(|e| format!("Failed to save batch file: {}", e))?;

        // Add to queue order
        self.batch_order.push(batch_id.clone());

        // Save updated index
        self.save_index()?;

        log::info!("Enqueued batch {} to offline queue ({} total)", batch_id, self.batch_order.len());
        Ok(())
    }

    /**
     * Dequeue oldest batch from queue
     * Returns Some(batch) if queue not empty, None if empty
     */
    pub fn dequeue(&mut self) -> Result<Option<LineProtocolBatch>, String> {
        if self.batch_order.is_empty() {
            return Ok(None);
        }

        // Remove first batch ID (oldest)
        let batch_id = self.batch_order.remove(0);
        let batch_path = self.queue_dir.join(format!("batch_{}.json", batch_id));

        // Read and deserialize batch
        let json_str = fs::read_to_string(&batch_path)
            .map_err(|e| format!("Failed to read batch file: {}", e))?;

        let batch: LineProtocolBatch = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse batch JSON: {}", e))?;

        // Delete batch file
        fs::remove_file(&batch_path)
            .map_err(|e| format!("Failed to delete batch file: {}", e))?;

        // Save updated index
        self.save_index()?;

        log::info!("Dequeued batch {} from offline queue ({} remaining)", batch_id, self.batch_order.len());
        Ok(Some(batch))
    }

    /**
     * Get number of batches in queue
     */
    pub fn len(&self) -> usize {
        self.batch_order.len()
    }

    /**
     * Check if queue is empty
     */
    pub fn is_empty(&self) -> bool {
        self.batch_order.is_empty()
    }

    /**
     * Clear all batches from queue
     * Deletes all batch files and resets index
     */
    pub fn clear(&mut self) -> Result<(), String> {
        log::info!("Clearing offline queue ({} batches)", self.batch_order.len());

        // Delete all batch files
        for batch_id in &self.batch_order {
            let batch_path = self.queue_dir.join(format!("batch_{}.json", batch_id));
            if let Err(e) = fs::remove_file(&batch_path) {
                log::warn!("Failed to delete batch file {}: {}", batch_id, e);
            }
        }

        // Clear order and save index
        self.batch_order.clear();
        self.save_index()?;

        log::info!("Offline queue cleared");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_batch(id: &str) -> LineProtocolBatch {
        LineProtocolBatch {
            batch_id: id.to_string(),
            created_at: Utc::now(),
            events: vec![
                "test_measurement field=1i 1234567890000000000".to_string(),
            ],
        }
    }

    #[test]
    fn test_queue_index_serialization() {
        let index = QueueIndex {
            batch_order: vec!["batch1".to_string(), "batch2".to_string()],
        };

        let json = serde_json::to_string(&index).unwrap();
        assert!(json.contains("\"batch1\""));
        assert!(json.contains("\"batch2\""));

        let deserialized: QueueIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.batch_order.len(), 2);
    }
}

