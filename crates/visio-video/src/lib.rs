//! Video frame pipeline with raw C FFI.
//!
//! Delivers I420 frames from LiveKit NativeVideoStream
//! directly to platform-native rendering surfaces.
//! This crate bypasses UniFFI for zero-copy performance.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::{Mutex, OnceLock};

use futures_util::StreamExt;
use livekit::prelude::*;
use livekit::webrtc::video_stream::native::NativeVideoStream;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::watch;
use tokio::task::JoinHandle;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
pub use android::render_i420_to_surface;

#[cfg(target_os = "android")]
fn android_log(msg: &str) {
    use std::ffi::CString;
    let tag = CString::new("VISIO_VIDEO").unwrap();
    let text = CString::new(msg).unwrap_or_else(|_| CString::new("(invalid)").unwrap());
    unsafe {
        unsafe extern "C" { fn __android_log_write(prio: i32, tag: *const std::ffi::c_char, text: *const std::ffi::c_char) -> i32; }
        __android_log_write(4, tag.as_ptr(), text.as_ptr());
    }
}

#[cfg(target_os = "ios")]
mod ios;

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
mod desktop;

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use desktop::visio_video_set_desktop_callback;

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use desktop::render_local_i420;

// ---------------------------------------------------------------------------
// Send-able surface pointer wrapper
// ---------------------------------------------------------------------------

/// Wrapper around `*mut c_void` that implements `Send`.
///
/// # Safety
/// The caller guarantees that the surface pointer remains valid for the
/// lifetime of the frame loop and is not concurrently accessed from other
/// threads. Each surface is owned by exactly one `TrackRenderer`.
struct SurfacePtr(*mut c_void);

// SAFETY: Surface pointers are passed from platform code and used exclusively
// by a single frame_loop task. The platform guarantees the pointer remains
// valid until `stop_track_renderer` / `visio_video_detach_surface` is called.
unsafe impl Send for SurfacePtr {}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------

/// Per-track renderer handle. Dropping cancels the background task.
struct TrackRenderer {
    cancel_tx: watch::Sender<bool>,
    _handle: JoinHandle<()>,
}

/// Registry of active track renderers, keyed by track SID.
static RENDERERS: OnceLock<Mutex<HashMap<String, TrackRenderer>>> = OnceLock::new();

/// Dedicated tokio runtime for video frame loops (2 worker threads).
static RT: OnceLock<Runtime> = OnceLock::new();

fn renderers() -> &'static Mutex<HashMap<String, TrackRenderer>> {
    RENDERERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn runtime() -> &'static Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("visio-video")
            .enable_all()
            .build()
            .expect("failed to create visio-video runtime")
    })
}

// ---------------------------------------------------------------------------
// Public API (called from visio-core / visio-ffi via Rust)
// ---------------------------------------------------------------------------

/// Start rendering frames from `track` onto `surface`.
///
/// `surface` is an opaque platform handle (ANativeWindow*, CVPixelBufferPool*,
/// or a callback pointer on desktop). Ownership is NOT transferred — the
/// caller must keep the surface alive until `stop_track_renderer` is called.
///
/// If `rt_handle` is provided, the frame loop is spawned on that runtime.
/// Otherwise it falls back to visio-video's internal runtime. Callers should
/// pass the application runtime handle to avoid cross-runtime issues (e.g.
/// on Android where NativeVideoStream may not yield frames on a separate runtime).
pub fn start_track_renderer(
    track_sid: String,
    track: RemoteVideoTrack,
    surface: *mut c_void,
    rt_handle: Option<Handle>,
) {
    // If there is already a renderer for this track, stop it first.
    stop_track_renderer(&track_sid);

    let (cancel_tx, cancel_rx) = watch::channel(false);
    let sid = track_sid.clone();

    let handle = match rt_handle {
        Some(h) => h.spawn(frame_loop(sid, track, SurfacePtr(surface), cancel_rx)),
        None => runtime().spawn(frame_loop(sid, track, SurfacePtr(surface), cancel_rx)),
    };

    let renderer = TrackRenderer {
        cancel_tx,
        _handle: handle,
    };

    renderers()
        .lock()
        .expect("renderer lock poisoned")
        .insert(track_sid, renderer);
}

/// Stop and remove the renderer for `track_sid`.
pub fn stop_track_renderer(track_sid: &str) {
    if let Some(renderer) = renderers()
        .lock()
        .expect("renderer lock poisoned")
        .remove(track_sid)
    {
        // Signal cancellation; the frame_loop will exit on next iteration.
        let _ = renderer.cancel_tx.send(true);
        // JoinHandle is dropped here — the task will be cancelled eventually.
    }
}

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------

