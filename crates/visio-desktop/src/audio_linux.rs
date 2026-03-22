//! Linux audio engine using PulseAudio/PipeWire.
//!
//! Note: `libpulse-simple-binding`'s `Simple::new()` API does not support
//! setting stream properties like `media.role=Communication`. AEC depends on:
//! - PipeWire: automatic echo cancellation for voice apps (enabled by default)
//! - PulseAudio: `module-echo-cancel` must be loaded system-wide
//! For explicit `media.role` support, would need the full async
//! `libpulse-binding` Context + Stream API (future improvement).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use livekit::webrtc::audio_source::native::NativeAudioSource;
use visio_core::{AudioCaptureBuffer, AudioPlayoutBuffer, CapturedFrame};

use super::audio_engine::{self, LK_CHANNELS, LK_SAMPLE_RATE, VoiceAudioEngine};

use libpulse_binding as pulse;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;

const FRAME_SIZE: usize = 480; // 10ms at 48kHz

pub struct LinuxAudioEngine {
    playback_thread: Option<std::thread::JoinHandle<()>>,
    record_thread: Option<std::thread::JoinHandle<()>>,
    playback_stop: Arc<AtomicBool>,
    record_stop: Arc<AtomicBool>,
    drain_running: Option<Arc<AtomicBool>>,
    _input_device: Option<String>,
    _output_device: Option<String>,
}

impl LinuxAudioEngine {
    pub fn new(input_device: Option<&str>, output_device: Option<&str>) -> Self {
        // Check if module-echo-cancel is available (informational only)
        Self::log_echo_cancel_status();

        Self {
            playback_thread: None,
            record_thread: None,
            playback_stop: Arc::new(AtomicBool::new(false)),
            record_stop: Arc::new(AtomicBool::new(false)),
            drain_running: None,
            _input_device: input_device.map(String::from),
            _output_device: output_device.map(String::from),
        }
    }

    fn log_echo_cancel_status() {
        // Use pactl to check if echo-cancel is loaded (simpler than async introspection API)
        match std::process::Command::new("pactl")
            .args(["list", "modules", "short"])
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("module-echo-cancel") {
                    tracing::info!("PulseAudio module-echo-cancel is loaded — AEC active");
                } else {
                    tracing::warn!(
                        "PulseAudio module-echo-cancel not loaded — AEC inactive. \
                         Load it with: pactl load-module module-echo-cancel"
                    );
                }
            }
            Err(_) => {
                tracing::warn!("Could not check PulseAudio modules (pactl not found)");
            }
        }
    }

    fn pulse_spec() -> Spec {
        Spec {
            format: Format::S16le,
            channels: LK_CHANNELS as u8,
            rate: LK_SAMPLE_RATE,
        }
    }
}

impl VoiceAudioEngine for LinuxAudioEngine {
    fn start_playout(&mut self, buffer: Arc<AudioPlayoutBuffer>) -> Result<(), String> {
        let stop = self.playback_stop.clone();
        stop.store(false, Ordering::Relaxed);

        let handle = std::thread::spawn(move || {
            let spec = LinuxAudioEngine::pulse_spec();

            let stream = match Simple::new(
                None,    // default server
                "Visio", // app name
                Direction::Playback,
                None,           // default device
                "Voice Output", // stream description
                &spec,
                None, // default channel map
                None, // default buffering
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("PulseAudio playback open failed: {e}");
                    return;
                }
            };

            let mut samples = vec![0i16; FRAME_SIZE];
            while !stop.load(Ordering::Relaxed) {
                buffer.pull_samples(&mut samples);

                // Convert i16 to bytes (little-endian)
                let bytes: Vec<u8> = samples.iter().flat_map(|&s| s.to_le_bytes()).collect();

                if let Err(e) = stream.write(&bytes) {
                    tracing::error!("PulseAudio write failed: {e}");
                    break;
                }
            }
        });

        self.playback_thread = Some(handle);
        tracing::info!("Linux PulseAudio Communication playback started");
        Ok(())
    }

    fn start_capture(
        &mut self,
        source: NativeAudioSource,
        noise_reduction: bool,
    ) -> Result<(), String> {
        let stop = self.record_stop.clone();
        stop.store(false, Ordering::Relaxed);

        let capture_buffer = Arc::new(AudioCaptureBuffer::new(50));
        let drain_running =
            audio_engine::start_drain_thread(capture_buffer.clone(), source, noise_reduction);

        let handle = std::thread::spawn(move || {
            let spec = LinuxAudioEngine::pulse_spec();

            let stream = match Simple::new(
                None,
                "Visio",
                Direction::Record,
                None,
                "Voice Input",
                &spec,
                None,
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("PulseAudio record open failed: {e}");
                    return;
                }
            };

            let mut bytes = vec![0u8; FRAME_SIZE * 2]; // i16 = 2 bytes
            while !stop.load(Ordering::Relaxed) {
                if let Err(e) = stream.read(&mut bytes) {
                    tracing::error!("PulseAudio read failed: {e}");
                    break;
                }

                // Convert bytes to i16 (little-endian)
                let pcm: Vec<i16> = bytes
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();

                let frame = CapturedFrame {
                    pcm,
                    sample_rate: LK_SAMPLE_RATE,
                    num_channels: LK_CHANNELS,
                    samples_per_channel: FRAME_SIZE as u32,
                };
                capture_buffer.push(frame);
            }
        });

        self.record_thread = Some(handle);
        self.drain_running = Some(drain_running);
        tracing::info!("Linux PulseAudio Communication capture started");
        Ok(())
    }

    fn stop_capture(&mut self) {
        self.record_stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.record_thread.take() {
            let _ = h.join();
        }
        if let Some(r) = self.drain_running.take() {
            r.store(false, Ordering::Relaxed);
        }
        tracing::info!("Linux audio capture stopped");
    }

    fn stop_playout(&mut self) {
        self.stop_capture();
        self.playback_stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.playback_thread.take() {
            let _ = h.join();
        }
        tracing::info!("Linux PulseAudio stopped");
    }

    fn set_device_change_callback(&mut self, _callback: super::audio_engine::DeviceChangeCallback) {
        tracing::warn!("device change detection not yet implemented on Linux");
    }
}

impl Drop for LinuxAudioEngine {
    fn drop(&mut self) {
        self.stop_playout();
    }
}
