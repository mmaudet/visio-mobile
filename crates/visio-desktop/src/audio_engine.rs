//! Audio engine abstraction with platform-native voice processing (AEC).
//!
//! Provides the `VoiceAudioEngine` trait and a factory that selects the
//! platform-specific implementation. Shared infrastructure (capture buffer,
//! drain thread) lives here; platform files only handle native API setup.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::{DeviceTrait, HostTrait};
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use serde::Serialize;
use tauri::AppHandle;
// Re-export for platform modules
pub use visio_core::{AudioCaptureBuffer, AudioPlayoutBuffer};

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