async fn frame_loop(
    track_sid: String,
    track: RemoteVideoTrack,
    surface: SurfacePtr,
    mut cancel_rx: watch::Receiver<bool>,
) {
    #[cfg(target_os = "android")]
    android_log(&format!("VISIO VIDEO: frame_loop started for track={track_sid}, enabled={}, muted={}",
        track.is_enabled(), track.is_muted()));
    tracing::info!(track_sid = %track_sid, "frame_loop started");

    let rtc_track = track.rtc_track();
    #[cfg(target_os = "android")]
    android_log(&format!("VISIO VIDEO: creating NativeVideoStream for track={track_sid}"));
    let mut stream = NativeVideoStream::new(rtc_track);
    #[cfg(target_os = "android")]
    android_log(&format!("VISIO VIDEO: NativeVideoStream created, waiting for frames track={track_sid}"));

    #[cfg(target_os = "android")]
    let mut android_frame_count: u64 = 0;
    #[cfg(target_os = "android")]
    let mut android_poll_count: u64 = 0;

    // Desktop: only render every Nth frame to save CPU.
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    let mut frame_count: u64 = 0;

    loop {
        tokio::select! {
            _ = cancel_rx.changed() => {
                #[cfg(target_os = "android")]
                android_log(&format!("VISIO VIDEO: frame_loop cancelled track={track_sid}"));
                tracing::info!(track_sid = %track_sid, "frame_loop cancelled");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
                #[cfg(target_os = "android")]
                {
                    android_poll_count += 1;
                    android_log(&format!("VISIO VIDEO: still waiting for frames track={track_sid} (poll #{android_poll_count}, got {android_frame_count} frames so far)"));
                }
            }
            frame_opt = stream.next() => {
                match frame_opt {
                    Some(frame) => {
                        // --- Android ---
                        #[cfg(target_os = "android")]
                        {
                            android_frame_count += 1;
                            if android_frame_count == 1 || android_frame_count % 100 == 0 {
                                android_log(&format!("VISIO VIDEO: frame #{android_frame_count} track={track_sid} {}x{}", frame.buffer.width(), frame.buffer.height()));
                            }
                            android::render_frame(&frame, surface.0, &track_sid);
                        }

                        // --- iOS ---
                        #[cfg(target_os = "ios")]
                        {
                            ios::render_frame(&frame, surface.0, &track_sid);
                        }

                        // --- Desktop (macOS / Linux / Windows) ---
                        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
                        {
                            frame_count += 1;
                            if frame_count == 1 {
                                tracing::info!(track_sid = %track_sid, width = frame.buffer.width(), height = frame.buffer.height(), "first video frame received");
                            }
                            // Throttle: render every 3rd frame (~10 fps at 30 fps input).
                            if frame_count % 3 == 0 {
                                desktop::render_frame(&frame, surface.0, &track_sid);
                            }
                        }
                    }
                    None => {
                        #[cfg(target_os = "android")]
                        android_log(&format!("VISIO VIDEO: stream ended (None) track={track_sid}, total frames={android_frame_count}"));
                        tracing::info!(track_sid = %track_sid, "video stream ended");
                        break;
                    }
                }
            }
        }
    }

    tracing::info!(track_sid = %track_sid, "frame_loop exited");
}

// ---------------------------------------------------------------------------
// C FFI entry points
// ---------------------------------------------------------------------------

/// Attach a native rendering surface to a video track.
///
/// This is called from platform code (Kotlin/JNI, Swift, or Tauri) to start
/// rendering frames from the given track onto the given surface.
///
/// # Safety
/// - `track_sid` must be a valid null-terminated C string.
/// - `surface` must be a valid platform surface handle:
///   - Android: ANativeWindow* obtained from SurfaceTexture
///   - iOS: pointer to a rendering layer/callback
///   - Desktop: pointer to a frame callback
///
/// Returns 0 on success, -1 on invalid arguments.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_attach_surface(
    track_sid: *const c_char,
    surface: *mut c_void,
) -> i32 {
    if track_sid.is_null() || surface.is_null() {
        return -1;
    }

    let sid = match unsafe { CStr::from_ptr(track_sid) }.to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return -1,
    };

    tracing::info!(track_sid = %sid, "visio_video_attach_surface called (track not yet wired)");

    // NOTE: The actual track attachment happens when visio-core calls
    // start_track_renderer() with the real RemoteVideoTrack. This C FFI
    // entry point is a placeholder for platform code that needs to
    // register a surface before the track is available.
    // For now, return success — the surface will be connected when the
    // track arrives via the Rust API.
    0
}

/// Detach the rendering surface from a video track.
///
/// # Safety
/// `track_sid` must be a valid null-terminated C string.
///
/// Returns 0 on success, -1 on invalid arguments.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_detach_surface(
    track_sid: *const c_char,
) -> i32 {
    if track_sid.is_null() {
        return -1;
    }

    let sid = match unsafe { CStr::from_ptr(track_sid) }.to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return -1,
    };

    tracing::info!(track_sid = %sid, "visio_video_detach_surface");
    stop_track_renderer(&sid);
    0
}
