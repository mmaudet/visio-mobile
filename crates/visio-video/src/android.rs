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

/// Lock an ANativeWindow for writing, returning `(bits, dst_stride)`.
/// Returns `None` if geometry setup or locking fails, in which case the surface
/// is already unlocked/posted as needed.
///
/// # Safety
/// `window` must be a valid, non-null `ANativeWindow*`.
unsafe fn lock_surface_buffer(
    window: *mut ndk_sys::ANativeWindow,
    surf_w: usize,
    surf_h: usize,
) -> Option<(*mut u8, usize)> {
    let result = unsafe {
        ndk_sys::ANativeWindow_setBuffersGeometry(
            window,
            surf_w as i32,
            surf_h as i32,
            1, // WINDOW_FORMAT_RGBA_8888
        )
    };
    if result != 0 {
        return None;
    }

    let mut native_buf = std::mem::MaybeUninit::<ndk_sys::ANativeWindow_Buffer>::uninit();
    let lock_result = unsafe {
        ndk_sys::ANativeWindow_lock(window, native_buf.as_mut_ptr(), std::ptr::null_mut())
    };
    if lock_result != 0 {
        return None;
    }

    let native_buf = unsafe { native_buf.assume_init() };
    let dst_stride = native_buf.stride as usize;
    let bits = native_buf.bits as *mut u8;

    if dst_stride < surf_w {
        unsafe { ndk_sys::ANativeWindow_unlockAndPost(window) };
        return None;
    }

    Some((bits, dst_stride))
}

/// Write rotated and mirrored I420 pixels to an RGBA surface buffer.
///
/// # Safety
/// `bits` must point to a locked ANativeWindow buffer with at least
/// `surf_h * dst_stride * 4` bytes available.
#[allow(clippy::too_many_arguments)]
unsafe fn write_i420_rotated_pixels(
    bits: *mut u8,
    dst_stride: usize,
    y_data: &[u8],
    u_data: &[u8],
    v_data: &[u8],
    y_stride: usize,
    u_stride: usize,
    v_stride: usize,
    src_w: usize,
    src_h: usize,
    surf_w: usize,
    surf_h: usize,
    rotation_degrees: u32,
    mirror: bool,
) {
    // Video dimensions after rotation.
    let (vid_w, vid_h) = match rotation_degrees {
        90 | 270 => (src_h, src_w),
        _ => (src_w, src_h),
    };

    // Cover-fill: scale so video fills the surface, cropping the center.
    let scale = (surf_w as f64 / vid_w as f64).max(surf_h as f64 / vid_h as f64);
    let render_w = (vid_w as f64 * scale) as usize;
    let render_h = (vid_h as f64 * scale) as usize;
    let off_x = (render_w - surf_w) / 2;
    let off_y = (render_h - surf_h) / 2;

    for out_row in 0..surf_h {
        for out_col in 0..surf_w {
            let vid_col = ((out_col + off_x) * vid_w / render_w).min(vid_w - 1);
            let vid_row = ((out_row + off_y) * vid_h / render_h).min(vid_h - 1);
            let vc = if mirror { vid_w - 1 - vid_col } else { vid_col };

            let (sr, sc) = match rotation_degrees {
                90 => (src_h - 1 - vc, vid_row),
                180 => (src_h - 1 - vid_row, src_w - 1 - vc),
                270 => (vc, src_w - 1 - vid_row),
                _ => (vid_row, vc),
            };

            let y = y_data[sr * y_stride + sc] as f32;
            let u = u_data[(sr / 2) * u_stride + (sc / 2)] as f32 - 128.0;
            let v = v_data[(sr / 2) * v_stride + (sc / 2)] as f32 - 128.0;

            let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;

            let out_offset = (out_row * dst_stride + out_col) * 4;
            debug_assert!(out_offset + 3 < surf_h * dst_stride * 4);
            unsafe {
                *bits.add(out_offset) = r;
                *bits.add(out_offset + 1) = g;
                *bits.add(out_offset + 2) = b;
                *bits.add(out_offset + 3) = 255;
            }
        }
    }
}

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

    let (y_data, u_data, v_data) = i420.data();
    let (stride_y, stride_u, stride_v) = i420.strides();
    let y_stride = stride_y as usize;
    let u_stride = stride_u as usize;
    let v_stride = stride_v as usize;

    let window = surface as *mut ndk_sys::ANativeWindow;

    unsafe {
        let surf_w = ndk_sys::ANativeWindow_getWidth(window) as usize;
        let surf_h = ndk_sys::ANativeWindow_getHeight(window) as usize;
        if surf_w == 0 || surf_h == 0 {
            return;
        }

        let Some((bits, dst_stride)) = lock_surface_buffer(window, surf_w, surf_h) else {
            return;
        };

        // Clear to opaque black
        let pixels = bits as *mut u32;
        for i in 0..(surf_h * dst_stride) {
            *pixels.add(i) = 0xFF000000u32;
        }

        write_i420_rotated_pixels(
            bits,
            dst_stride,
            y_data,
            u_data,
            v_data,
            y_stride,
            u_stride,
            v_stride,
            src_w,
            src_h,
            surf_w,
            surf_h,
            rotation_degrees,
            mirror,
        );

        ndk_sys::ANativeWindow_unlockAndPost(window);
    }
}

