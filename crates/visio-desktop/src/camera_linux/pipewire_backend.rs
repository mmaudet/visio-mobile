//! PipeWire Camera portal backend.
//!
//! Uses ashpd to request camera access via XDG portal (consent dialog),
//! then opens a PipeWire stream to receive video frames. This enables
//! proper Flatpak sandboxing without --device=all.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};

use livekit::webrtc::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;

use super::VideoDeviceInfo;
use super::convert;

/// Return a single synthetic camera entry — the portal handles device selection.
pub fn list_cameras() -> Vec<VideoDeviceInfo> {
    vec![VideoDeviceInfo {
        name: "Camera (PipeWire)".to_string(),
        unique_id: "pipewire:camera".to_string(),
        is_default: true,
    }]
}

pub struct PipewireCameraCapture {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl PipewireCameraCapture {
    /// Start camera capture via XDG Camera portal + PipeWire stream.
    /// Shows a consent dialog on first use (cached by the portal afterwards).
    pub fn start(source: NativeVideoSource) -> Result<Self, String> {
        // Request camera access via portal on a dedicated thread
        // (ashpd is async, and we may be called from a tokio context)
        let pw_fd = thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio runtime: {e}"))?;
            rt.block_on(async {
                let camera = ashpd::desktop::camera::Camera::new()
                    .await
                    .map_err(|e| format!("camera portal: {e}"))?;
                camera
                    .request_access()
                    .await
                    .map_err(|e| format!("camera access denied: {e}"))?;
                let fd = camera
                    .open_pipe_wire_remote()
                    .await
                    .map_err(|e| format!("pipewire remote: {e}"))?;
                Ok::<_, String>(fd)
            })
        })
        .join()
        .map_err(|_| "portal thread panicked".to_string())??;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let capture_thread = thread::Builder::new()
            .name("visio-camera-pipewire".into())
            .spawn(move || {
                if let Err(e) = pipewire_capture_loop(pw_fd, source, running_clone) {
                    tracing::error!("PipeWire camera capture error: {e}");
                }
            })
            .map_err(|e| format!("Failed to spawn PipeWire thread: {e}"))?;

        tracing::info!("PipeWire camera capture started");

        Ok(Self {
            running,
            thread: Some(capture_thread),
        })
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        tracing::info!("PipeWire camera capture stopped");
    }
}

impl Drop for PipewireCameraCapture {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.stop();
        }
    }
}

fn pipewire_capture_loop(
    pw_fd: std::os::fd::OwnedFd,
    source: NativeVideoSource,
    running: Arc<AtomicBool>,
) -> Result<(), String> {
    use pipewire::context::ContextBox;
    use pipewire::main_loop::MainLoopBox;

    let mainloop = MainLoopBox::new(None).map_err(|e| format!("MainLoopBox::new: {e}"))?;
    let context =
        ContextBox::new(&mainloop.loop_(), None).map_err(|e| format!("ContextBox::new: {e}"))?;

    let core = context
        .connect_fd(pw_fd, None)
        .map_err(|e| format!("connect_fd: {e}"))?;

    let stream = pipewire::stream::StreamBox::new(
        &core,
        "visio-camera",
        pipewire::properties::properties! {
            *pipewire::keys::MEDIA_TYPE => "Video",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Camera",
        },
    )
    .map_err(|e| format!("StreamBox::new: {e}"))?;

    let frame_count = Arc::new(AtomicU64::new(0));
    let frame_count_cb = frame_count.clone();
    let source_cb = source;

    // Shared state for negotiated video format
    let video_width = Arc::new(AtomicU64::new(0));
    let video_height = Arc::new(AtomicU64::new(0));
    let video_format = Arc::new(AtomicU64::new(0));
    let vw_cb = video_width.clone();
    let vh_cb = video_height.clone();
    let vf_cb = video_format.clone();

    let running_cb = running.clone();

    // Process callback — called for each video frame
    let _listener = stream
        .add_local_listener()
        .param_changed(move |_stream, _user_data: &mut (), id, param| {
            let Some(param) = param else { return };
            if id == pipewire::spa::param::ParamType::Format.as_raw() {
                if let Some((w, h, fmt)) = parse_video_format_pod(param) {
                    vw_cb.store(w as u64, Ordering::SeqCst);
                    vh_cb.store(h as u64, Ordering::SeqCst);
                    vf_cb.store(fmt as u64, Ordering::SeqCst);
                    tracing::info!("PipeWire format negotiated: {w}x{h} format={fmt}");
                }
            }
        })
        .process(move |stream, _user_data: &mut ()| {
            if !running_cb.load(Ordering::Relaxed) {
                return;
            }
            let width = video_width.load(Ordering::SeqCst) as u32;
            let height = video_height.load(Ordering::SeqCst) as u32;
            let format = video_format.load(Ordering::SeqCst) as u32;

            if width == 0 || height == 0 {
                return; // Format not yet negotiated
            }

            let Some(data) = dequeue_pipewire_data(stream) else {
                return;
            };
            let mut i420 = I420Buffer::new(width, height);
            if !convert_spa_frame(&data, format, width as usize, height as usize, &mut i420) {
                return;
            }

            // Apply background blur
            {
                let strides = i420.strides();
                let (y, u, v) = i420.data_mut();
                visio_ffi::blur::BlurProcessor::process_i420(
                    y,
                    u,
                    v,
                    width as usize,
                    height as usize,
                    strides.0 as usize,
                    strides.1 as usize,
                    strides.2 as usize,
                    0,
                );
            }

            let video_frame = VideoFrame {
                rotation: VideoRotation::VideoRotation0,
                timestamp_us: 0,
                buffer: i420,
            };
            source_cb.capture_frame(&video_frame);

            let count = frame_count_cb.fetch_add(1, Ordering::Relaxed);
            if count.is_multiple_of(3) {
                visio_video::render_local_i420(&video_frame.buffer, "local-camera");
            }
            if count == 0 {
                tracing::info!("First PipeWire camera frame captured");
            }
        })
        .register()
        .map_err(|e| format!("stream listener: {e}"))?;

    // Connect stream — accept any video format the camera offers
    stream
        .connect(
            pipewire::spa::utils::Direction::Input,
            None,
            pipewire::stream::StreamFlags::AUTOCONNECT | pipewire::stream::StreamFlags::MAP_BUFFERS,
            &mut [],
        )
        .map_err(|e| format!("stream connect: {e}"))?;

    tracing::info!("PipeWire stream connected, entering main loop");
    // Run the main loop, checking the running flag periodically
    while running.load(Ordering::Relaxed) {
        // Iterate the main loop with a short timeout (10ms) to check running flag
        mainloop
            .loop_()
            .iterate(std::time::Duration::from_millis(10));
    }
    tracing::info!("PipeWire main loop exited");

    Ok(())
}

