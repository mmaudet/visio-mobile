//! Audio engine abstraction with platform-native voice processing (AEC).
//!
//! Provides the `VoiceAudioEngine` trait and a factory that selects the
//! platform-specific implementation. Shared infrastructure (capture buffer,
//! drain thread) lives here; platform files only handle native API setup.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use serde::Serialize;
use tauri::AppHandle;
// Re-export for platform modules
pub use visio_core::{AudioCaptureBuffer, AudioPlayoutBuffer};

// ---------------------------------------------------------------------------
// Mic level monitoring (VU meter)
// ---------------------------------------------------------------------------

/// Current mic RMS level stored as f32 bits for lock-free atomic access.
static MIC_LEVEL: AtomicU32 = AtomicU32::new(0);

/// Returns the current mic RMS level in the range 0.0–1.0.
pub fn get_mic_level() -> f32 {
    f32::from_bits(MIC_LEVEL.load(Ordering::Relaxed))
}

/// Resets the mic level to 0.0 (call when stopping capture/preview).
pub fn clear_mic_level() {
    MIC_LEVEL.store(0f32.to_bits(), Ordering::Relaxed);
}

/// Internal sample rate used by LiveKit (48kHz mono i16).
pub const LK_SAMPLE_RATE: u32 = 48_000;
pub const LK_CHANNELS: u32 = 1;

/// Callback invoked when audio devices change (plug/unplug).
pub type DeviceChangeCallback = Arc<dyn Fn() + Send + Sync>;

/// Unified audio engine handling both playout and capture with AEC.
pub trait VoiceAudioEngine: Send + Sync {
    fn start_playout(&mut self, buffer: Arc<AudioPlayoutBuffer>) -> Result<(), String>;
    fn start_capture(&mut self, source: NativeAudioSource, noise_reduction: bool) -> Result<(), String>;
    fn stop_capture(&mut self);
    fn stop_playout(&mut self);
    /// Register a callback for device add/remove events.
    fn set_device_change_callback(&mut self, callback: DeviceChangeCallback);
    /// Start a lightweight preview capture for VU meter only (no LiveKit source).
    /// Default implementation uses cpal and is platform-agnostic.
    fn start_preview_capture(&mut self, device_name: Option<&str>) -> Result<(), String> {
        start_cpal_preview(device_name)
    }
    /// Stop the preview capture stream started by `start_preview_capture`.
    fn stop_preview_capture(&mut self) {
        stop_cpal_preview();
    }
}

/// Create the platform-appropriate audio engine.
pub fn create_audio_engine(
    input_device: Option<&str>,
    output_device: Option<&str>,
) -> Box<dyn VoiceAudioEngine> {
    #[cfg(target_os = "macos")]
    {
        Box::new(super::audio_macos::MacAudioEngine::new(input_device, output_device))
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(super::audio_windows::WindowsAudioEngine::new(input_device, output_device))
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(super::audio_linux::LinuxAudioEngine::new(input_device, output_device))
    }
}

// ---------------------------------------------------------------------------
// Shared: drain thread (capture buffer → NativeAudioSource)
// ---------------------------------------------------------------------------