/// Compute cover/letterbox scale parameters.
/// Returns `(loop_w, loop_h, off_x, off_y, pad_x, pad_y, render_w, render_h)`.
fn compute_scale_params(
    surf_w: usize,
    surf_h: usize,
    width: usize,
    height: usize,
    is_screencast: bool,
) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    let scale_w = surf_w as f64 / width as f64;
    let scale_h = surf_h as f64 / height as f64;
    let scale = if is_screencast {
        scale_w.min(scale_h)
    } else {
        scale_w.max(scale_h)
    };
    let render_w = (width as f64 * scale) as usize;
    let render_h = (height as f64 * scale) as usize;
    let (loop_w, loop_h, off_x, off_y) = if is_screencast {
        (render_w, render_h, 0usize, 0usize)
    } else {
        (
            surf_w,
            surf_h,
            render_w.saturating_sub(surf_w) / 2,
            render_h.saturating_sub(surf_h) / 2,
        )
    };
    let pad_x = if is_screencast {
        surf_w.saturating_sub(render_w) / 2
    } else {
        0
    };
    let pad_y = if is_screencast {
        surf_h.saturating_sub(render_h) / 2
    } else {
        0
    };
    (
        loop_w, loop_h, off_x, off_y, pad_x, pad_y, render_w, render_h,
    )
}

