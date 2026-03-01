//! iOS frame renderer.
//!
//! Wraps I420 frames in CVPixelBuffers and enqueues them on an
//! AVSampleBufferDisplayLayer for GPU-accelerated rendering.

use std::ffi::c_void;

use livekit::webrtc::prelude::BoxVideoFrame;

/// Render a single video frame to the iOS display layer.
///
/// `surface` is a pointer to the rendering context.
/// `track_sid` identifies which track this frame belongs to.
///
/// Stub implementation -- will be filled in Task 14.
pub(crate) fn render_frame(
    _frame: &BoxVideoFrame,
    _surface: *mut c_void,
    _track_sid: &str,
) {
    // TODO(task-14): Wrap I420 in CVPixelBuffer, enqueue on display layer.
}
