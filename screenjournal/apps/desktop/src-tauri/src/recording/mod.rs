/**
 * ============================================================================
 * RECORDING MODULE
 * ============================================================================
 * 
 * PURPOSE: Multi-display MP4 screen recording system using scap + bundled FFmpeg
 * 
 * SUBMODULES:
 * - capture: Per-display frame capture and MP4 encoding via bundled FFmpeg
 * - config: Configuration persistence and management
 * - manager: Lifecycle management and Tauri commands
 * - storage: Video file storage and cleanup
 * - types: Data structures and models
 * - gemini: AI-powered video analysis using Google Gemini
 * 
 * ARCHITECTURE:
 * The recording system captures all displays simultaneously:
 * 1. One capture thread per display: Each grabs frames via scap
 * 2. Each thread pipes to bundled FFmpeg process for H.264 encoding
 * 3. Segment rotation: Finalizes all displays, starts new segment (60s default)
 * 4. Combined metadata sidecar: JSON file with per-display info
 * 5. Gemini analysis: After segment completion, video sent to AI for timeline extraction
 * 
 * OUTPUT FORMAT:
 * - segment_*_d0.mp4, segment_*_d1.mp4, ...: One H.264 MP4 per display
 * - segment_*.json: Combined metadata sidecar
 * 
 * REQUIREMENTS:
 * - FFmpeg binary bundled in resources/ffmpeg/{platform}/{arch}/
 * - Run 'npm run setup-ffmpeg' during development setup
 * - capture::init_ffmpeg_path() must be called on app startup
 * - Gemini API key stored in OS keychain for AI analysis
 * 
 * ============================================================================
 */

pub mod capture;
pub mod config;
pub mod gemini;
pub mod manager;
pub mod microphone;
pub mod storage;
pub mod transcription;
pub mod types;
pub mod upload;