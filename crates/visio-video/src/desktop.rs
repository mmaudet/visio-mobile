//! Desktop video renderer — converts I420 frames to JPEG base64.
//!
//! Emits frames via a registered callback so the Tauri app can
//! forward them to the frontend as events.

use std::ffi::c_void;
use std::sync::OnceLock;

use image::codecs::jpeg::JpegEncoder;
use image::{ImageBuffer, Rgb};
use livekit::webrtc::prelude::BoxVideoFrame;

/// Callback type: (track_sid, base64_data, data_len, width, height, user_data)
type FrameCallback = unsafe extern "C" fn(
    track_sid: *const std::ffi::c_char,
    data: *const u8,
    data_len: usize,
    width: u32,
    height: u32,
    user_data: *mut c_void,
);

struct CallbackInfo {
    callback: FrameCallback,
    user_data: *mut c_void,
}

// SAFETY: user_data is managed by the caller (Tauri side) and only
// accessed from the callback which runs on a single thread.
unsafe impl Send for CallbackInfo {}
unsafe impl Sync for CallbackInfo {}

static CALLBACK: OnceLock<CallbackInfo> = OnceLock::new();

/// Register a callback for receiving video frames on desktop.
///
/// # Safety
/// `user_data` must be valid for the lifetime of the application.
/// `callback` must be a valid function pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_set_desktop_callback(
    callback: FrameCallback,
    user_data: *mut c_void,
) {
    let _ = CALLBACK.set(CallbackInfo {
        callback,
        user_data,
    });
}

/// Render a single I420 frame by converting to JPEG and calling the callback.
pub(crate) fn render_frame(
    frame: &BoxVideoFrame,
    _surface: *mut c_void,
    track_sid: &str,
) {
    let Some(cb) = CALLBACK.get() else {
        return;
    };

    let buffer = &frame.buffer;
    let width = buffer.width();
    let height = buffer.height();

    // Convert to I420 for plane access (handles Native buffers too).
    let i420 = buffer.to_i420();
    let (y_data, u_data, v_data) = i420.data();
    let (stride_y, stride_u, stride_v) = i420.strides();

    let w = width as usize;
    let h = height as usize;

    // I420 → RGB conversion (BT.601)
    let mut rgb = vec![0u8; w * h * 3];

    for row in 0..h {
        for col in 0..w {
            let y_idx = row * stride_y as usize + col;
            let u_idx = (row / 2) * stride_u as usize + (col / 2);
            let v_idx = (row / 2) * stride_v as usize + (col / 2);

            let y = y_data[y_idx] as f32;
            let u = u_data[u_idx] as f32 - 128.0;
            let v = v_data[v_idx] as f32 - 128.0;

            let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;

            let out_idx = (row * w + col) * 3;
            rgb[out_idx] = r;
            rgb[out_idx + 1] = g;
            rgb[out_idx + 2] = b;
        }
    }

    // Encode as JPEG (quality 60 — good balance of size vs. quality).
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgb).expect("buffer size mismatch");

    let mut jpeg_buf = Vec::with_capacity(w * h / 4);
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, 60);
    if encoder.encode_image(&img).is_err() {
        tracing::warn!("JPEG encode failed for track {track_sid}");
        return;
    }

    // Base64 encode
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_buf);

    // Deliver via callback
    let sid_cstr = std::ffi::CString::new(track_sid).unwrap();
    unsafe {
        (cb.callback)(
            sid_cstr.as_ptr(),
            b64.as_ptr(),
            b64.len(),
            width,
            height,
            cb.user_data,
        );
    }
}
