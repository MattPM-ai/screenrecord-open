/**
 * ============================================================================
 * RECORDING CAPTURE MODULE
 * ============================================================================
 * 
 * PURPOSE: Multi-display screen capture and MP4 encoding using scap + FFmpeg
 * 
 * FUNCTIONALITY:
 * - Check platform support and permissions
 * - Enumerate all available displays
 * - Capture frames from multiple displays simultaneously
 * - Pipe BGRA frames to bundled FFmpeg for H.264 MP4 encoding
 * 
 * OUTPUT FORMAT:
 * - One MP4 file per display with H.264 video codec
 * - JSON sidecar: Metadata including dimensions, framerate, frame count
 * 
 * REQUIREMENTS:
 * - FFmpeg binary must be bundled in resources/ffmpeg/{platform}/{arch}/
 * - Call init_ffmpeg_path() on app startup to initialize the path
 * 
 * ============================================================================
 */

// Microphone capture imports - using full paths to avoid import issues
// use crate::recording::microphone::{
//     MicrophoneCapture, convert_mic_f32_to_s16_bytes, write_mic_wav_header, finalize_mic_wav_header
// };
use crate::recording::types::MonitorInfo;
use once_cell::sync::Lazy;
use scap::{
    capturer::{Capturer, Options},
    frame::{AudioFormat, Frame, FrameType, VideoFrame},
    Target,
};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

// =============================================================================
// Audio Capture Role (for multi-display shared audio)
// =============================================================================

/// Paths to shared audio files from the primary display (display 0)
#[derive(Clone)]
pub struct SharedAudioPaths {
    /// Path to display 0's system audio WAV file
    pub system_audio_path: PathBuf,
    /// Path to display 0's microphone audio WAV file
    pub mic_audio_path: PathBuf,
}

/// Defines whether a display captures its own audio or uses shared audio
/// 
/// - Primary (display 0): Captures audio and signals when ready
/// - Secondary (displays 1+): Waits for primary's audio and uses shared paths
pub enum AudioCaptureRole {
    /// Display 0: Captures audio and sets the ready signal when done
    Primary {
        /// Signal to set when audio files are finalized
        audio_ready_signal: Arc<AtomicBool>,
        /// Signal to set if audio capture fails
        audio_failed_signal: Arc<AtomicBool>,
    },
    /// Displays 1+: Uses shared audio from display 0
    Secondary {
        /// Paths to display 0's audio files
        shared_audio: SharedAudioPaths,
        /// Signal to wait for before muxing
        audio_ready_signal: Arc<AtomicBool>,
        /// Signal indicating audio capture failed
        audio_failed_signal: Arc<AtomicBool>,
    },
}

// =============================================================================
// FFmpeg Path Management
// =============================================================================

// Global FFmpeg binary path - initialized on app startup
static FFMPEG_PATH: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

// Get platform-specific FFmpeg binary name and subdirectory
fn ffmpeg_platform_info() -> (&'static str, &'static str) {
    #[cfg(target_os = "windows")]
    {
        ("windows/x86_64", "ffmpeg.exe")
    }
    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        {
            ("darwin/aarch64", "ffmpeg")
        }
        #[cfg(target_arch = "x86_64")]
        {
            ("darwin/x86_64", "ffmpeg")
        }
    }
    #[cfg(target_os = "linux")]
    {
        ("linux/x86_64", "ffmpeg")
    }
}

// Resolve the bundled FFmpeg binary path
// 
// Searches in order:
// 1. DEV: src-tauri/resources/ffmpeg/{platform}/{arch}/ffmpeg
// 2. DEV: resources/ffmpeg/{platform}/{arch}/ffmpeg (when cwd is src-tauri)
// 3. PROD: {resource_dir}/ffmpeg/{platform}/{arch}/ffmpeg
fn resolve_ffmpeg_path(app: &AppHandle) -> PathBuf {
    let (platform_subdir, bin_name) = ffmpeg_platform_info();
    
    // Helper to build path from a root
    let build_path = |root: PathBuf| -> PathBuf {
        root.join(platform_subdir).join(bin_name)
    };
    
    // DEV: Try project paths first
    if cfg!(debug_assertions) {
        // 1) src-tauri/resources/ffmpeg path from project root
        let candidate1 = build_path(
            PathBuf::from("src-tauri")
                .join("resources")
                .join("ffmpeg"),
        );
        if candidate1.exists() {
            log::info!("FFmpeg found at dev path: {:?}", candidate1);
            return candidate1;
        }
        
        // 2) resources/ffmpeg path when cwd is src-tauri
        let candidate2 = build_path(PathBuf::from("resources").join("ffmpeg"));
        if candidate2.exists() {
            log::info!("FFmpeg found at dev path: {:?}", candidate2);
            return candidate2;
        }
        
        // 3) Try Tauri resource resolver
        if let Ok(p) = app.path().resolve(
            format!("ffmpeg/{}/{}", platform_subdir, bin_name),
            tauri::path::BaseDirectory::Resource,
        ) {
            if p.exists() {
                log::info!("FFmpeg found via Tauri resolver: {:?}", p);
                return p;
            }
        }
    }
    
    // PROD: Use packaged resource dir
    let prod_path = build_path(
        app.path()
            .resource_dir()
            .expect("resource_dir available")
            .join("ffmpeg"),
    );
    
    log::info!("FFmpeg path (prod): {:?}", prod_path);
    prod_path
}