/// Start a drain thread that pops frames from `capture_buffer` and sends
/// them to `audio_source`. When `noise_reduction` is true, frames are
/// passed through RNNoise before being sent. Returns a stop flag.
pub fn start_drain_thread(
    capture_buffer: Arc<AudioCaptureBuffer>,
    audio_source: NativeAudioSource,
    noise_reduction: bool,
) -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let running_flag = running.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("audio drain runtime");

        let mut denoiser = if noise_reduction {
            Some(super::noise_reduction::NoiseReducer::new())
        } else {
            None
        };

        rt.block_on(async move {
            while running_flag.load(Ordering::Relaxed) {
                if let Some(frame) = capture_buffer.pop() {
                    // Update VU meter with RMS of the raw captured frame.
                    if !frame.pcm.is_empty() {
                        let sum_sq: f64 = frame.pcm.iter().map(|&s| {
                            let f = s as f64 / i16::MAX as f64;
                            f * f
                        }).sum();
                        let rms = ((sum_sq / frame.pcm.len() as f64).sqrt() as f32).min(1.0);
                        MIC_LEVEL.store(rms.to_bits(), Ordering::Relaxed);
                    }

                    let pcm = if let Some(ref mut nr) = denoiser {
                        let processed = nr.process(&frame.pcm);
                        if processed.is_empty() {
                            continue;
                        }
                        processed
                    } else {
                        frame.pcm
                    };

                    let samples_per_channel = pcm.len() as u32;
                    let lk_frame = AudioFrame {
                        data: pcm.into(),
                        sample_rate: frame.sample_rate,
                        num_channels: frame.num_channels,
                        samples_per_channel,
                    };
                    let _ = audio_source.capture_frame(&lk_frame).await;
                } else {
                    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                }
            }
        });
    });
    running
}

// ---------------------------------------------------------------------------
// cpal-based preview capture for VU meter (no LiveKit source required)
// ---------------------------------------------------------------------------

/// Holds a live cpal input stream.  Keeping the stream alive keeps it running;
/// dropping it stops capture.
struct PreviewStream {
    _stream: cpal::Stream,
}

// SAFETY: cpal::Stream is not Send on all platforms (e.g. macOS CoreAudio).
// We only ever access this through the global Mutex, never across threads.
unsafe impl Send for PreviewStream {}

static PREVIEW_STREAM: std::sync::Mutex<Option<PreviewStream>> =
    std::sync::Mutex::new(None);

/// Open a cpal input stream on `device_name` (or the default device) and
/// compute RMS into `MIC_LEVEL` on every callback.  Idempotent — calling
/// this while a preview is already running replaces it.
pub fn start_cpal_preview(device_name: Option<&str>) -> Result<(), String> {
    let host = cpal::default_host();

    let device = if let Some(name) = device_name {
        host.input_devices()
            .map_err(|e| format!("cpal input_devices: {e}"))?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| format!("audio input device not found: {name}"))?
    } else {
        host.default_input_device()
            .ok_or_else(|| "no default audio input device".to_string())?
    };

    let config = device
        .default_input_config()
        .map_err(|e| format!("cpal default_input_config: {e}"))?;

    let channels = config.channels() as usize;

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    if data.is_empty() { return; }
                    // Mix to mono then compute RMS.
                    let frames = data.len() / channels.max(1);
                    let sum_sq: f64 = (0..frames).map(|f| {
                        let mut sum = 0.0f64;
                        for ch in 0..channels {
                            sum += data[f * channels + ch] as f64;
                        }
                        let mono = sum / channels as f64;
                        mono * mono
                    }).sum();
                    let rms = ((sum_sq / frames as f64).sqrt() as f32).min(1.0);
                    MIC_LEVEL.store(rms.to_bits(), Ordering::Relaxed);
                },
                |err| tracing::warn!("preview capture error: {err}"),
                None,
            ).map_err(|e| format!("build_input_stream(f32): {e}"))?
        }
        cpal::SampleFormat::I16 => {
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    if data.is_empty() { return; }
                    let frames = data.len() / channels.max(1);
                    let sum_sq: f64 = (0..frames).map(|f| {
                        let mut sum = 0.0f64;
                        for ch in 0..channels {
                            sum += data[f * channels + ch] as f64 / i16::MAX as f64;
                        }
                        let mono = sum / channels as f64;
                        mono * mono
                    }).sum();
                    let rms = ((sum_sq / frames as f64).sqrt() as f32).min(1.0);
                    MIC_LEVEL.store(rms.to_bits(), Ordering::Relaxed);
                },
                |err| tracing::warn!("preview capture error: {err}"),
                None,
            ).map_err(|e| format!("build_input_stream(i16): {e}"))?
        }
        cpal::SampleFormat::U16 => {
            device.build_input_stream(
                &config.into(),
                move |data: &[u16], _| {
                    if data.is_empty() { return; }
                    let frames = data.len() / channels.max(1);
                    let sum_sq: f64 = (0..frames).map(|f| {
                        let mut sum = 0.0f64;
                        for ch in 0..channels {
                            // u16 centre is 32768; normalise to [-1, 1]
                            sum += (data[f * channels + ch] as f64 - 32768.0) / 32768.0;
                        }
                        let mono = sum / channels as f64;
                        mono * mono
                    }).sum();
                    let rms = ((sum_sq / frames as f64).sqrt() as f32).min(1.0);
                    MIC_LEVEL.store(rms.to_bits(), Ordering::Relaxed);
                },
                |err| tracing::warn!("preview capture error: {err}"),
                None,
            ).map_err(|e| format!("build_input_stream(u16): {e}"))?
        }
        fmt => return Err(format!("unsupported sample format for preview: {fmt:?}")),
    };

    stream.play().map_err(|e| format!("preview stream play: {e}"))?;

    let mut guard = PREVIEW_STREAM.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(PreviewStream { _stream: stream });
    tracing::info!("mic preview capture started");
    Ok(())
}

