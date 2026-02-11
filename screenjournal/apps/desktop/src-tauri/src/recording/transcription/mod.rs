/**
 * ============================================================================
 * TRANSCRIPTION MODULE
 * ============================================================================
 * 
 * PURPOSE: Local audio transcription using whisper.cpp (via whisper-rs)
 * 
 * SUBMODULES:
 * - types: Data structures for transcription jobs and results
 * - whisper: Whisper model integration for speech-to-text
 * - queue: Background job queue for async transcription processing
 * - storage: Transcript file I/O and path management
 * - formatter: InfluxDB line protocol formatting
 * 
 * ARCHITECTURE:
 * After each recording segment completes:
 * 1. Two transcription jobs are submitted (microphone + system audio)
 * 2. Jobs are processed sequentially in background to manage CPU load
 * 3. Results include word-level timestamps for detailed analysis
 * 4. Microphone audio is labeled as "employee" speech
 * 5. System audio is labeled as "customer/client" speech
 * 
 * MODEL:
 * - Uses bundled whisper-tiny.en.bin (~75MB) for fast, lightweight transcription
 * - Optimized for low-end hardware (~30s processing per 5-min segment)
 * 
 * OUTPUT:
 * - {segment_id}_d{display}.mic.transcript.json: Employee transcript
 * - {segment_id}_d{display}.audio.transcript.json: Customer transcript
 * - InfluxDB line protocol with local audio_path (absolute path)
 * 
 * ============================================================================
 */

pub mod formatter;
pub mod queue;
pub mod storage;
pub mod types;
pub mod whisper;

// Re-exports for convenient access
pub use queue::{get_queue_status, init_queue, shutdown_queue, submit_job, update_config, update_config_from_audio_feature, QueueStatus};
pub use types::{AudioSource, TranscriptionConfig, TranscriptionJob, TranscriptionResult};