// Initialize FFmpeg path on app startup
// 
// Must be called from the Tauri setup hook before recording is used.
pub fn init_ffmpeg_path(app: &AppHandle) {
    let path = resolve_ffmpeg_path(app);
    log::info!("Initializing FFmpeg path: {:?}", path);
    
    let mut ffmpeg_path = FFMPEG_PATH.lock().unwrap();
    *ffmpeg_path = Some(path);
}

// Get the stored FFmpeg binary path
pub fn get_ffmpeg_path() -> Result<PathBuf, String> {
    let path_guard = FFMPEG_PATH.lock().unwrap();
    path_guard.clone().ok_or_else(|| {
        "FFmpeg path not initialized. Call init_ffmpeg_path() on app startup.".to_string()
    })
}

// =============================================================================
// Platform Support Checks
// =============================================================================

// Check if screen capture is supported on this platform
pub fn is_supported() -> bool {
    scap::is_supported()
}

// Check if we have screen recording permission
pub fn has_permission() -> bool {
    scap::has_permission()
}

// Request screen recording permission (opens system dialog on macOS)
pub fn request_permission() -> bool {
    scap::request_permission()
}

// Check if bundled FFmpeg is available and working
pub fn check_ffmpeg() -> Result<(), String> {
    let ffmpeg_path = get_ffmpeg_path()?;
    
    // Check if binary exists
    if !ffmpeg_path.exists() {
        return Err(format!(
            "FFmpeg binary not found at {:?}. Run 'npm run setup-ffmpeg' to install.",
            ffmpeg_path
        ));
    }
    
    // Try to run it (no console window on Windows)
    let mut cmd = Command::new(&ffmpeg_path);
    cmd.arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    cmd.status()
        .map_err(|e| format!("FFmpeg failed to execute: {}. Path: {:?}", e, ffmpeg_path))?;
    
    log::info!("FFmpeg check passed: {:?}", ffmpeg_path);
    Ok(())
}

// =============================================================================
// Display Enumeration
// =============================================================================

// Get information about all available displays
pub fn get_all_displays() -> Result<Vec<MonitorInfo>, String> {
    let targets = scap::get_all_targets();
    
    let monitors: Vec<MonitorInfo> = targets
        .iter()
        .enumerate()
        .filter_map(|(idx, target)| {
            if let Target::Display(_) = target {
                Some(MonitorInfo {
                    id: idx as u32,
                    // Dimensions determined at capture time from first frame
                    width: 0,
                    height: 0,
                    x: 0,
                    y: 0,
                    scale_factor: 1.0,
                    is_primary: idx == 0,
                })
            } else {
                None
            }
        })
        .collect();
    
    log::info!("Found {} display(s)", monitors.len());
    for monitor in &monitors {
        log::info!("  Display {} (primary: {})", monitor.id, monitor.is_primary);
    }
    
    Ok(monitors)
}

// Get all display targets for capture
pub fn get_display_targets() -> Vec<Target> {
    scap::get_all_targets()
        .into_iter()
        .filter(|t| matches!(t, Target::Display(_)))
        .collect()
}

// Get display count
pub fn get_display_count() -> usize {
    get_display_targets().len()
}

// =============================================================================
// Capture Result
// =============================================================================

// Result from capture operation for a single display
#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub display_index: u32,
    pub width: u32,
    pub height: u32,
    pub frame_count: u64,
    pub file_size: u64,
}

// =============================================================================
// TODO: Future enhancement - composite multiple displays into single video
// =============================================================================
// This would combine _d0.mp4 and _d1.mp4 side-by-side for unified Gemini analysis.
// 
// Implementation approach:
// 1. After all capture threads complete in manager.rs finalize_current_segment()
// 2. Use FFmpeg to composite videos: ffmpeg -i d0.mp4 -i d1.mp4 -filter_complex hstack output.mp4
// 3. Optionally delete individual display files after composite
// 4. Update metadata to reference composite file
//
// Benefits:
// - Single file upload to Gemini instead of multiple
// - Unified timeline view across all displays
// - Reduced API calls and simpler processing pipeline
// =============================================================================

// =============================================================================
// Audio Format Helpers
// =============================================================================

// Audio capture state for system audio from scap
struct AudioCaptureState {
    format: Option<AudioFormat>,
    channels: Option<u16>,
    sample_rate: Option<u32>,
    is_planar: Option<bool>,
    sample_size: Option<usize>,
    writer: Option<BufWriter<File>>,
    temp_path: PathBuf,
    sample_count: u64,
    data_bytes_written: u64,
}

impl AudioCaptureState {
    fn new(temp_path: PathBuf) -> Self {
        Self {
            format: None,
            channels: None,
            sample_rate: None,
            is_planar: None,
            sample_size: None,
            writer: None,
            temp_path,
            sample_count: 0,
            data_bytes_written: 0,
        }
    }

    fn has_audio(&self) -> bool {
        self.format.is_some() && self.writer.is_some()
    }
}