/// Stop the cpal preview stream and clear the VU meter level.
pub fn stop_cpal_preview() {
    let mut guard = PREVIEW_STREAM.lock().unwrap_or_else(|e| e.into_inner());
    if guard.take().is_some() {
        tracing::info!("mic preview capture stopped");
    }
    clear_mic_level();
}

// ---------------------------------------------------------------------------
// Speaker test tone
// ---------------------------------------------------------------------------

/// Play a short test tone (1 second, 440 Hz sine wave) on the default/selected
/// output device.  Blocks the calling thread for approximately 1.1 seconds.
pub fn play_speaker_test(device_name: Option<&str>) -> Result<(), String> {
    let host = cpal::default_host();

    let device = if let Some(name) = device_name {
        host.output_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| format!("Output device '{}' not found", name))?
    } else {
        host.default_output_device()
            .ok_or_else(|| "No default output device".to_string())?
    };

    let config = device.default_output_config().map_err(|e| e.to_string())?;
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;

    const FREQUENCY: f32 = 440.0;
    const DURATION_SECS: f32 = 1.0;
    let total_frames = (sample_rate * DURATION_SECS) as usize;
    let mut frame_idx = 0usize;
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done_clone = done.clone();

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    if frame_idx >= total_frames {
                        for s in frame.iter_mut() {
                            *s = 0.0;
                        }
                        done_clone.store(true, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        let t = frame_idx as f32 / sample_rate;
                        let value =
                            (t * FREQUENCY * 2.0 * std::f32::consts::PI).sin() * 0.3;
                        for s in frame.iter_mut() {
                            *s = value;
                        }
                        frame_idx += 1;
                    }
                }
            },
            |err| tracing::warn!("speaker test error: {err}"),
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    if frame_idx >= total_frames {
                        for s in frame.iter_mut() {
                            *s = 0;
                        }
                        done_clone.store(true, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        let t = frame_idx as f32 / sample_rate;
                        let value =
                            (t * FREQUENCY * 2.0 * std::f32::consts::PI).sin() * 0.3;
                        let sample = (value * i16::MAX as f32) as i16;
                        for s in frame.iter_mut() {
                            *s = sample;
                        }
                        frame_idx += 1;
                    }
                }
            },
            |err| tracing::warn!("speaker test error: {err}"),
            None,
        ),
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config.into(),
            move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    if frame_idx >= total_frames {
                        for s in frame.iter_mut() {
                            *s = 32768;
                        }
                        done_clone.store(true, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        let t = frame_idx as f32 / sample_rate;
                        let value =
                            (t * FREQUENCY * 2.0 * std::f32::consts::PI).sin() * 0.3;
                        let sample = ((value * i16::MAX as f32) as i32 + 32768) as u16;
                        for s in frame.iter_mut() {
                            *s = sample;
                        }
                        frame_idx += 1;
                    }
                }
            },
            |err| tracing::warn!("speaker test error: {err}"),
            None,
        ),
        fmt => return Err(format!("unsupported output sample format: {fmt:?}")),
    }
    .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    // Wait for playback to complete (max 2 seconds)
    let start = std::time::Instant::now();
    while !done.load(std::sync::atomic::Ordering::Relaxed) {
        if start.elapsed() > std::time::Duration::from_secs(2) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    // Small extra delay to let the last samples drain
    std::thread::sleep(std::time::Duration::from_millis(100));

    Ok(())
}