/// Dequeue a PipeWire buffer and return a copy of its first data chunk.
/// Returns `None` if no buffer is available or the buffer has no data.
fn dequeue_pipewire_data(stream: &pipewire::stream::StreamBox) -> Option<Vec<u8>> {
    let mut buffer = stream.dequeue_buffer()?;
    let buf = buffer.datas_mut().first_mut()?;
    let data = buf.data()?;
    Some(data.to_vec())
}

/// Extract (format, width, height) from SPA object properties.
fn extract_video_format_from_props(
    properties: &[pipewire::spa::pod::Property],
) -> Option<(u32, u32, u32)> {
    use pipewire::spa::pod::Value;
    let mut format: Option<u32> = None;
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;

    for prop in properties {
        match prop.key {
            // SPA_FORMAT_VIDEO_format = 0x20001
            0x20001 => {
                if let Value::Id(id) = &prop.value {
                    format = Some(id.0);
                }
            }
            // SPA_FORMAT_VIDEO_size = 0x20003
            0x20003 => {
                if let Value::Rectangle(rect) = &prop.value {
                    width = Some(rect.width);
                    height = Some(rect.height);
                }
            }
            _ => {}
        }
    }

    match (width, height, format) {
        (Some(w), Some(h), Some(f)) => Some((w, h, f)),
        _ => None,
    }
}

/// Parse a SPA format pod to extract (width, height, video_format).
/// Returns None if parsing fails.
fn parse_video_format_pod(pod: &pipewire::spa::pod::Pod) -> Option<(u32, u32, u32)> {
    use pipewire::spa::pod::deserialize::PodDeserializer;
    let deserializer =
        PodDeserializer::deserialize_from::<pipewire::spa::pod::Value>(pod.as_bytes());
    match deserializer {
        Ok((_, pipewire::spa::pod::Value::Object(obj))) => {
            extract_video_format_from_props(&obj.properties)
        }
        _ => None,
    }
}

/// Convert a SPA video buffer to I420 based on the negotiated format.
fn convert_spa_frame(
    data: &[u8],
    format: u32,
    width: usize,
    height: usize,
    i420: &mut I420Buffer,
) -> bool {
    // SPA video format IDs (from spa/param/video/format.h)
    const SPA_VIDEO_FORMAT_NV12: u32 = 25;
    const SPA_VIDEO_FORMAT_YUY2: u32 = 20;
    const SPA_VIDEO_FORMAT_MJPG: u32 = 1; // encoded
    const SPA_VIDEO_FORMAT_RGB: u32 = 12;

    let strides = i420.strides();
    let (y, u, v) = i420.data_mut();

    if format == SPA_VIDEO_FORMAT_NV12 {
        convert::nv12_to_i420(
            data,
            width,
            height,
            y,
            strides.0 as usize,
            u,
            strides.1 as usize,
            v,
            strides.2 as usize,
        );
        true
    } else if format == SPA_VIDEO_FORMAT_YUY2 {
        convert::yuyv_to_i420(
            data,
            width,
            height,
            y,
            strides.0 as usize,
            u,
            strides.1 as usize,
            v,
            strides.2 as usize,
        );
        true
    } else if format == SPA_VIDEO_FORMAT_MJPG {
        match convert::decode_mjpeg(data) {
            Ok(rgb) => {
                convert::rgb_to_i420(
                    &rgb,
                    width,
                    height,
                    y,
                    strides.0 as usize,
                    u,
                    strides.1 as usize,
                    v,
                    strides.2 as usize,
                );
                true
            }
            Err(e) => {
                tracing::warn!("PipeWire MJPEG decode: {e}");
                false
            }
        }
    } else if format == SPA_VIDEO_FORMAT_RGB {
        convert::rgb_to_i420(
            data,
            width,
            height,
            y,
            strides.0 as usize,
            u,
            strides.1 as usize,
            v,
            strides.2 as usize,
        );
        true
    } else {
        tracing::warn!("Unsupported PipeWire format: {format}");
        false
    }
}
