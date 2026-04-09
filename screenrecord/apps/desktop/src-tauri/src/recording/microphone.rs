/**
 * ============================================================================
 * MICROPHONE CAPTURE MODULE
 * ============================================================================
 * 
 * PURPOSE: Capture audio from the system microphone using cpal
 * 
 * FUNCTIONALITY:
 * - Opens the default input device
 * - Collects F32 audio samples in a thread-safe buffer
 * - Provides start/stop/drain interface for integration with screen capture
 * - Handles device errors gracefully (recording continues without mic)
 * 
 * INTEGRATION:
 * - Used by capture.rs to record microphone audio alongside screen capture
 * - Mic audio is written to a separate WAV file, then mixed with system audio
 * 
 * ============================================================================
 */

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Maximum buffer size in samples before oldest samples are dropped.
/// 5 minutes of stereo audio at 48kHz = 48000 * 300 * 2 = 28,800,000 samples (~110 MB).
/// This prevents unbounded memory growth if take_samples() is not called regularly.
const MAX_BUFFER_SAMPLES: usize = 48_000 * 300 * 2;

/// Microphone capture state and stream management
pub struct MicrophoneCapture {
    /// The cpal input stream (None if not started or failed)
    stream: Option<cpal::Stream>,
    /// Thread-safe buffer for collected samples
    buffer: Arc<Mutex<Vec<f32>>>,
    /// Sample rate of the input device
    sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    channels: u16,
    /// Whether the stream is currently running
    is_running: Arc<AtomicBool>,
}

// cpal::Stream is not Send, so we need to handle this carefully
// The stream is only accessed from the thread that created it
unsafe impl Send for MicrophoneCapture {}

impl MicrophoneCapture {
    /// Create a new MicrophoneCapture instance
    /// 
    /// This initializes the capture but does not start recording.
    /// Call `start()` to begin capturing audio.
    /// 
    /// Returns an error if no input device is available.
    pub fn new() -> Result<Self, String> {
        // Get the default host
        let host = cpal::default_host();
        
        // Get the default input device
        let device = host
            .default_input_device()
            .ok_or_else(|| "No input device available".to_string())?;
        
        // Get device name for logging (cpal 0.15 API)
        let device_name = device.name().unwrap_or_else(|_| "Unknown device".to_string());
        log::info!("[MIC] Using input device: {}", device_name);
        
        // Get the default input config
        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;
        
        let sample_rate_val = config.sample_rate().0; // Extract u32 from SampleRate
        let channels = config.channels();
        
        log::info!(
            "[MIC] Input config: {} Hz, {} channels, format: {:?}",
            sample_rate_val, channels, config.sample_format()
        );
        
        Ok(Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: sample_rate_val,
            channels,
            is_running: Arc::new(AtomicBool::new(false)),
        })
    }
    
    /// Start capturing audio from the microphone
    /// 
    /// Audio samples are collected in an internal buffer.
    /// Call `take_samples()` to drain the buffer.
    pub fn start(&mut self) -> Result<(), String> {
        if self.is_running.load(Ordering::SeqCst) {
            return Ok(()); // Already running
        }
        
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No input device available".to_string())?;
        
        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;
        
        let buffer = Arc::clone(&self.buffer);
        let is_running = Arc::clone(&self.is_running);
        
        // Build the input stream based on sample format
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                self.build_input_stream::<f32>(&device, &config.into(), buffer, is_running)?
            }
            cpal::SampleFormat::I16 => {
                self.build_input_stream::<i16>(&device, &config.into(), Arc::clone(&self.buffer), Arc::clone(&self.is_running))?
            }
            cpal::SampleFormat::U16 => {
                self.build_input_stream::<u16>(&device, &config.into(), Arc::clone(&self.buffer), Arc::clone(&self.is_running))?
            }
            format => {
                return Err(format!("Unsupported sample format: {:?}", format));
            }
        };
        
        // Start the stream
        stream
            .play()
            .map_err(|e| format!("Failed to start input stream: {}", e))?;
        
        self.stream = Some(stream);
        self.is_running.store(true, Ordering::SeqCst);
        
        log::info!("[MIC] Microphone capture started");
        Ok(())
    }
    
    /// Build an input stream for a specific sample format
    fn build_input_stream<T>(
        &self,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        buffer: Arc<Mutex<Vec<f32>>>,
        is_running: Arc<AtomicBool>,
    ) -> Result<cpal::Stream, String>
    where
        T: cpal::Sample + cpal::SizedSample + Send + 'static,
        f32: Sample + cpal::FromSample<T>,
    {
        let err_fn = |err| log::error!("[MIC] Input stream error: {}", err);
        
        device
            .build_input_stream(
                config,
                move |data: &[T], _: &cpal::InputCallbackInfo| {
                    if !is_running.load(Ordering::SeqCst) {
                        return;
                    }
                    
                    // Convert samples to f32 and add to buffer
                    let mut buf = buffer.lock().unwrap_or_else(|e| {
                        log::error!("[MIC] Buffer mutex poisoned, recovering: {}", e);
                        e.into_inner()
                    });
                    
                    // Enforce maximum buffer size to prevent unbounded memory growth.
                    // If the consumer (take_samples) falls behind, drop oldest samples.
                    let incoming = data.len();
                    if buf.len() + incoming > MAX_BUFFER_SAMPLES {
                        let overflow = (buf.len() + incoming).saturating_sub(MAX_BUFFER_SAMPLES);
                        buf.drain(..overflow);
                        log::warn!(
                            "[MIC] Buffer overflow: dropped {} oldest samples (buffer at {} limit)",
                            overflow, MAX_BUFFER_SAMPLES
                        );
                    }
                    
                    for sample in data {
                        buf.push(<f32 as cpal::FromSample<T>>::from_sample_(*sample));
                    }
                },
                err_fn,
                None, // No timeout
            )
            .map_err(|e| format!("Failed to build input stream: {}", e))
    }
    
    /// Stop capturing audio
    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        
        // Drop the stream to stop it
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        
        log::info!("[MIC] Microphone capture stopped");
    }
    
    /// Take all samples from the buffer, clearing it
    /// 
    /// Returns the samples as F32 values in the range [-1.0, 1.0]
    pub fn take_samples(&self) -> Vec<f32> {
        let mut buf = self.buffer.lock().unwrap_or_else(|e| {
            log::error!("[MIC] Buffer mutex poisoned in take_samples, recovering: {}", e);
            e.into_inner()
        });
        std::mem::take(&mut *buf)
    }
    
    /// Get the sample rate of the input device
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    
    /// Get the number of channels
    pub fn channels(&self) -> u16 {
        self.channels
    }
    
    /// Check if the microphone is currently running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
    
    /// Get the current buffer size (number of samples)
    pub fn buffer_size(&self) -> usize {
        self.buffer.lock().unwrap_or_else(|e| {
            log::error!("[MIC] Buffer mutex poisoned in buffer_size, recovering: {}", e);
            e.into_inner()
        }).len()
    }
}