// ---------------------------------------------------------------------------
// Device enumeration (still uses cpal)
// ---------------------------------------------------------------------------

static AUDIO_APP_HANDLE: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

pub fn set_app_handle(handle: AppHandle) {
    let _ = AUDIO_APP_HANDLE.set(handle);
}

#[derive(Serialize, Clone)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    let mut seen = std::collections::HashSet::new();
    host.input_devices()
        .map(|devices| {
            devices
                .filter_map(|d| {
                    let name = d.name().ok()?;
                    if !seen.insert(name.clone()) {
                        return None;
                    }
                    Some(AudioDeviceInfo {
                        is_default: default_name.as_deref() == Some(&name),
                        name,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn list_output_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let mut seen = std::collections::HashSet::new();
    host.output_devices()
        .map(|devices| {
            devices
                .filter_map(|d| {
                    let name = d.name().ok()?;
                    if !seen.insert(name.clone()) {
                        return None;
                    }
                    Some(AudioDeviceInfo {
                        is_default: default_name.as_deref() == Some(&name),
                        name,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Resampling helpers (moved from audio_cpal.rs)
// ---------------------------------------------------------------------------

/// Linear interpolation resampling.
#[allow(dead_code)]
pub fn linear_resample(input: &[i16], output_len: usize) -> Vec<i16> {
    if input.is_empty() || output_len == 0 {
        return vec![0i16; output_len];
    }
    if input.len() == output_len {
        return input.to_vec();
    }
    let mut output = Vec::with_capacity(output_len);
    let ratio = (input.len() - 1) as f64 / (output_len - 1).max(1) as f64;
    for i in 0..output_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = pos - idx as f64;
        let sample = if idx + 1 < input.len() {
            input[idx] as f64 * (1.0 - frac) + input[idx + 1] as f64 * frac
        } else {
            input[idx] as f64
        };
        output.push(sample.round() as i16);
    }
    output
}

/// Mix multi-channel f32 interleaved audio to mono.
#[allow(dead_code)]
pub fn mix_to_mono(data: &[f32], channels: usize) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    let frames = data.len() / channels;
    let mut mono = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut sum = 0.0f32;
        for ch in 0..channels {
            sum += data[f * channels + ch];
        }
        mono.push(sum / channels as f32);
    }
    mono
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_same_length() {
        let input: Vec<i16> = vec![0, 100, 200, 300, 400];
        assert_eq!(linear_resample(&input, 5), input);
    }

    #[test]
    fn resample_upsample_2x() {
        let input: Vec<i16> = vec![0, 100];
        assert_eq!(linear_resample(&input, 3), vec![0, 50, 100]);
    }

    #[test]
    fn resample_downsample() {
        let input: Vec<i16> = vec![0, 50, 100];
        let output = linear_resample(&input, 2);
        assert_eq!(output[0], 0);
        assert_eq!(output[1], 100);
    }

    #[test]
    fn resample_empty() {
        assert!(linear_resample(&[], 0).is_empty());
    }

    #[test]
    fn resample_single() {
        assert_eq!(linear_resample(&[42], 5), vec![42, 42, 42, 42, 42]);
    }

    #[test]
    fn mix_mono_stereo() {
        let stereo = vec![100.0f32, 200.0, 300.0, 400.0];
        let mono = mix_to_mono(&stereo, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 150.0).abs() < f32::EPSILON);
        assert!((mono[1] - 350.0).abs() < f32::EPSILON);
    }
}
