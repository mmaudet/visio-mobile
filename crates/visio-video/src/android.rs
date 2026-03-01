//! Android frame renderer.
//!
//! Writes I420 frames to an ANativeWindow surface obtained from
//! a SurfaceTexture on the Kotlin side.

use std::ffi::c_void;

use livekit::webrtc::prelude::BoxVideoFrame;

/// Render a single video frame to the Android native window.
///
/// `surface` is an ANativeWindow* obtained via JNI.
/// `track_sid` identifies which track this frame belongs to.
///
/// Stub implementation -- will be filled in Task 13.
pub(crate) fn render_frame(
    _frame: &BoxVideoFrame,
    _surface: *mut c_void,
    _track_sid: &str,
) {
    // TODO(task-13): Write I420 directly to ANativeWindow buffer.
}