impl Drop for MicrophoneCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Convert F32 samples to S16 bytes (little-endian)
/// 
/// This matches the format used by the system audio capture.
pub fn convert_mic_f32_to_s16_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    
    for &sample in samples {
        // Clamp to valid range and convert
        let clamped = sample.clamp(-1.0, 1.0);
        let s16_sample = (clamped * 32767.0) as i16;
        bytes.extend_from_slice(&s16_sample.to_le_bytes());
    }
    
    bytes
}

/// Write WAV header for microphone audio (S16 PCM)
/// 
/// This creates a standard WAV header compatible with FFmpeg.
pub fn write_mic_wav_header(
    writer: &mut std::io::BufWriter<std::fs::File>,
    sample_rate: u32,
    channels: u16,
) -> Result<(), std::io::Error> {
    use std::io::Write;
    
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

/// Update WAV header with correct file sizes after all data is written
pub fn finalize_mic_wav_header(path: &std::path::PathBuf, data_bytes: u64) -> Result<(), std::io::Error> {
    use std::io::{Seek, SeekFrom, Write};
    
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_convert_f32_to_s16() {
        let samples = vec![0.0, 1.0, -1.0, 0.5, -0.5];
        let bytes = convert_mic_f32_to_s16_bytes(&samples);
        
        // 5 samples * 2 bytes = 10 bytes
        assert_eq!(bytes.len(), 10);
        
        // Check first sample (0.0 -> 0)
        let s0 = i16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(s0, 0);
        
        // Check second sample (1.0 -> 32767)
        let s1 = i16::from_le_bytes([bytes[2], bytes[3]]);
        assert_eq!(s1, 32767);
        
        // Check third sample (-1.0 -> -32767)
        let s2 = i16::from_le_bytes([bytes[4], bytes[5]]);
        assert_eq!(s2, -32767);
    }
}
