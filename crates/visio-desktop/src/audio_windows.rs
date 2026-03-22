//! Windows audio engine using WASAPI Communications mode.
//!
//! Separate IAudioClient instances for capture and render, both set to
//! AudioCategory_Communications to activate Windows Audio DSP (AEC + AGC + NS).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use livekit::webrtc::audio_source::native::NativeAudioSource;
use visio_core::{AudioCaptureBuffer, AudioPlayoutBuffer, CapturedFrame};

use super::audio_engine::{self, LK_CHANNELS, LK_SAMPLE_RATE, VoiceAudioEngine};

use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;

pub struct WindowsAudioEngine {
    render_thread: Option<std::thread::JoinHandle<()>>,
    capture_thread: Option<std::thread::JoinHandle<()>>,
    render_stop: Arc<AtomicBool>,
    capture_stop: Arc<AtomicBool>,
    drain_running: Option<Arc<AtomicBool>>,
    _input_device: Option<String>,
    _output_device: Option<String>,
}

unsafe impl Send for WindowsAudioEngine {}
unsafe impl Sync for WindowsAudioEngine {}

impl WindowsAudioEngine {
    pub fn new(input_device: Option<&str>, output_device: Option<&str>) -> Self {
        Self {
            render_thread: None,
            capture_thread: None,
            render_stop: Arc::new(AtomicBool::new(false)),
            capture_stop: Arc::new(AtomicBool::new(false)),
            drain_running: None,
            _input_device: input_device.map(String::from),
            _output_device: output_device.map(String::from),
        }
    }
}

/// Map a windows::core::Error to String.
fn w(r: windows::core::Result<()>) -> Result<(), String> {
    r.map_err(|e| format!("{e}"))
}

/// Initialize COM on the current thread (each thread needs its own init).
fn init_com() -> Result<(), String> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .map_err(|e| format!("{e}"))
}

/// Get the default audio endpoint for the given data flow.
fn get_default_device(data_flow: EDataFlow) -> windows::core::Result<IMMDevice> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        enumerator.GetDefaultAudioEndpoint(data_flow, eCommunications)
    }
}

/// Create and configure an IAudioClient with Communications category.
fn create_audio_client(device: &IMMDevice) -> windows::core::Result<IAudioClient2> {
    unsafe {
        let client: IAudioClient2 = device.Activate(CLSCTX_ALL, None)?;

        // Set communications category — this activates AEC DSP
        let props = AudioClientProperties {
            cbSize: std::mem::size_of::<AudioClientProperties>() as u32,
            bIsOffload: false.into(),
            eCategory: AudioCategory_Communications,
            Options: AUDCLNT_STREAMOPTIONS_NONE,
        };
        client.SetClientProperties(&props)?;

        Ok(client)
    }
}

/// Run a WASAPI closure, mapping errors to String.
fn run_wasapi(f: impl FnOnce() -> windows::core::Result<()>) -> Result<(), String> {
    f().map_err(|e| format!("{e}"))
}

/// Polling interval for WASAPI shared-mode buffer checks (~5ms).
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Run the WASAPI render loop: pull from `buffer`, resample, and write to `render_client`.
fn wasapi_render_loop(
    stop: Arc<AtomicBool>,
    buffer: Arc<AudioPlayoutBuffer>,
    render_client: IAudioRenderClient,
    client: IAudioClient2,
    buffer_size: usize,
    device_channels: usize,
    device_rate: u32,
) -> windows::core::Result<()> {
    unsafe { client.Start()? };

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(POLL_INTERVAL);

        let padding = unsafe { client.GetCurrentPadding()? } as usize;
        let available = buffer_size - padding;
        if available == 0 {
            continue;
        }

        // Pull mono i16 samples at LK rate, resample to device rate
        let lk_samples = (available as f64 * LK_SAMPLE_RATE as f64 / device_rate as f64) as usize;
        let mut mono_i16 = vec![0i16; lk_samples];
        buffer.pull_samples(&mut mono_i16);

        let resampled = audio_engine::linear_resample(&mono_i16, available);

        // Convert to f32 and expand to device channels
        let buf_ptr = unsafe { render_client.GetBuffer(available as u32)? };
        let dst = unsafe {
            std::slice::from_raw_parts_mut(buf_ptr as *mut f32, available * device_channels)
        };
        for (i, &s) in resampled.iter().enumerate() {
            let f = s as f32 / 32768.0;
            for ch in 0..device_channels {
                dst[i * device_channels + ch] = f;
            }
        }

        unsafe { render_client.ReleaseBuffer(available as u32, 0)? };
    }

    unsafe { client.Stop()? };
    Ok(())
}

