/**
 * ============================================================================
 * RECORDING CLIENT
 * ============================================================================
 * 
 * PURPOSE: TypeScript client for multi-display screen recording functionality
 * 
 * FUNCTIONALITY:
 * - Start/stop screen recording (all displays)
 * - Get recording status and configuration
 * - Fetch recordings by date range
 * - Update recording configuration
 * 
 * ============================================================================
 */

import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';

/**
 * Configuration for screen recording
 */
export type RecordingConfig = {
  enabled: boolean;
  segment_duration_seconds: number;
  framerate: number;
  retention_days: number;
  max_storage_bytes: number;
  /** Output width for recordings (height calculated to maintain 16:9 aspect). Default: 1280 (720p) */
  output_width: number;
  /** CRF quality (0-51, lower = better quality, higher = smaller files). Default: 30 */
  crf: number;
  /** FFmpeg preset (ultrafast, superfast, veryfast, faster, fast, medium, slow). Default: "fast" */
  preset: string;
};

/**
 * Information about a monitor/display
 */
export type MonitorInfo = {
  id: number;
  width: number;
  height: number;
  x: number;
  y: number;
  scale_factor: number;
  is_primary: boolean;
};

/**
 * Per-display recording information within a segment
 */
export type DisplayRecording = {
  display_index: number;
  width: number;
  height: number;
  frame_count: number;
  file_size_bytes: number;
  filename: string;
};

/**
 * Metadata for a recording segment (may contain multiple displays)
 */
export type RecordingMetadata = {
  id: string;
  format: string;
  codec: string;
  framerate: number;
  start_time: string;
  end_time: string;
  duration_seconds: number;
  total_file_size_bytes: number;
  display_count: number;
  displays: DisplayRecording[];
};

/**
 * Recording status for display
 */
export type RecordingStatus = {
  enabled: boolean;
  is_recording: boolean;
  current_segment_id: string | null;
  current_segment_start: string | null;
  current_segment_duration_seconds: number | null;
  display_count: number;
  total_segments: number;
  total_storage_bytes: number;
};

/**
 * Response containing recordings for a time range
 */
export type RecordingsResponse = {
  recordings: RecordingMetadata[];
  total_count: number;
};

/**
 * Start screen recording (all displays)
 */
export async function startRecording(): Promise<void> {
  return await invoke('start_recording');
}

/**
 * Stop screen recording
 */
export async function stopRecording(): Promise<void> {
  return await invoke('stop_recording');
}

/**
 * Get current recording status
 */
export async function getRecordingStatus(): Promise<RecordingStatus> {
  return await invoke('get_recording_status');
}

/**
 * Get number of available displays
 */
export async function getDisplayCount(): Promise<number> {
  return await invoke('get_display_count');
}

/**
 * Get recordings within a date range
 */
export async function getRecordingsByDateRange(
  startTime: string,
  endTime: string
): Promise<RecordingsResponse> {
  return await invoke('get_recordings_by_date_range', {
    startTime,
    endTime,
  });
}

/**
 * Update recording configuration
 */
export async function updateRecordingConfig(
  config: RecordingConfig
): Promise<void> {
  return await invoke('update_recording_config', { newConfig: config });
}

/**
 * Get current recording configuration
 */
export async function getRecordingConfig(): Promise<RecordingConfig> {
  return await invoke('get_recording_config');
}

/**
 * Convert a local file path to a URL for display
 */
export function getRecordingUrl(filePath: string): string {
  return convertFileSrc(filePath);
}

/**
 * Format duration in seconds to human readable string
 */
