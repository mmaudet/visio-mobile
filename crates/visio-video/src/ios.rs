//! iOS video renderer — passes I420 planes to Swift callback.
//!
//! Swift side creates CVPixelBuffer from the planes and displays
//! on an AVSampleBufferDisplayLayer for GPU-accelerated rendering.

use std::ffi::c_void;
use std::sync::OnceLock;

use livekit::webrtc::prelude::BoxVideoFrame;
use livekit::webrtc::video_frame::I420Buffer;

/// Callback: (width, height, y_ptr, y_stride, u_ptr, u_stride, v_ptr, v_stride, track_sid, user_data)
type IosFrameCallback = unsafe extern "C" fn(
    width: u32,
    height: u32,
    y_ptr: *const u8,
    y_stride: u32,
    u_ptr: *const u8,
    u_stride: u32,
    v_ptr: *const u8,
    v_stride: u32,
    track_sid: *const std::ffi::c_char,
    user_data: *mut c_void,
);

struct IosCallbackInfo {
    callback: IosFrameCallback,
    user_data: *mut c_void,
}

// SAFETY: The callback and user_data pointer are set once at app startup from
// the main thread and remain valid for the application's entire lifetime. The
// callback is invoked from the visio-video tokio worker threads, but the Swift
// side synchronises access internally.
unsafe impl Send for IosCallbackInfo {}
unsafe impl Sync for IosCallbackInfo {}

static IOS_CALLBACK: OnceLock<IosCallbackInfo> = OnceLock::new();

/// Register a frame callback from Swift.
///
/// # Safety
/// - `callback` must point to a valid function with the `IosFrameCallback` signature.
/// - `user_data` must remain valid for the application's lifetime.
/// - This function should be called exactly once, before any frames arrive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_set_ios_callback(
    callback: IosFrameCallback,
    user_data: *mut c_void,
) {
    let _ = IOS_CALLBACK.set(IosCallbackInfo {
        callback,
        user_data,
    });
}

/// Send I420 planes to the iOS callback for a given track SID.
pub fn deliver_i420_to_ios_callback(
    width: u32,
    height: u32,
    y_ptr: *const u8,
    y_stride: u32,
    u_ptr: *const u8,
    u_stride: u32,
    v_ptr: *const u8,
    v_stride: u32,
    track_sid: &str,
) {
    let Some(cb) = IOS_CALLBACK.get() else {
        return;
    };

    let sid_cstr = match std::ffi::CString::new(track_sid) {
        Ok(s) => s,
        Err(_) => return,
    };

    unsafe {
        (cb.callback)(
            width,
            height,
            y_ptr,
            y_stride,
            u_ptr,
            u_stride,
            v_ptr,
            v_stride,
            sid_cstr.as_ptr(),
            cb.user_data,
        );
    }
}

/// Render a single I420 frame by passing plane pointers to the iOS callback.
///
/// The Swift callback receives raw Y/U/V plane pointers and strides so it can
/// create a CVPixelBuffer (or copy into one from a pool) and enqueue it on an
/// AVSampleBufferDisplayLayer for GPU-accelerated YUV-to-RGB conversion.
pub(crate) fn render_frame(frame: &BoxVideoFrame, _surface: *mut c_void, track_sid: &str, _is_screencast: bool) {
    let Some(cb) = IOS_CALLBACK.get() else {
        return;
    };

    let buffer = &frame.buffer;
    let width = buffer.width();
    let height = buffer.height();

    // Convert the (possibly native) buffer to I420 planar format.
    let i420 = buffer.to_i420();

    // Get Y, U, V plane data as byte slices.
    let (y_data, u_data, v_data) = i420.data();

    // Get per-plane strides (bytes per row).
    let (stride_y, stride_u, stride_v) = i420.strides();

    // Convert the track SID to a C string for the callback.
    let sid_cstr = match std::ffi::CString::new(track_sid) {
        Ok(s) => s,
        Err(_) => return, // track_sid contained a null byte — skip frame
    };

    unsafe {
        (cb.callback)(
            width,
            height,
            y_data.as_ptr(),
            stride_y,
            u_data.as_ptr(),
            stride_u,
            v_data.as_ptr(),
            stride_v,
            sid_cstr.as_ptr(),
            cb.user_data,
        );
    }
}

/// Render a preview frame without feeding it to a LiveKit source.
///
/// Called from Swift CameraCapture in preview mode (pre-join lobby).
/// Accepts pre-separated I420 planes (Y/U/V) and delivers them to the
/// iOS frame callback with the special track SID `"local-preview"`.
///
/// Blur processing is not applied here — it is applied upstream by
/// `visio_push_ios_camera_frame` in visio-ffi. This function is used
/// when no LiveKit connection exists yet (preview-only mode).
///
/// # Safety
/// - All plane pointers must be valid for the specified dimensions and strides.
/// - The function must not be called concurrently with `visio_video_set_ios_callback`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_process_preview_frame(
    y_ptr: *const u8,
    y_stride: u32,
    u_ptr: *const u8,
    u_stride: u32,
    v_ptr: *const u8,
    v_stride: u32,
    width: u32,
    height: u32,
    _rotation: u32,
) {
    let Some(cb) = IOS_CALLBACK.get() else {
        return;
    };

    let w = width as usize;
    let h = height as usize;
    let chroma_w = w / 2;
    let chroma_h = h / 2;

    // Build an owned I420Buffer so the planes are laid out contiguously.
    let mut i420 = I420Buffer::new(width, height);
    let (strides_y, strides_u, strides_v) = i420.strides();
    let (y_dst, u_dst, v_dst) = i420.data_mut();

    // Copy Y plane row-by-row.
    for row in 0..h {
        let src = unsafe {
            std::slice::from_raw_parts(y_ptr.add(row * y_stride as usize), w)
        };
        let dst_start = row * strides_y as usize;
        y_dst[dst_start..dst_start + w].copy_from_slice(src);
    }

    // Copy U plane row-by-row.
    for row in 0..chroma_h {
        let src = unsafe {
            std::slice::from_raw_parts(u_ptr.add(row * u_stride as usize), chroma_w)
        };
        let dst_start = row * strides_u as usize;
        u_dst[dst_start..dst_start + chroma_w].copy_from_slice(src);
    }

    // Copy V plane row-by-row.
    for row in 0..chroma_h {
        let src = unsafe {
            std::slice::from_raw_parts(v_ptr.add(row * v_stride as usize), chroma_w)
        };
        let dst_start = row * strides_v as usize;
        v_dst[dst_start..dst_start + chroma_w].copy_from_slice(src);
    }

    // Deliver to the iOS frame callback with the "local-preview" track SID.
    let (y_data, u_data, v_data) = i420.data();
    let (out_sy, out_su, out_sv) = i420.strides();

    let sid_cstr = match std::ffi::CString::new("local-preview") {
        Ok(s) => s,
        Err(_) => return,
    };

    unsafe {
        (cb.callback)(
            width,
            height,
            y_data.as_ptr(),
            out_sy,
            u_data.as_ptr(),
            out_su,
            v_data.as_ptr(),
            out_sv,
            sid_cstr.as_ptr(),
            cb.user_data,
        );
    }
}