impl VoiceAudioEngine for WindowsAudioEngine {
    fn start_playout(&mut self, buffer: Arc<AudioPlayoutBuffer>) -> Result<(), String> {
        let stop = self.render_stop.clone();
        stop.store(false, Ordering::Relaxed);

        let handle = std::thread::spawn(move || {
            if let Err(e) = init_com() {
                tracing::error!("COM init failed in render thread: {e}");
                return;
            }

            let result = run_wasapi(|| {
                let device = get_default_device(eRender)?;
                let client = create_audio_client(&device)?;

                // Get mix format and initialize in shared mode (polling)
                let mix_format = unsafe { client.GetMixFormat()? };
                unsafe {
                    client.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        0, // no event callback — we poll instead
                        0, // buffer duration (0 = default)
                        0, // periodicity
                        mix_format,
                        None,
                    )?;
                }

                let render_client: IAudioRenderClient = unsafe { client.GetService()? };
                let buffer_size = unsafe { client.GetBufferSize()? } as usize;
                let format = unsafe { &*mix_format };
                let device_channels = format.nChannels as usize;
                let device_rate = format.nSamplesPerSec;

                wasapi_render_loop(
                    stop,
                    buffer,
                    render_client,
                    client,
                    buffer_size,
                    device_channels,
                    device_rate,
                )
            });

            if let Err(e) = result {
                tracing::error!("WASAPI render thread error: {e}");
            }
        });

        self.render_thread = Some(handle);
        tracing::info!("Windows WASAPI Communications playout started");
        Ok(())
    }

    fn start_capture(
        &mut self,
        source: NativeAudioSource,
        noise_reduction: bool,
    ) -> Result<(), String> {
        let stop = self.capture_stop.clone();
        stop.store(false, Ordering::Relaxed);

        let capture_buffer = Arc::new(AudioCaptureBuffer::new(50));
        let drain_running =
            audio_engine::start_drain_thread(capture_buffer.clone(), source, noise_reduction);

        let handle = std::thread::spawn(move || {
            if let Err(e) = init_com() {
                tracing::error!("COM init failed in capture thread: {e}");
                return;
            }

            let result = run_wasapi(|| {
                let device = get_default_device(eCapture)?;
                let client = create_audio_client(&device)?;

                let mix_format = unsafe { client.GetMixFormat()? };

                unsafe {
                    client.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        0, // polling mode
                        0,
                        0,
                        mix_format,
                        None,
                    )?;
                }

                let capture_client: IAudioCaptureClient = unsafe { client.GetService()? };
                let format = unsafe { &*mix_format };
                let device_channels = format.nChannels as usize;
                let device_rate = format.nSamplesPerSec;

                unsafe { client.Start()? };

                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(POLL_INTERVAL);

                    let mut packet_size = unsafe { capture_client.GetNextPacketSize()? };
                    while packet_size > 0 {
                        let mut buf_ptr = std::ptr::null_mut();
                        let mut num_frames = 0u32;
                        let mut flags = 0u32;

                        unsafe {
                            capture_client.GetBuffer(
                                &mut buf_ptr,
                                &mut num_frames,
                                &mut flags,
                                None,
                                None,
                            )?;
                        }

                        let frames = num_frames as usize;
                        let src = unsafe {
                            std::slice::from_raw_parts(
                                buf_ptr as *const f32,
                                frames * device_channels,
                            )
                        };

                        // Mix to mono f32
                        let mono = audio_engine::mix_to_mono(src, device_channels);

                        // Convert f32 → i16
                        let mono_i16: Vec<i16> = mono
                            .iter()
                            .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
                            .collect();

                        // Resample to 48kHz
                        let target_len = (mono_i16.len() as f64 * LK_SAMPLE_RATE as f64
                            / device_rate as f64) as usize;
                        let resampled = audio_engine::linear_resample(&mono_i16, target_len);

                        let frame = CapturedFrame {
                            pcm: resampled,
                            sample_rate: LK_SAMPLE_RATE,
                            num_channels: LK_CHANNELS,
                            samples_per_channel: target_len as u32,
                        };
                        capture_buffer.push(frame);

                        unsafe { capture_client.ReleaseBuffer(num_frames)? };
                        packet_size = unsafe { capture_client.GetNextPacketSize()? };
                    }
                }

                unsafe { client.Stop()? };
                Ok(())
            });

            if let Err(e) = result {
                tracing::error!("WASAPI capture thread error: {e}");
            }
        });

        self.capture_thread = Some(handle);
        self.drain_running = Some(drain_running);
        tracing::info!("Windows WASAPI Communications capture started");
        Ok(())
    }

    fn stop_capture(&mut self) {
        self.capture_stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.capture_thread.take() {
            let _ = h.join();
        }
        if let Some(r) = self.drain_running.take() {
            r.store(false, Ordering::Relaxed);
        }
        tracing::info!("Windows audio capture stopped");
    }

    fn stop_playout(&mut self) {
        self.stop_capture();
        self.render_stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.render_thread.take() {
            let _ = h.join();
        }
        tracing::info!("Windows WASAPI stopped");
    }

    fn set_device_change_callback(&mut self, _callback: super::audio_engine::DeviceChangeCallback) {
        tracing::warn!("device change detection not yet implemented on Windows");
    }
}

impl Drop for WindowsAudioEngine {
    fn drop(&mut self) {
        self.stop_playout();
    }
}