// Convert planar audio (LLLLRRRR) to interleaved (LRLRLR)
// scap on macOS reports planar: false but actually provides planar audio
#[cfg(target_os = "macos")]
fn convert_planar_to_interleaved_f32(planar_data: &[u8], channels: u16) -> Vec<u8> {
    if channels != 2 {
        return planar_data.to_vec();
    }
    
    let samples_per_channel = planar_data.len() / 4 / 2; // Number of samples per channel
    let mut interleaved = vec![0u8; planar_data.len()];
    
    // Input is PLANAR: [L0 L1 L2 ... Ln] [R0 R1 R2 ... Rn]
    // Output should be INTERLEAVED: [L0 R0] [L1 R1] [L2 R2] ...
    for i in 0..samples_per_channel {
        // Source positions in planar data
        let left_src = i * 4;                              // Left channel in first half
        let right_src = (samples_per_channel + i) * 4;     // Right channel in second half
        
        // Destination positions in interleaved data
        let left_dst = i * 8;         // Left sample
        let right_dst = i * 8 + 4;    // Right sample immediately after
        
        if right_src + 4 <= planar_data.len() && right_dst + 4 <= interleaved.len() {
            interleaved[left_dst..left_dst + 4].copy_from_slice(&planar_data[left_src..left_src + 4]);
            interleaved[right_dst..right_dst + 4].copy_from_slice(&planar_data[right_src..right_src + 4]);
        }
    }
    
    interleaved
}

// Write WAV file header for 16-bit PCM audio (most compatible format)
fn write_wav_header_s16(
    writer: &mut BufWriter<File>,
    sample_rate: u32,
    channels: u16,
) -> Result<(), std::io::Error> {
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * 2; // 2 bytes per sample
    let block_align = channels * 2;
    
    // RIFF header
    writer.write_all(b"RIFF")?;
    writer.write_all(&0u32.to_le_bytes())?; // File size - 8 (placeholder)
    writer.write_all(b"WAVE")?;
    
    // fmt subchunk
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?; // Subchunk1 size (16 for PCM)
    writer.write_all(&1u16.to_le_bytes())?;  // Audio format: 1 = PCM
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;
    
    // data subchunk
    writer.write_all(b"data")?;
    writer.write_all(&0u32.to_le_bytes())?; // Data size (placeholder)
    
    Ok(())
}

// Convert F32 audio samples to S16 (signed 16-bit) for better compatibility
// F32 range: -1.0 to 1.0 -> S16 range: -32768 to 32767
fn convert_f32_to_s16(f32_data: &[u8]) -> Vec<u8> {
    let mut s16_data = Vec::with_capacity(f32_data.len() / 2);
    
    for chunk in f32_data.chunks_exact(4) {
        let f32_sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        // Clamp to valid range and convert
        let clamped = f32_sample.clamp(-1.0, 1.0);
        let s16_sample = (clamped * 32767.0) as i16;
        s16_data.extend_from_slice(&s16_sample.to_le_bytes());
    }
    
    s16_data
}

// Update WAV header with correct sizes after all data is written
fn finalize_wav_header(path: &PathBuf, data_bytes: u64) -> Result<(), std::io::Error> {
    use std::io::{Seek, SeekFrom};
    
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)?;
    
    // Update RIFF chunk size (file size - 8)
    let riff_size = (data_bytes + 36) as u32; // 36 = header size - 8
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&riff_size.to_le_bytes())?;
    
    // Update data chunk size
    file.seek(SeekFrom::Start(40))?;
    file.write_all(&(data_bytes as u32).to_le_bytes())?;
    
    Ok(())
}

fn interleave_planar_audio(
    raw_data: &[u8],
    channels: u16,
    sample_count: usize,
    sample_size: usize,
) -> Vec<u8> {
    let plane_size = sample_count * sample_size;
    let total_size = plane_size * channels as usize;
    
    // Verify data size
    if raw_data.len() < total_size {
        log::warn!(
            "[AUDIO] Planar audio data too small: expected {} bytes, got {} bytes",
            total_size, raw_data.len()
        );
        return raw_data.to_vec();
    }
    
    let mut interleaved = vec![0u8; total_size];
    
    for sample_idx in 0..sample_count {
        for channel in 0..channels as usize {
            let src_offset = channel * plane_size + sample_idx * sample_size;
            let dst_offset = sample_idx * channels as usize * sample_size + channel * sample_size;
            
            // Copy one sample
            interleaved[dst_offset..dst_offset + sample_size]
                .copy_from_slice(&raw_data[src_offset..src_offset + sample_size]);
        }
    }
    
    interleaved
}

// =============================================================================
// FFmpeg Process Management
// =============================================================================

