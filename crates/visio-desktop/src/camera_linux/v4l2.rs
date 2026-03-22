//! V4L2 camera capture backend for Linux.
//!
//! Opens the camera via Video4Linux2, captures MJPEG or YUYV frames,
//! converts to I420, and feeds them into a LiveKit NativeVideoSource.
//! Used as fallback when the PipeWire Camera portal is not available.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use livekit::webrtc::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use v4l::buffer::Type;
use v4l::io::mmap::Stream;
use v4l::io::traits::CaptureStream;
use v4l::video::Capture;
use v4l::{Device, FourCC};

use super::VideoDeviceInfo;
use super::convert;

// ---------------------------------------------------------------------------
// Video device enumeration
// ---------------------------------------------------------------------------

/// List available V4L2 video capture devices.
pub fn list_cameras() -> Vec<VideoDeviceInfo> {
    let mut devices = Vec::new();
    for i in 0..10 {
        let path = format!("/dev/video{}", i);
        if let Ok(dev) = Device::new(i) {
            if let Ok(caps) = dev.query_caps() {
                if caps
                    .capabilities
                    .contains(v4l::capability::Flags::VIDEO_CAPTURE)
                {
                    devices.push(VideoDeviceInfo {
                        name: caps.card.clone(),
                        unique_id: path,
                        is_default: devices.is_empty(),
                    });
                }
            }
        }
    }
    devices
}

// ---------------------------------------------------------------------------
// V4L2 camera capture
// ---------------------------------------------------------------------------

pub struct V4lCameraCapture {
    running: Arc<AtomicBool>,
    _capture_thread: JoinHandle<()>,
}

impl V4lCameraCapture {
    pub fn start(device_index: usize, source: NativeVideoSource) -> Result<Self, String> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let capture_thread = thread::Builder::new()
            .name("visio-camera-v4l2".into())
            .spawn(move || {
                if let Err(e) = capture_loop(device_index, source, running_clone) {
                    tracing::error!("V4L2 camera capture error: {e}");
                }
            })
            .map_err(|e| format!("Failed to spawn V4L2 camera thread: {e}"))?;

        tracing::info!("V4L2 camera capture started (device index {device_index})");

        Ok(V4lCameraCapture {
            running,
            _capture_thread: capture_thread,
        })
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        tracing::info!("V4L2 camera capture stopped");
    }
}

impl Drop for V4lCameraCapture {
    fn drop(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            self.stop();
        }
    }
}

fn capture_loop(
    device_index: usize,
    source: NativeVideoSource,
    running: Arc<AtomicBool>,
) -> Result<(), String> {
    let dev = Device::new(device_index).map_err(|e| format!("Failed to open camera: {e}"))?;

    let format = dev
        .format()
        .map_err(|e| format!("Failed to get format: {e}"))?;
    tracing::info!(
        "V4L2 camera format: {}x{} {:?}",
        format.width,
        format.height,
        format.fourcc
    );

    let format = if format.width < 320 || format.height < 240 {
        let mut new_format = format.clone();
        new_format.width = 640;
        new_format.height = 480;
        match dev.set_format(&new_format) {
            Ok(f) => {
                tracing::info!("V4L2 format updated to: {}x{}", f.width, f.height);
                f
            }
            Err(e) => {
                tracing::warn!("Could not update V4L2 format: {e}, using current");
                format
            }
        }
    } else {
        format
    };

    let width = format.width;
    let height = format.height;
    let fourcc = format.fourcc;

    let mut stream = Stream::with_buffers(&dev, Type::VideoCapture, 4)
        .map_err(|e| format!("Failed to create V4L2 stream: {e}"))?;

    let mut frame_count: u64 = 0;

    while running.load(Ordering::SeqCst) {
        let (buf, _meta) = match stream.next() {
            Ok(frame) => frame,
            Err(e) => {
                tracing::warn!("V4L2 frame capture error: {e}");
                continue;
            }
        };

        let mut i420 = I420Buffer::new(width, height);

        let converted = if fourcc == FourCC::new(b"MJPG") {
            match convert::decode_mjpeg(buf) {
                Ok(rgb) => {
                    let strides = i420.strides();
                    let (y, u, v) = i420.data_mut();
                    convert::rgb_to_i420(
                        &rgb,
                        width as usize,
                        height as usize,
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
                    tracing::warn!("MJPEG decode error: {e}");
                    false
                }
            }
        } else if fourcc == FourCC::new(b"YUYV") {
            let strides = i420.strides();
            let (y, u, v) = i420.data_mut();
            convert::yuyv_to_i420(
                buf,
                width as usize,
                height as usize,
                y,
                strides.0 as usize,
                u,
                strides.1 as usize,
                v,
                strides.2 as usize,
            );
            true
        } else {
            tracing::warn!("Unsupported V4L2 format: {:?}", fourcc);
            false
        };

        if !converted {
            continue;
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
        source.capture_frame(&video_frame);

        frame_count += 1;
        if frame_count % 3 == 0 {
            visio_video::render_local_i420(&video_frame.buffer, "local-camera");
        }
        if frame_count == 1 {
            tracing::info!("First V4L2 camera frame captured");
        }
    }

    Ok(())
}
