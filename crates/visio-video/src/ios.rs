//! iOS video renderer — passes I420 planes to Swift callback.
//!
//! Swift side creates CVPixelBuffer from the planes and displays
//! on an AVSampleBufferDisplayLayer for GPU-accelerated rendering.

use std::ffi::c_void;
use std::sync::OnceLock;

use livekit::webrtc::prelude::BoxVideoFrame;

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

/// Render a single I420 frame by passing plane pointers to the iOS callback.
///
/// The Swift callback receives raw Y/U/V plane pointers and strides so it can
/// create a CVPixelBuffer (or copy into one from a pool) and enqueue it on an
/// AVSampleBufferDisplayLayer for GPU-accelerated YUV-to-RGB conversion.
pub(crate) fn render_frame(
    frame: &BoxVideoFrame,
    _surface: *mut c_void,
    track_sid: &str,
) {
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