// Spawn FFmpeg process for encoding with configurable quality settings
// 
// # Arguments
// * `width` - Input frame width
// * `height` - Input frame height
// * `output_width` - Target output width (height calculated to maintain aspect, with letterbox/pillarbox)
// * `fps` - Target framerate
// * `crf` - Constant Rate Factor (0-51, lower = better quality, higher = smaller files)
// * `preset` - FFmpeg encoding preset (ultrafast, superfast, veryfast, faster, fast, medium, slow)
// * `output_path` - Path to write the MP4 file
fn spawn_ffmpeg(
    width: u32,
    height: u32,
    output_width: u32,
    fps: u8,
    crf: u8,
    preset: &str,
    output_path: &PathBuf,
) -> Result<Child, String> {
    let ffmpeg_path = get_ffmpeg_path()?;
    
    // Calculate output height maintaining 16:9 aspect ratio for standardized output
    let output_height = (output_width * 9) / 16;
    // Ensure height is even (required for yuv420p)
    let output_height = if output_height % 2 == 1 { output_height + 1 } else { output_height };
    
    // Build scale filter with letterbox/pillarbox to maintain aspect ratio
    // This scales the input to fit within output dimensions, then pads to exact size
    let scale_filter = format!(
        "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:black",
        output_width, output_height, output_width, output_height
    );
    
    log::info!(
        "Spawning FFmpeg: {}x{} -> {}x{} @ {} fps, CRF {}, preset {} -> {:?}",
        width, height, output_width, output_height, fps, crf, preset, output_path
    );
    log::info!("FFmpeg binary: {:?}", ffmpeg_path);
    
    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args([
        "-y",                           // Overwrite output
        "-f", "rawvideo",               // Input format
        "-pix_fmt", "bgra",             // Input pixel format
        "-s", &format!("{}x{}", width, height),  // Input size
        "-r", &fps.to_string(),         // Input framerate
        "-i", "pipe:0",                 // Read from stdin
        "-vf", &scale_filter,           // Scale and letterbox/pillarbox filter
        "-c:v", "libx264",              // H.264 codec
        "-preset", preset,              // Encoding preset (configurable)
        "-crf", &crf.to_string(),       // Quality (configurable)
        "-tune", "stillimage",          // Optimized for screen content
        "-pix_fmt", "yuv420p",          // Output pixel format (MP4 compatibility)
        "-movflags", "+faststart",      // Enable streaming
    ])
    .arg(output_path)
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null());             // Discard stderr to prevent buffer blocking
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);     // CREATE_NO_WINDOW
    cmd.spawn()
        .map_err(|e| format!("Failed to spawn FFmpeg at {:?}: {}", ffmpeg_path, e))
}

// =============================================================================
// Main Capture Function
// =============================================================================

