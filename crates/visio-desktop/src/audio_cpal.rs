use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use visio_core::AudioPlayoutBuffer;

/// Internal sample rate used by LiveKit (48kHz mono i16).
const LK_SAMPLE_RATE: u32 = 48_000;
const LK_CHANNELS: u32 = 1;

// cpal::Stream is !Send + !Sync due to platform internals, but it is safe
// to hold in Tauri state — we never move the stream across threads, we just
// keep it alive so the OS audio callback keeps firing.
struct SendSyncStream(cpal::Stream);
unsafe impl Send for SendSyncStream {}
unsafe impl Sync for SendSyncStream {}

// ---------------------------------------------------------------------------
// Playout — remote audio → speakers
// ---------------------------------------------------------------------------

pub struct CpalAudioPlayout {
    _stream: SendSyncStream,
}

impl CpalAudioPlayout {
    pub fn start(playout_buffer: Arc<AudioPlayoutBuffer>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no output audio device available")?;

        let default_cfg = device
            .default_output_config()
            .map_err(|e| format!("default output config: {e}"))?;

        let device_sr = default_cfg.sample_rate().0;
        let device_ch = default_cfg.channels();

        tracing::info!(
            "audio playout: device={:?}, rate={device_sr}, channels={device_ch}, format={:?}",
            device.name(),
            default_cfg.sample_format(),
        );

        // Use the device's default config — CoreAudio works best with f32
        let config = cpal::StreamConfig {
            channels: device_ch,
            sample_rate: cpal::SampleRate(device_sr),
            buffer_size: cpal::BufferSize::Default,
        };

        // Pre-compute how many mono 48kHz samples to pull per device callback.
        // If device runs at a different rate we do naive nearest-neighbor resampling.
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // Number of frames (one sample per channel) the device wants
                    let device_frames = data.len() / device_ch as usize;

                    // How many mono 48kHz samples correspond to these frames
                    let lk_samples =
                        (device_frames as u64 * LK_SAMPLE_RATE as u64 / device_sr as u64) as usize;
                    let lk_samples = lk_samples.max(1);

                    let mut buf = vec![0i16; lk_samples];
                    playout_buffer.pull_samples(&mut buf);

                    // Write to output: resample + mono→stereo expansion + i16→f32
                    for frame_idx in 0..device_frames {
                        let src_idx = if device_sr == LK_SAMPLE_RATE {
                            frame_idx
                        } else {
                            (frame_idx as u64 * lk_samples as u64 / device_frames as u64) as usize
                        };
                        let src_idx = src_idx.min(lk_samples - 1);
                        let sample_f32 = buf[src_idx] as f32 / 32768.0;

                        // Duplicate mono to all channels
                        for ch in 0..device_ch as usize {
                            data[frame_idx * device_ch as usize + ch] = sample_f32;
                        }
                    }
                },
                |err| {
                    tracing::error!("audio playout stream error: {err}");
                },
                None,
            )
            .map_err(|e| format!("build output stream: {e}"))?;

        stream.play().map_err(|e| format!("play output stream: {e}"))?;
        tracing::info!("cpal audio playout started");

        Ok(Self {
            _stream: SendSyncStream(stream),
        })
    }
}

// ---------------------------------------------------------------------------
// Capture — microphone → NativeAudioSource
// ---------------------------------------------------------------------------

pub struct CpalAudioCapture {
    _stream: SendSyncStream,
    running: Arc<AtomicBool>,
}

impl CpalAudioCapture {
    pub fn start(audio_source: NativeAudioSource) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no input audio device available")?;

        let default_cfg = device
            .default_input_config()
            .map_err(|e| format!("default input config: {e}"))?;

        let device_sr = default_cfg.sample_rate().0;
        let device_ch = default_cfg.channels();

        tracing::info!(
            "audio capture: device={:?}, rate={device_sr}, channels={device_ch}, format={:?}",
            device.name(),
            default_cfg.sample_format(),
        );

        let config = cpal::StreamConfig {
            channels: device_ch,
            sample_rate: cpal::SampleRate(device_sr),
            buffer_size: cpal::BufferSize::Default,
        };

        let running = Arc::new(AtomicBool::new(true));
        let running_flag = running.clone();

        // capture_frame is async — use a dedicated single-thread runtime
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("audio capture runtime: {e}"))?;

        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !running_flag.load(Ordering::Relaxed) {
                        return;
                    }

                    let device_frames = data.len() / device_ch as usize;

                    // Resample to 48kHz mono i16
                    let lk_frames = if device_sr == LK_SAMPLE_RATE {
                        device_frames
                    } else {
                        (device_frames as u64 * LK_SAMPLE_RATE as u64 / device_sr as u64) as usize
                    };
                    let lk_frames = lk_frames.max(1);

                    let mut pcm = vec![0i16; lk_frames];
                    for i in 0..lk_frames {
                        let src_frame = if device_sr == LK_SAMPLE_RATE {
                            i
                        } else {
                            (i as u64 * device_frames as u64 / lk_frames as u64) as usize
                        };
                        let src_frame = src_frame.min(device_frames - 1);

                        // Average all channels → mono
                        let mut sum = 0.0f32;
                        for ch in 0..device_ch as usize {
                            sum += data[src_frame * device_ch as usize + ch];
                        }
                        let mono = sum / device_ch as f32;
                        pcm[i] = (mono * 32767.0).clamp(-32768.0, 32767.0) as i16;
                    }

                    let frame = AudioFrame {
                        data: pcm.into(),
                        sample_rate: LK_SAMPLE_RATE,
                        num_channels: LK_CHANNELS,
                        samples_per_channel: lk_frames as u32,
                    };

                    let _ = rt.block_on(audio_source.capture_frame(&frame));
                },
                |err| {
                    tracing::error!("audio capture stream error: {err}");
                },
                None,
            )
            .map_err(|e| format!("build input stream: {e}"))?;

        stream.play().map_err(|e| format!("play input stream: {e}"))?;
        tracing::info!("cpal audio capture started");

        Ok(Self {
            _stream: SendSyncStream(stream),
            running,
        })
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        tracing::info!("cpal audio capture stopped");
    }
}