export function formatDuration(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

/**
 * Format file size in bytes to human readable string
 */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/**
 * Default recording configuration
 * Optimized for Gemini AI analysis with small file sizes
 */
export const DEFAULT_RECORDING_CONFIG: RecordingConfig = {
  enabled: false,
  segment_duration_seconds: 300, // 5 minutes - optimized for Gemini chunk size
  framerate: 4,
  retention_days: 3,
  max_storage_bytes: 5_000_000_000,
  output_width: 1280, // 720p width - good balance for AI analysis
  crf: 30, // Higher compression, acceptable for screen content
  preset: 'fast', // Good compression with reasonable CPU usage
};

// =============================================================================
// GEMINI AI INTEGRATION
// =============================================================================

/**
 * Configuration for Gemini AI analysis
 */
export type GeminiConfig = {
  enabled: boolean;
  rate_limit_per_minute: number;
  max_retries: number;
  retry_delay_seconds: number;
  thinking_budget: number;
};

/**
 * Queue statistics for Gemini processing
 */
export type GeminiQueueStats = {
  jobs_submitted: number;
  jobs_completed: number;
  jobs_failed: number;
  jobs_pending: number;
  last_error: string | null;
};

/**
 * Gemini queue status
 */
export type GeminiQueueStatus = {
  running: boolean;
  stats: GeminiQueueStats;
  config: GeminiConfig;
};

/**
 * Default Gemini configuration
 */
export const DEFAULT_GEMINI_CONFIG: GeminiConfig = {
  enabled: false,
  rate_limit_per_minute: 0, // No rate limiting
  max_retries: 3,
  retry_delay_seconds: 5,
  thinking_budget: 1024,
};

/**
 * Check if Gemini API key is available (user-provided, env var, or embedded at build time)
 */
export async function hasGeminiApiKey(): Promise<boolean> {
  return await invoke('has_gemini_api_key');
}

/**
 * Get Gemini API key status (whether a key is configured, not the key itself)
 */
export async function getGeminiApiKeyStatus(): Promise<boolean> {
  return await invoke('get_gemini_api_key_status');
}

/**
 * Set Gemini API key (user-provided from settings)
 */
export async function setGeminiApiKey(apiKey: string): Promise<void> {
  return await invoke('set_gemini_api_key', { apiKey });
}

/**
 * Delete Gemini API key (remove user-provided key)
 */
export async function deleteGeminiApiKey(): Promise<void> {
  return await invoke('delete_gemini_api_key');
}

/**
 * Get Gemini configuration
 */
export async function getGeminiConfig(): Promise<GeminiConfig> {
  return await invoke('get_gemini_config');
}

/**
 * Update Gemini configuration
 */
export async function updateGeminiConfig(config: GeminiConfig): Promise<void> {
  return await invoke('update_gemini_config', { newConfig: config });
}

/**
 * Get Gemini queue status
 */
export async function getGeminiQueueStatus(): Promise<GeminiQueueStatus> {
  return await invoke('get_gemini_queue_status');
}

// =============================================================================
// AUDIO FEATURE CONFIGURATION
// =============================================================================

/**
 * Configuration for audio recording and transcription
 */
export type AudioFeatureConfig = {
  enabled: boolean;
  transcription_enabled: boolean;
  transcription_model: string;
  transcription_max_retries: number;
  transcription_retry_delay_seconds: number;
  transcription_processing_delay_seconds: number;
};

/**
 * Default audio feature configuration
 */
export const DEFAULT_AUDIO_FEATURE_CONFIG: AudioFeatureConfig = {
  enabled: true,
  transcription_enabled: true,
  transcription_model: "tiny.en",
  transcription_max_retries: 3,
  transcription_retry_delay_seconds: 5,
  transcription_processing_delay_seconds: 5,
};

/**
 * Get audio feature configuration
 */
export async function getAudioFeatureConfig(): Promise<AudioFeatureConfig> {
  return await invoke('get_audio_feature_config');
}

/**
 * Update audio feature configuration
 */
export async function updateAudioFeatureConfig(
  config: AudioFeatureConfig
): Promise<void> {
  return await invoke('update_audio_feature_config', { newConfig: config });
}

// =============================================================================
// TRANSCRIPTION
// =============================================================================

/**
 * Queue statistics for transcription processing
 */
export type TranscriptionQueueStats = {
  jobs_submitted: number;
  jobs_completed: number;
  jobs_failed: number;
  jobs_pending: number;
  last_error: string | null;
  total_audio_processed_ms: number;
  total_processing_time_ms: number;
};

/**
 * Transcription queue status
 */
export type TranscriptionQueueStatus = {
  running: boolean;
  whisper_available: boolean;
  stats: TranscriptionQueueStats;
  config: {
    enabled: boolean;
    model: string;
    max_retries: number;
    retry_delay_seconds: number;
    processing_delay_seconds: number;
  };
};

/**
 * Check if Whisper is available
 */
export async function isWhisperAvailable(): Promise<boolean> {
  return await invoke('is_whisper_available');
}

/**
 * Get transcription queue status
 */
export async function getTranscriptionQueueStatus(): Promise<TranscriptionQueueStatus> {
  return await invoke('get_transcription_queue_status');
}
