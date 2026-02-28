//! Video frame pipeline with raw C FFI.
//!
//! Delivers I420 frames from LiveKit NativeVideoStream
//! directly to platform-native rendering surfaces.
//! This crate bypasses UniFFI for zero-copy performance.

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "ios")]
mod ios;

#[cfg(target_os = "macos")]
mod desktop;

/// Attach a native rendering surface to a video track.
///
/// # Safety
/// `surface` must be a valid platform surface handle:
/// - Android: ANativeWindow* obtained from SurfaceTexture
/// - iOS: pointer to AVSampleBufferDisplayLayer
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_attach_surface(
    _track_sid: *const std::ffi::c_char,
    _surface: *mut std::ffi::c_void,
) -> i32 {
    0
}

/// Detach the rendering surface from a video track.
///
/// # Safety
/// `track_sid` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_detach_surface(
    _track_sid: *const std::ffi::c_char,
) -> i32 {
    0
}
