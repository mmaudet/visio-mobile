//! Android video renderer — writes I420 frames to ANativeWindow.
//!
//! The native (Kotlin) side obtains an `ANativeWindow*` from its
//! `SurfaceView` / `SurfaceTexture` via JNI and passes the raw pointer
//! through `start_track_renderer`.  This module locks the window buffer,
//! converts the incoming I420 video frame to RGBA, writes the pixels,
//! and posts the result.  The `SurfaceView` takes care of display.

use std::ffi::c_void;

use livekit::webrtc::prelude::BoxVideoFrame;
use livekit::webrtc::video_frame::I420Buffer;
use livekit::webrtc::video_frame::VideoBuffer;

/// Render raw I420 planes to an ANativeWindow surface with rotation and mirror.
///
/// Used for local camera self-view: the I420 buffer is already constructed
/// in the JNI capture path, so we skip `NativeVideoStream` and render directly.
///
/// `rotation_degrees` is the camera's `sensorOrientation` (0, 90, 180, 270).
/// `mirror` should be `true` for front-camera self-view (horizontal flip).
///
/// # Safety
/// `surface` must be a valid, non-null `ANativeWindow*`.
pub fn render_i420_to_surface(
    i420: &I420Buffer,
    surface: *mut c_void,
    rotation_degrees: u32,
    mirror: bool,
) {
    let src_w = i420.width() as usize;
    let src_h = i420.height() as usize;
    if src_w == 0 || src_h == 0 {
        return;
    }

    // Video dimensions after rotation.
    let (vid_w, vid_h) = match rotation_degrees {
        90 | 270 => (src_h, src_w),
        _ => (src_w, src_h),
    };

    let (y_data, u_data, v_data) = i420.data();
    let (stride_y, stride_u, stride_v) = i420.strides();
    let y_stride = stride_y as usize;
    let u_stride = stride_u as usize;
    let v_stride = stride_v as usize;

    let window = surface as *mut ndk_sys::ANativeWindow;

    unsafe {
        // Use the SurfaceView's actual dimensions so Android doesn't stretch.
        let surf_w = ndk_sys::ANativeWindow_getWidth(window) as usize;
        let surf_h = ndk_sys::ANativeWindow_getHeight(window) as usize;
        if surf_w == 0 || surf_h == 0 {
            return;
        }

        let result = ndk_sys::ANativeWindow_setBuffersGeometry(
            window,
            surf_w as i32,
            surf_h as i32,
            1, // WINDOW_FORMAT_RGBA_8888
        );
        if result != 0 {
            return;
        }

        let mut native_buf = std::mem::MaybeUninit::<ndk_sys::ANativeWindow_Buffer>::uninit();
        let lock_result = ndk_sys::ANativeWindow_lock(
            window,
            native_buf.as_mut_ptr(),
            std::ptr::null_mut(),
        );
        if lock_result != 0 {
            return;
        }

        let native_buf = native_buf.assume_init();
        let dst_stride = native_buf.stride as usize;
        let bits = native_buf.bits as *mut u8;

        // Clear to opaque black — RGBA(0,0,0,255) = 0xFF000000 on little-endian.
        let pixels = bits as *mut u32;
        for i in 0..(surf_h * dst_stride) {
            *pixels.add(i) = 0xFF000000u32;
        }

        // Fit video inside surface preserving aspect ratio (letterbox).
        let scale = (surf_w as f64 / vid_w as f64).min(surf_h as f64 / vid_h as f64);
        let render_w = (vid_w as f64 * scale) as usize;
        let render_h = (vid_h as f64 * scale) as usize;
        let off_x = (surf_w - render_w) / 2;
        let off_y = (surf_h - render_h) / 2;

        for out_row in 0..render_h {
            for out_col in 0..render_w {
                // Nearest-neighbour scale to video coordinates.
                let vid_col = out_col * vid_w / render_w;
                let vid_row = out_row * vid_h / render_h;

                // Apply mirror (horizontal flip).
                let vc = if mirror { vid_w - 1 - vid_col } else { vid_col };

                // Map rotated video pixel back to source coordinates.
                let (sr, sc) = match rotation_degrees {
                    90 => (src_h - 1 - vc, vid_row),
                    180 => (src_h - 1 - vid_row, src_w - 1 - vc),
                    270 => (vc, src_w - 1 - vid_row),
                    _ => (vid_row, vc),
                };

                let y_idx = sr * y_stride + sc;
                let u_idx = (sr / 2) * u_stride + (sc / 2);
                let v_idx = (sr / 2) * v_stride + (sc / 2);

                let y = y_data[y_idx] as f32;
                let u = u_data[u_idx] as f32 - 128.0;
                let v = v_data[v_idx] as f32 - 128.0;

                let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
                let g = (y - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0) as u8;
                let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;

                let dx = out_col + off_x;
                let dy = out_row + off_y;
                let out_offset = (dy * dst_stride + dx) * 4;
                *bits.add(out_offset) = r;
                *bits.add(out_offset + 1) = g;
                *bits.add(out_offset + 2) = b;
                *bits.add(out_offset + 3) = 255;
            }
        }

        ndk_sys::ANativeWindow_unlockAndPost(window);
    }
}

