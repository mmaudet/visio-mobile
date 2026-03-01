//! Desktop (macOS / Linux / Windows) frame renderer.
//!
//! Converts I420 frames to JPEG -> base64 data URIs and delivers
//! them via a callback to the Tauri frontend.

use std::ffi::c_void;

use livekit::webrtc::prelude::BoxVideoFrame;

/// Render a single video frame to the desktop surface.
///
/// `surface` is an opaque pointer to a callback context.
/// `track_sid` identifies which track this frame belongs to.
///
/// Stub implementation -- will be filled in Task 10.
pub(crate) fn render_frame(
    _frame: &BoxVideoFrame,
    _surface: *mut c_void,
    _track_sid: &str,
) {
    // TODO(task-10): Convert I420 -> JPEG -> base64, invoke callback.
}