/// Write scaled I420 pixels to an RGBA surface buffer (cover or letterbox).
///
/// # Safety
/// `bits` must point to a locked ANativeWindow buffer.
#[allow(clippy::too_many_arguments)]
unsafe fn write_i420_scaled_pixels(
    bits: *mut u8,
    dst_stride: usize,
    y_data: &[u8],
    u_data: &[u8],
    v_data: &[u8],
    y_stride: usize,
    u_stride: usize,
    v_stride: usize,
    width: usize,
    height: usize,
    surf_h: usize,
    loop_w: usize,
    loop_h: usize,
    off_x: usize,
    off_y: usize,
    pad_x: usize,
    pad_y: usize,
    render_w: usize,
    render_h: usize,
) {
    for out_row in 0..loop_h {
        for out_col in 0..loop_w {
            let src_row = ((out_row + off_y) * height / render_h).min(height - 1);
            let src_col = ((out_col + off_x) * width / render_w).min(width - 1);

            let y = y_data[src_row * y_stride + src_col] as f32;
            let u = u_data[(src_row / 2) * u_stride + (src_col / 2)] as f32 - 128.0;
            let v = v_data[(src_row / 2) * v_stride + (src_col / 2)] as f32 - 128.0;

            let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;

            let out_offset = ((out_row + pad_y) * dst_stride + (out_col + pad_x)) * 4;
            debug_assert!(out_offset + 3 < surf_h * dst_stride * 4);
            unsafe {
                *bits.add(out_offset) = r;
                *bits.add(out_offset + 1) = g;
                *bits.add(out_offset + 2) = b;
                *bits.add(out_offset + 3) = 255;
            }
        }
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
/// Returns `false` if the surface is invalid (destroyed/released),
/// signalling the caller to stop the frame loop.
pub(crate) fn render_frame(
    frame: &BoxVideoFrame,
    surface: *mut c_void,
    _track_sid: &str,
    is_screencast: bool,
) -> bool {
    let buffer = &frame.buffer;
    let width = buffer.width() as usize;
    let height = buffer.height() as usize;

    if width == 0 || height == 0 {
        return true; // Not a surface error, just skip this frame.
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
        // Use the surface's actual dimensions for cover-crop scaling.
        let surf_w = ndk_sys::ANativeWindow_getWidth(window) as usize;
        let surf_h = ndk_sys::ANativeWindow_getHeight(window) as usize;
        if surf_w == 0 || surf_h == 0 {
            // Surface was likely destroyed — signal caller to stop.
            return false;
        }

        let result = ndk_sys::ANativeWindow_setBuffersGeometry(
            window,
            surf_w as i32,
            surf_h as i32,
            1, // WINDOW_FORMAT_RGBA_8888
        );
        if result != 0 {
            tracing::warn!("ANativeWindow_setBuffersGeometry failed: {result}");
            return false;
        }

        // Lock the surface buffer for writing.
        let mut native_buf = std::mem::MaybeUninit::<ndk_sys::ANativeWindow_Buffer>::uninit();
        let lock_result =
            ndk_sys::ANativeWindow_lock(window, native_buf.as_mut_ptr(), std::ptr::null_mut());
        if lock_result != 0 {
            tracing::warn!("ANativeWindow_lock failed: {lock_result}");
            return false;
        }

        let native_buf = native_buf.assume_init();
        let dst_stride = native_buf.stride as usize;
        let bits = native_buf.bits as *mut u8;

        // Validate stride — must be at least surface width for safe pixel writes.
        if dst_stride < surf_w {
            ndk_sys::ANativeWindow_unlockAndPost(window);
            return true; // Odd but not a fatal surface error.
        }

        // Clear to opaque black
        let pixels = bits as *mut u32;
        for i in 0..(surf_h * dst_stride) {
            *pixels.add(i) = 0xFF000000u32;
        }

        let (loop_w, loop_h, off_x, off_y, pad_x, pad_y, render_w, render_h) =
            compute_scale_params(surf_w, surf_h, width, height, is_screencast);

        write_i420_scaled_pixels(
            bits, dst_stride, y_data, u_data, v_data, y_stride, u_stride, v_stride, width, height,
            surf_h, loop_w, loop_h, off_x, off_y, pad_x, pad_y, render_w, render_h,
        );

        ndk_sys::ANativeWindow_unlockAndPost(window);
    }
    true
}

/// Paint an ANativeWindow surface solid black.
/// Called before the frame loop starts to avoid showing the uninitialized
/// green TextureView buffer while waiting for the first WebRTC frame.
pub fn paint_surface_black(surface: *mut c_void) {
    if surface.is_null() {
        return;
    }
    let window = surface as *mut ndk_sys::ANativeWindow;
    unsafe {
        let surf_w = ndk_sys::ANativeWindow_getWidth(window) as usize;
        let surf_h = ndk_sys::ANativeWindow_getHeight(window) as usize;
        if surf_w == 0 || surf_h == 0 {
            return;
        }
        if ndk_sys::ANativeWindow_setBuffersGeometry(window, surf_w as i32, surf_h as i32, 1) != 0 {
            return;
        }
        let mut native_buf = std::mem::MaybeUninit::<ndk_sys::ANativeWindow_Buffer>::uninit();
        if ndk_sys::ANativeWindow_lock(window, native_buf.as_mut_ptr(), std::ptr::null_mut()) != 0 {
            return;
        }
        let native_buf = native_buf.assume_init();
        let pixels = native_buf.bits as *mut u32;
        let stride = native_buf.stride as usize;
        for i in 0..(surf_h * stride) {
            *pixels.add(i) = 0xFF000000u32; // opaque black
        }
        ndk_sys::ANativeWindow_unlockAndPost(window);
    }
}