/// Render a single I420 frame to an ANativeWindow surface.
///
/// # Arguments
/// * `frame`     — the video frame from the LiveKit NativeVideoStream
/// * `surface`   — an `ANativeWindow*` obtained via `ANativeWindow_fromSurface()`
/// * `track_sid` — identifies which track this frame belongs to (for logging)
///
/// # Safety contract (upheld by caller)
/// `surface` must be a valid, non-null `ANativeWindow*` that remains alive for
/// the duration of this call.  The frame loop in `lib.rs` guarantees this.
pub(crate) fn render_frame(
    frame: &BoxVideoFrame,
    surface: *mut c_void,
    _track_sid: &str,
) {
    let buffer = &frame.buffer;
    let width = buffer.width() as usize;
    let height = buffer.height() as usize;

    if width == 0 || height == 0 {
        return;
    }

    // Convert native buffer to I420 (may be a no-op if already I420).
    let i420 = buffer.to_i420();
    let (y_data, u_data, v_data) = i420.data();
    let (stride_y, stride_u, stride_v) = i420.strides();
    let y_stride = stride_y as usize;
    let u_stride = stride_u as usize;
    let v_stride = stride_v as usize;

    let window = surface as *mut ndk_sys::ANativeWindow;

    unsafe {
        // Configure the buffer geometry to match the incoming frame.
        // WINDOW_FORMAT_RGBA_8888 = 1
        let result = ndk_sys::ANativeWindow_setBuffersGeometry(
            window,
            width as i32,
            height as i32,
            1, // AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM / WINDOW_FORMAT_RGBA_8888
        );
        if result != 0 {
            tracing::warn!("ANativeWindow_setBuffersGeometry failed: {result}");
            return;
        }

        // Lock the surface buffer for writing.
        let mut native_buf = std::mem::MaybeUninit::<ndk_sys::ANativeWindow_Buffer>::uninit();
        let lock_result = ndk_sys::ANativeWindow_lock(
            window,
            native_buf.as_mut_ptr(),
            std::ptr::null_mut(), // no dirty rect — redraw everything
        );
        if lock_result != 0 {
            tracing::warn!("ANativeWindow_lock failed: {lock_result}");
            return;
        }

        let native_buf = native_buf.assume_init();
        let dst_stride = native_buf.stride as usize; // in pixels (RGBA = 4 bytes each)
        let bits = native_buf.bits as *mut u8;

        // ---------------------------------------------------------------
        // I420 → RGBA conversion (BT.601 full-range)
        //
        // Y  is full-resolution (width x height),  stride = y_stride
        // U,V are half-resolution (width/2 x height/2), strides u/v
        //
        // R = Y + 1.402 * (V - 128)
        // G = Y - 0.344136 * (U - 128) - 0.714136 * (V - 128)
        // B = Y + 1.772 * (U - 128)
        // A = 255
        // ---------------------------------------------------------------
        for row in 0..height {
            for col in 0..width {
                let y_idx = row * y_stride + col;
                let u_idx = (row / 2) * u_stride + (col / 2);
                let v_idx = (row / 2) * v_stride + (col / 2);

                let y = y_data[y_idx] as f32;
                let u = u_data[u_idx] as f32 - 128.0;
                let v = v_data[v_idx] as f32 - 128.0;

                let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
                let g = (y - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0) as u8;
                let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;

                // dst_stride is in pixels; each pixel is 4 bytes (RGBA).
                let out_offset = (row * dst_stride + col) * 4;
                *bits.add(out_offset) = r;
                *bits.add(out_offset + 1) = g;
                *bits.add(out_offset + 2) = b;
                *bits.add(out_offset + 3) = 255; // fully opaque
            }
        }

        // Post the buffer to the display.
        ndk_sys::ANativeWindow_unlockAndPost(window);
    }
}