// Capture frames from a specific display to an MP4 file
// 
// This function captures frames from one display and pipes them to FFmpeg.
// It will run until either:
// - The shutdown signal is set (primary termination method)
// - The safety_timeout is reached (fallback if shutdown signal fails)
// 
// # Arguments
// * `display_index` - Index of the display to capture
// * `fps` - Target framerate
// * `output_width` - Target output width (height calculated to maintain 16:9 aspect)
// * `crf` - Constant Rate Factor for quality (0-51, lower = better quality)
// * `preset` - FFmpeg encoding preset
// * `output_path` - Path to write MP4 file
// * `safety_timeout` - Safety timeout (segment_duration + buffer), only triggers if shutdown signal fails
// * `shutdown` - Shutdown signal
// * `audio_role` - Whether this display captures audio (Primary) or uses shared audio (Secondary)
// * `capture_audio` - Whether to capture audio (system + microphone)
pub fn capture_display_to_file(
    display_index: u32,
    fps: u8,
    output_width: u32,
    crf: u8,
    preset: &str,
    output_path: &PathBuf,
    safety_timeout: Duration,
    shutdown: Arc<AtomicBool>,
    audio_role: AudioCaptureRole,
    capture_audio: bool,
) -> Result<CaptureResult, String> {
    log::info!(
        "Starting capture for display {} to {:?} ({} fps, {}px wide, CRF {}, preset {}, safety_timeout {:?}, capture_audio: {})",
        display_index, output_path, fps, output_width, crf, preset, safety_timeout, capture_audio
    );
    
    // Check FFmpeg availability
    check_ffmpeg()?;
    
    // Check platform support
    if !scap::is_supported() {
        return Err("Screen capture not supported on this platform".to_string());
    }
    
    // Check permission
    if !scap::has_permission() {
        return Err(
            "Screen recording permission not granted. On macOS, enable in System Preferences > Privacy & Security > Screen Recording".to_string()
        );
    }
    
    // Get specific display target
    let targets = get_display_targets();
    let target = targets
        .into_iter()
        .nth(display_index as usize)
        .ok_or_else(|| format!("Display {} not found", display_index))?;
    
    // Determine if this display should capture audio based on role and capture_audio flag
    let is_audio_primary = matches!(audio_role, AudioCaptureRole::Primary { .. });
    let should_capture_system_audio = capture_audio && is_audio_primary;
    
    log::info!(
        "[AUDIO] Display {}: Audio capture role: {}, capture_audio: {}, should_capture_system_audio: {}",
        display_index,
        if is_audio_primary { "PRIMARY" } else { "SECONDARY" },
        capture_audio,
        should_capture_system_audio
    );
    
    log::info!("Creating capturer for display {} (capture_audio: {})", display_index, capture_audio);
    
    // Configure capture options for this specific display
    // System audio capture requires captures_audio: true
    let options = Options {
        fps: fps as u32,
        target: Some(target),
        show_cursor: true,
        show_highlight: false,
        excluded_targets: None,
        output_type: FrameType::BGRAFrame,
        output_resolution: scap::capturer::Resolution::Captured,
        captures_audio: should_capture_system_audio,
        ..Default::default()
    };
    
    // Create and start capturer
    let mut capturer = Capturer::build(options)
        .map_err(|e| format!("Failed to create capturer for display {}: {:?}", display_index, e))?;
    
    capturer.start_capture();
    
    // Wait briefly for capturer to initialize
    std::thread::sleep(Duration::from_millis(100));
    
    // Get first frame to determine dimensions
    log::info!("Display {}: Waiting for first frame...", display_index);
    let (width, height, first_frame_data) = wait_for_first_frame(&mut capturer, display_index)?;
    
    log::info!("[CAPTURE] Display {}: Capture initialized: {}x{}", display_index, width, height);
    
    // Create permanent paths for audio files (matching transcription::storage::get_audio_path format)
    // Format: {segment_id}_d{display_index}.{source}.wav
    let permanent_audio_path = output_path.with_file_name(
        output_path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| format!("{}.audio.wav", s))
            .unwrap_or_else(|| format!("{}_d{}.audio.wav", "segment", display_index))
    );
    let permanent_mic_path = output_path.with_file_name(
        output_path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| format!("{}.mic.wav", s))
            .unwrap_or_else(|| format!("{}_d{}.mic.wav", "segment", display_index))
    );
    
    log::info!("[CAPTURE] Display {}: Final output path: {:?}", display_index, output_path);
    log::info!("[CAPTURE] Display {}: System audio path: {:?}", display_index, permanent_audio_path);
    log::info!("[CAPTURE] Display {}: Mic audio path: {:?}", display_index, permanent_mic_path);
    
    // Verify parent directory exists
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            log::error!("[CAPTURE] Display {}: Parent directory does not exist: {:?}", display_index, parent);
            return Err(format!("Parent directory does not exist: {:?}", parent));
        }
        log::info!("[CAPTURE] Display {}: Parent directory verified: {:?}", display_index, parent);
    }
    
    // Spawn FFmpeg process with encoding configuration
    let mut ffmpeg = spawn_ffmpeg(width, height, output_width, fps, crf, preset, output_path)?;
    let mut stdin = ffmpeg.stdin.take()
        .ok_or_else(|| "Failed to get FFmpeg stdin".to_string())?;
    
    // Write first frame
    stdin.write_all(&first_frame_data)
        .map_err(|e| format!("Failed to write frame to FFmpeg: {}", e))?;
    
    // Initialize audio capture state (only if capturing audio)
    // Note: System audio capture not available in scap 0.0.8, kept for future compatibility
    let mut audio_state = AudioCaptureState::new(permanent_audio_path.clone());
    
    // Initialize microphone capture (only if capture_audio is true and this is the primary display)
    let should_capture_mic = capture_audio && is_audio_primary;
    let mut mic_capture: Option<crate::recording::microphone::MicrophoneCapture> = if should_capture_mic {
        match crate::recording::microphone::MicrophoneCapture::new() {
            Ok(mic) => {
                log::info!("[MIC] Display {}: Microphone initialized ({}Hz, {} channels)", 
                    display_index, mic.sample_rate(), mic.channels());
                Some(mic)
            }
            Err(e) => {
                log::warn!("[MIC] Display {}: Microphone unavailable: {}", display_index, e);
                None
            }
        }
    } else {
        log::info!("[MIC] Display {}: Skipping microphone capture (capture_audio: {}, is_primary: {})", 
            display_index, capture_audio, is_audio_primary);
        None
    };
    
    // Start microphone capture if available (Primary only)
    if let Some(ref mut mic) = mic_capture {
        if let Err(e) = mic.start() {
            log::warn!("[MIC] Display {}: Failed to start microphone: {}", display_index, e);
            mic_capture = None;
        } else {
            log::info!("[MIC] Display {}: Microphone recording started", display_index);
        }
    }
    
    // Mic WAV file state
    let mut mic_writer: Option<BufWriter<File>> = None;
    let mut mic_bytes_written: u64 = 0;
    
    // Create mic WAV file if mic is available (Primary only)
    if let Some(ref mic) = mic_capture {
        let sample_rate = mic.sample_rate();
        let channels = mic.channels();
        
        match File::create(&permanent_mic_path) {
            Ok(file) => {
                let mut writer = BufWriter::new(file);
                if let Err(e) = crate::recording::microphone::write_mic_wav_header(&mut writer, sample_rate, channels) {
                    log::error!("[MIC] Display {}: Failed to write mic WAV header: {}", display_index, e);
                } else {
                    log::info!("[MIC] Display {}: Mic WAV file created ({}Hz, {} channels)", 
                        display_index, sample_rate, channels);
                    mic_writer = Some(writer);
                }
            }
            Err(e) => {
                log::error!("[MIC] Display {}: Failed to create mic WAV file: {}", display_index, e);
            }
        }
    }
    
    // Capture loop - primary exit via shutdown signal, safety_timeout is fallback
    let start_time = Instant::now();
    let mut frame_count: u64 = 1; // Already wrote first frame
    let expected_frame_size = (width * height * 4) as usize; // BGRA = 4 bytes per pixel
    
    // Frame buffer for handling empty frames from scap
    let mut last_good_frame: Vec<u8> = first_frame_data;
    let mut empty_frame_count: u64 = 0;
    let mut wrong_size_count: u64 = 0;
    let mut audio_frame_count: u64 = 0;
    
    while !shutdown.load(Ordering::SeqCst) && start_time.elapsed() < safety_timeout {
        match capturer.get_next_frame() {
            Ok(frame) => {
                match frame {
                    Frame::Video(video_frame) => {
                        let data = match video_frame {
                            VideoFrame::BGRA(bgra_frame) => bgra_frame.data,
                            VideoFrame::BGR0(bgr_frame) => bgr_frame.data,
                            VideoFrame::RGB(rgb_frame) => rgb_frame.data,
                            VideoFrame::RGBx(rgbx_frame) => rgbx_frame.data,
                            VideoFrame::XBGR(xbgr_frame) => xbgr_frame.data,
                            VideoFrame::BGRx(bgrx_frame) => bgrx_frame.data,
                            VideoFrame::YUVFrame(_) => {
                                log::warn!("Display {}: YUV frame not supported, skipping", display_index);
                                continue;
                            }
                        };
                        
                        // Handle frame data - reuse buffer for empty frames from scap
                        let frame_data: &[u8] = if data.len() == expected_frame_size {
                            // Valid frame - update buffer
                            last_good_frame = data;
                            &last_good_frame
                        } else if data.is_empty() {
                            // Empty frame from scap - reuse last good frame
                            empty_frame_count += 1;
                            &last_good_frame
                        } else {
                            // Wrong size (not empty, not correct) - skip to prevent FFmpeg desync
                            wrong_size_count += 1;
                            if wrong_size_count <= 3 {
                                log::warn!(
                                    "Display {}: Wrong frame size! Expected {} bytes, got {} bytes",
                                    display_index, expected_frame_size, data.len()
                                );
                            }
                            continue;
                        };
                        
                        // Write frame to FFmpeg
                        if let Err(e) = stdin.write_all(frame_data) {
                            log::error!("Display {}: Failed to write frame to FFmpeg: {}", display_index, e);
                            break;
                        }
                        
                        frame_count += 1;
                        
                        // Drain microphone buffer periodically (every 10 frames to avoid overhead)
                        if frame_count % 10 == 0 {
                            if let Some(ref mic) = mic_capture {
                                if let Some(ref mut writer) = mic_writer {
                                    let samples = mic.take_samples();
                                    if !samples.is_empty() {
                                        let bytes = crate::recording::microphone::convert_mic_f32_to_s16_bytes(&samples);
                                        if let Err(e) = writer.write_all(&bytes) {
                                            log::error!("[MIC] Display {}: Failed to write mic data: {}", display_index, e);
                                        } else {
                                            mic_bytes_written += bytes.len() as u64;
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Log progress periodically (every 10 seconds worth of frames)
                        if frame_count % (fps as u64 * 10) == 0 {
                            log::info!(
                                "[VIDEO] Display {}: Progress - {} video frames, {} audio frames ({:.1}s elapsed)",
                                display_index,
                                frame_count,
                                audio_frame_count,
                                start_time.elapsed().as_secs_f32()
                            );
                        }
                    },
                    Frame::Audio(audio_frame) => {
                        // Capture audio frame
                        audio_frame_count += 1;
                        
                        // Initialize audio state on first audio frame
                        if audio_state.format.is_none() {
                            let format = audio_frame.format();
                            let sample_size = match format {
                                AudioFormat::I8 | AudioFormat::U8 => 1,
                                AudioFormat::I16 | AudioFormat::U16 => 2,
                                AudioFormat::I32 | AudioFormat::U32 | AudioFormat::F32 => 4,
                                AudioFormat::I64 | AudioFormat::U64 | AudioFormat::F64 => 8,
                                _ => 4, // Default to 4 bytes
                            };
                            
                            audio_state.format = Some(format);
                            audio_state.channels = Some(audio_frame.channels());
                            audio_state.sample_rate = Some(audio_frame.rate());
                            audio_state.is_planar = Some(audio_frame.is_planar());
                            audio_state.sample_size = Some(sample_size);
                            
                            log::info!(
                                "[AUDIO] Display {}: First audio frame received - format: {:?}, channels: {}, rate: {}Hz, planar: {}, sample_size: {} bytes",
                                display_index, format, 
                                audio_frame.channels(), audio_frame.rate(), 
                                audio_frame.is_planar(), sample_size
                            );
                            log::info!("[AUDIO] Display {}: Creating audio temp file (WAV S16 format): {:?}", display_index, audio_state.temp_path);
                            
                            // Create audio temp file with WAV header (S16 PCM for maximum compatibility)
                            match File::create(&audio_state.temp_path) {
                                Ok(file) => {
                                    let mut writer = BufWriter::new(file);
                                    
                                    // Write WAV header for S16 PCM audio
                                    if let Err(e) = write_wav_header_s16(
                                        &mut writer,
                                        audio_frame.rate(),
                                        audio_frame.channels(),
                                    ) {
                                        log::error!("[AUDIO] Display {}: Failed to write WAV header: {}", display_index, e);
                                    } else {
                                        log::info!(
                                            "[AUDIO] Display {}: WAV header written successfully (S16 PCM, {}ch, {}Hz)",
                                            display_index, audio_frame.channels(), audio_frame.rate()
                                        );
                                    }
                                    
                                    audio_state.writer = Some(writer);
                                }
                                Err(e) => {
                                    log::error!("[AUDIO] Display {}: Failed to create audio temp file: {} (path: {:?})", 
                                        display_index, e, audio_state.temp_path);
                                }
                            }
                        }
                        
                        // Write audio data to temp file
                        if let Some(ref mut writer) = audio_state.writer {
                            let is_planar = audio_state.is_planar.unwrap_or(false);
                            let channels = audio_state.channels.unwrap_or(2);
                            let sample_size = audio_state.sample_size.unwrap_or(4);
                            
                            // Get audio data - convert planar to interleaved if necessary
                            let audio_data: Vec<u8> = if is_planar && channels > 1 {
                                interleave_planar_audio(
                                    audio_frame.raw_data(),
                                    channels,
                                    audio_frame.sample_count(),
                                    sample_size,
                                )
                            } else {
                                audio_frame.raw_data().to_vec()
                            };
                            
                            // macOS-specific: scap reports planar: false but actually provides planar audio
                            // This is a known scap bug - apply conversion only on macOS
                            #[cfg(target_os = "macos")]
                            let audio_data = {
                                let channels = audio_state.channels.unwrap_or(2);
                                convert_planar_to_interleaved_f32(&audio_data, channels)
                            };
                            
                            // Convert F32 to S16 for the WAV file
                            let s16_data = convert_f32_to_s16(&audio_data);
                            
                            if let Err(e) = writer.write_all(&s16_data) {
                                log::error!("[AUDIO] Display {}: Failed to write audio data: {}", display_index, e);
                            } else {
                                audio_state.sample_count += audio_frame.sample_count() as u64;
                                audio_state.data_bytes_written += s16_data.len() as u64;
                            }
                        }
                        
                        // Log audio progress periodically
                        if audio_frame_count % 100 == 0 {
                            log::debug!(
                                "[AUDIO] Display {}: {} audio frames, {} samples captured",
                                display_index, audio_frame_count, audio_state.sample_count
                            );
                        }
                        
                        // Drain microphone buffer and write to WAV (do this during audio frames)
                        if let Some(ref mic) = mic_capture {
                            if let Some(ref mut writer) = mic_writer {
                                let samples = mic.take_samples();
                                if !samples.is_empty() {
                                    let bytes = crate::recording::microphone::convert_mic_f32_to_s16_bytes(&samples);
                                    if let Err(e) = writer.write_all(&bytes) {
                                        log::error!("[MIC] Display {}: Failed to write mic data: {}", display_index, e);
                                    } else {
                                        mic_bytes_written += bytes.len() as u64;
                                    }
                                }
                            }
                        }
                        
                    }
                }
            }
            Err(e) => {
                log::error!("Display {}: Capture error: {:?}", display_index, e);
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    
    
    // Log frame statistics
    log::info!("[CAPTURE] Display {}: Capture loop ended after {:.1}s", display_index, start_time.elapsed().as_secs_f32());
    log::info!("[CAPTURE] Display {}: Video frames captured: {}", display_index, frame_count);
    if audio_frame_count > 0 {
        log::info!("[CAPTURE] Display {}: Audio frames captured: {} ({} samples)", display_index, audio_frame_count, audio_state.sample_count);
    }
    
    if empty_frame_count > 0 {
        log::info!(
            "[CAPTURE] Display {}: Reused previous frame {} times (empty frames from scap)",
            display_index, empty_frame_count
        );
    }
    if wrong_size_count > 0 {
        log::warn!(
            "[CAPTURE] Display {}: Skipped {} frames with wrong size",
            display_index, wrong_size_count
        );
    }
    
    // Stop capture
    log::info!("[CAPTURE] Display {}: Stopping capturer...", display_index);
    capturer.stop_capture();
    
    // Flush and close audio writer
    if let Some(ref mut writer) = audio_state.writer {
        log::info!("[AUDIO] Display {}: Flushing audio buffer...", display_index);
        if let Err(e) = writer.flush() {
            log::error!("[AUDIO] Display {}: Failed to flush audio buffer: {}", display_index, e);
        }
    }
    // Drop writer to close file before updating header
    audio_state.writer = None;
    
    // Finalize WAV header with correct data size
    if audio_state.data_bytes_written > 0 {
        log::info!(
            "[AUDIO] Display {}: Finalizing WAV header (data bytes: {})",
            display_index, audio_state.data_bytes_written
        );
        if let Err(e) = finalize_wav_header(&audio_state.temp_path, audio_state.data_bytes_written) {
            log::error!("[AUDIO] Display {}: Failed to finalize WAV header: {}", display_index, e);
        }
    }
    
    // Check audio file
    let system_audio_available = if audio_state.temp_path.exists() && audio_state.data_bytes_written > 0 {
        let audio_size = std::fs::metadata(&audio_state.temp_path).map(|m| m.len()).unwrap_or(0);
        log::info!("[AUDIO] Display {}: System audio file (WAV S16): {:?} ({} bytes)", display_index, audio_state.temp_path, audio_size);
        true
    } else {
        log::info!("[AUDIO] Display {}: No system audio captured", display_index);
        false
    };
    
    // Stop microphone capture and finalize mic WAV
    if let Some(ref mut mic) = mic_capture {
        mic.stop();
        
        // Drain any remaining samples
        if let Some(ref mut writer) = mic_writer {
            let remaining_samples = mic.take_samples();
            if !remaining_samples.is_empty() {
                let bytes = crate::recording::microphone::convert_mic_f32_to_s16_bytes(&remaining_samples);
                if let Err(e) = writer.write_all(&bytes) {
                    log::error!("[MIC] Display {}: Failed to write remaining mic data: {}", display_index, e);
                } else {
                    mic_bytes_written += bytes.len() as u64;
                }
            }
            
            log::info!("[MIC] Display {}: Flushing mic buffer...", display_index);
            if let Err(e) = writer.flush() {
                log::error!("[MIC] Display {}: Failed to flush mic buffer: {}", display_index, e);
            }
        }
    }
    // Drop writer to close file before updating header
    drop(mic_writer);
    
    // Finalize mic WAV header
    if mic_bytes_written > 0 {
        log::info!(
            "[MIC] Display {}: Finalizing mic WAV header (data bytes: {})",
            display_index, mic_bytes_written
        );
        if let Err(e) = crate::recording::microphone::finalize_mic_wav_header(&permanent_mic_path, mic_bytes_written) {
            log::error!("[MIC] Display {}: Failed to finalize mic WAV header: {}", display_index, e);
        }
    }
    
    // Check mic file
    let mic_audio_available = if permanent_mic_path.exists() && mic_bytes_written > 0 {
        let mic_size = std::fs::metadata(&permanent_mic_path).map(|m| m.len()).unwrap_or(0);
        log::info!("[MIC] Display {}: Mic audio file (WAV S16): {:?} ({} bytes)", display_index, permanent_mic_path, mic_size);
        true
    } else {
        log::info!("[MIC] Display {}: No mic audio captured", display_index);
        false
    };
    
    // Close stdin to signal EOF to FFmpeg
    drop(stdin);
    
    // Wait for FFmpeg to finish
    log::info!("Display {}: Waiting for FFmpeg to finish encoding...", display_index);
    let ffmpeg_result = ffmpeg.wait()
        .map_err(|e| format!("Failed to wait for FFmpeg: {}", e))?;
    
    if !ffmpeg_result.success() {
        return Err(format!("FFmpeg exited with error: {:?}", ffmpeg_result.code()));
    }
    
    // Get file size
    let file_size = std::fs::metadata(output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    
    let actual_fps = if start_time.elapsed().as_secs_f64() > 0.0 {
        frame_count as f64 / start_time.elapsed().as_secs_f64()
    } else {
        0.0
    };
    
    log::info!(
        "Display {}: Capture finished: {} frames in {:.1}s ({:.1} fps), {} bytes MP4",
        display_index,
        frame_count,
        start_time.elapsed().as_secs_f64(),
        actual_fps,
        file_size
    );
    
    // Signal audio ready for primary display
    // Audio is ready if we have either system audio or microphone audio
    match audio_role {
        AudioCaptureRole::Primary { audio_ready_signal, audio_failed_signal } => {
            if system_audio_available || mic_audio_available {
                audio_ready_signal.store(true, Ordering::SeqCst);
                log::info!("[AUDIO] Display {}: Audio capture complete (system: {}, mic: {}), ready signal set", 
                    display_index, system_audio_available, mic_audio_available);
            } else {
                audio_failed_signal.store(true, Ordering::SeqCst);
                log::warn!("[AUDIO] Display {}: No audio captured (system: {}, mic: {}), failed signal set", 
                    display_index, system_audio_available, mic_audio_available);
            }
        }
        AudioCaptureRole::Secondary { .. } => {
            // Secondary displays don't capture audio, they use shared audio from primary
            log::info!("[AUDIO] Display {}: Secondary display, using shared audio from primary", display_index);
        }
    }
    
    Ok(CaptureResult {
        display_index,
        width,
        height,
        frame_count,
        file_size,
    })
}

// =============================================================================
// Helper Functions
// =============================================================================

// Wait for the first frame and return dimensions + data
fn wait_for_first_frame(capturer: &mut Capturer, display_index: u32) -> Result<(u32, u32, Vec<u8>), String> {
    let start = Instant::now();
    let timeout = Duration::from_secs(15);
    let mut attempt = 0;
    
    while start.elapsed() < timeout {
        attempt += 1;
        
        match capturer.get_next_frame() {
            Ok(frame) => {
                let (width, height, data) = match frame {
                    Frame::Video(video_frame) => match video_frame {
                        VideoFrame::BGRA(f) => (f.width as u32, f.height as u32, f.data),
                        VideoFrame::BGR0(f) => (f.width as u32, f.height as u32, f.data),
                        VideoFrame::RGB(f) => (f.width as u32, f.height as u32, f.data),
                        VideoFrame::RGBx(f) => (f.width as u32, f.height as u32, f.data),
                        VideoFrame::XBGR(f) => (f.width as u32, f.height as u32, f.data),
                        VideoFrame::BGRx(f) => (f.width as u32, f.height as u32, f.data),
                        VideoFrame::YUVFrame(_) => {
                            continue;
                        }
                    },
                    Frame::Audio(_) => {
                        continue;
                    }
                };
                
                log::info!(
                    "Display {}: Got first frame after {} attempts: {}x{}, {} bytes",
                    display_index, attempt, width, height, data.len()
                );
                
                return Ok((width, height, data));
            }
            Err(_) => {
                if attempt % 50 == 0 {
                    log::warn!(
                        "Display {}: Still waiting for first frame (attempt {}, {:.1}s elapsed)",
                        display_index,
                        attempt,
                        start.elapsed().as_secs_f32()
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    
    Err(format!(
        "Display {}: Timeout waiting for first frame after {:.1}s. Check screen recording permissions.",
        display_index,
        timeout.as_secs_f32()
    ))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported() {
        let _ = is_supported();
    }

    #[test]
    fn test_has_permission() {
        let _ = has_permission();
    }

    #[test]
    fn test_ffmpeg_platform_info() {
        let (subdir, name) = ffmpeg_platform_info();
        assert!(!subdir.is_empty());
        assert!(!name.is_empty());
        #[cfg(target_os = "windows")]
        assert!(name.ends_with(".exe"));
    }

    #[test]
    fn test_get_display_count() {
        let count = get_display_count();
        assert!(count >= 0);
    }
}
