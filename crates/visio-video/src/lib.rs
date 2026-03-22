//! Video frame pipeline with raw C FFI.
//!
//! Delivers I420 frames from LiveKit NativeVideoStream
//! directly to platform-native rendering surfaces.
//! This crate bypasses UniFFI for zero-copy performance.

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_void};
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
pub use android::paint_surface_black;

#[cfg(target_os = "android")]
fn android_log(msg: &str) {
    use std::ffi::CString;
    let text = CString::new(msg).unwrap_or_else(|_| c"(invalid)".into());
    unsafe {
        unsafe extern "C" {
            fn __android_log_write(
                prio: i32,
                tag: *const std::ffi::c_char,
                text: *const std::ffi::c_char,
            ) -> i32;
        }
        __android_log_write(4, c"VISIO_VIDEO".as_ptr(), text.as_ptr());
    }
}

#[cfg(target_os = "ios")]
mod ios;

#[cfg(target_os = "ios")]
pub use ios::deliver_i420_to_ios_callback;

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
    is_screencast: bool,
) {
    tracing::info!(track_sid = %track_sid, is_screencast, "start_track_renderer called");
    #[cfg(target_os = "android")]
    android_log(&format!("VISIO VIDEO: start_track_renderer track={track_sid} screencast={is_screencast}"));

    // If there is already a renderer for this track, stop it first.
    stop_track_renderer(&track_sid);

    let (cancel_tx, cancel_rx) = watch::channel(false);
    let sid = track_sid.clone();

    let handle = match rt_handle {
        Some(h) => h.spawn(frame_loop(sid, track, SurfacePtr(surface), cancel_rx, is_screencast)),
        None => runtime().spawn(frame_loop(sid, track, SurfacePtr(surface), cancel_rx, is_screencast)),
    };

    let renderer = TrackRenderer {
        cancel_tx,
        _handle: handle,
    };

    renderers()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(track_sid, renderer);
}

/// Stop and remove the renderer for `track_sid`.
pub fn stop_track_renderer(track_sid: &str) {
    tracing::info!(track_sid = %track_sid, "stop_track_renderer called");
    #[cfg(target_os = "android")]
    android_log(&format!("VISIO VIDEO: stop_track_renderer track={track_sid}"));

    if let Some(renderer) = renderers()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
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
    is_screencast: bool,
) {
    #[cfg(target_os = "android")]
    android_log(&format!(
        "VISIO VIDEO: frame_loop started for track={track_sid}, enabled={}, muted={}",
        track.is_enabled(),
        track.is_muted()
    ));
    tracing::info!(track_sid = %track_sid, "frame_loop started");

    let rtc_track = track.rtc_track();
    #[cfg(target_os = "android")]
    android_log(&format!(
        "VISIO VIDEO: creating NativeVideoStream for track={track_sid}"
    ));
    let mut stream = NativeVideoStream::new(rtc_track.clone());
    #[cfg(target_os = "android")]
    android_log(&format!(
        "VISIO VIDEO: NativeVideoStream created, waiting for frames track={track_sid}"
    ));

    #[cfg(target_os = "android")]
    let mut android_frame_count: u64 = 0;
    #[cfg(target_os = "android")]
    let mut android_poll_count: u64 = 0;

    // Desktop: only render every Nth frame to save CPU.
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    let mut frame_count: u64 = 0;

    /// Maximum number of consecutive stream-ended retries before giving up.
    /// This handles the case where a muted track's NativeVideoStream yields
    /// None immediately — we re-create the stream and retry after a short
    /// delay, giving the track time to resume producing frames after unmute.
    const MAX_STREAM_RETRIES: u32 = 10;
    let mut stream_retries: u32 = 0;

    /// Maximum number of outer retries after all stream retries are exhausted.
    /// Each outer retry waits longer (2s) before recreating the stream, handling
    /// the case where a muted track unmutes after the inner retry window.
    const MAX_OUTER_RETRIES: u32 = 3;
    let mut outer_retries: u32 = 0;

    'outer: loop {
    loop {
        tokio::select! {
            _ = cancel_rx.changed() => {
                #[cfg(target_os = "android")]
                android_log(&format!("VISIO VIDEO: frame_loop cancelled track={track_sid}"));
                tracing::info!(track_sid = %track_sid, "frame_loop cancelled");
                break 'outer;
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
                        // Reset retry counters on successful frame reception.
                        stream_retries = 0;
                        outer_retries = 0;

                        // --- Android ---
                        #[cfg(target_os = "android")]
                        {
                            android_frame_count += 1;
                            if android_frame_count == 1 || android_frame_count % 100 == 0 {
                                android_log(&format!("VISIO VIDEO: frame #{android_frame_count} track={track_sid} {}x{}", frame.buffer.width(), frame.buffer.height()));
                            }
                            if !android::render_frame(&frame, surface.0, &track_sid, is_screencast) {
                                android_log(&format!("VISIO VIDEO: surface invalid, waiting for new surface track={track_sid}"));
                                tracing::warn!(track_sid = %track_sid, "render_frame failed — surface destroyed, waiting for re-attach");

                                const MAX_SURFACE_RETRIES: u32 = 30;
                                let mut surface_recovered = false;
                                for attempt in 1..=MAX_SURFACE_RETRIES {
                                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                                    // Check for cancellation while waiting.
                                    if *cancel_rx.borrow() {
                                        android_log(&format!("VISIO VIDEO: cancelled during surface wait track={track_sid}"));
                                        break 'outer;
                                    }

                                    // Try rendering again — if the platform attached a new surface
                                    // it will have called start_track_renderer which replaces us,
                                    // but if it reused the same pointer we can resume.
                                    if android::render_frame(&frame, surface.0, &track_sid, is_screencast) {
                                        android_log(&format!("VISIO VIDEO: surface recovered after {attempt} attempts track={track_sid}"));
                                        tracing::info!(track_sid = %track_sid, attempt, "surface recovered");
                                        surface_recovered = true;
                                        break;
                                    }

                                    if attempt % 10 == 0 {
                                        android_log(&format!("VISIO VIDEO: still waiting for surface track={track_sid} attempt={attempt}/{MAX_SURFACE_RETRIES}"));
                                    }
                                }

                                if !surface_recovered {
                                    android_log(&format!("VISIO VIDEO: surface not recovered after {MAX_SURFACE_RETRIES} attempts, stopping frame_loop track={track_sid}"));
                                    tracing::warn!(track_sid = %track_sid, "surface not recovered after 3s, exiting frame_loop");
                                    break 'outer;
                                }
                            }
                        }

                        // --- iOS ---
                        #[cfg(target_os = "ios")]
                        {
                            ios::render_frame(&frame, surface.0, &track_sid, is_screencast);
                        }

                        // --- Desktop (macOS / Linux / Windows) ---
                        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
                        {
                            frame_count += 1;
                            if frame_count == 1 {
                                tracing::info!(track_sid = %track_sid, width = frame.buffer.width(), height = frame.buffer.height(), "first video frame received");
                            }
                            // Throttle: render every 3rd frame (~10fps at 30fps input)
                            // to avoid memory explosion from JPEG+base64 encoding.
                            if frame_count % 3 == 0 {
                                desktop::render_frame(&frame, surface.0, &track_sid, is_screencast);
                            }
                        }
                    }
                    None => {
                        stream_retries += 1;
                        #[cfg(target_os = "android")]
                        android_log(&format!(
                            "VISIO VIDEO: stream yielded None track={track_sid}, retry {stream_retries}/{MAX_STREAM_RETRIES}"
                        ));
                        tracing::info!(
                            track_sid = %track_sid,
                            retry = stream_retries,
                            max_retries = MAX_STREAM_RETRIES,
                            "video stream yielded None, will retry with new NativeVideoStream"
                        );

                        if stream_retries > MAX_STREAM_RETRIES {
                            #[cfg(target_os = "android")]
                            android_log(&format!(
                                "VISIO VIDEO: stream retries exhausted track={track_sid}, outer retry {outer_retries}/{MAX_OUTER_RETRIES}"
                            ));
                            tracing::info!(
                                track_sid = %track_sid,
                                outer_retry = outer_retries,
                                max_outer = MAX_OUTER_RETRIES,
                                "stream retries exhausted, attempting outer retry"
                            );

                            outer_retries += 1;
                            if outer_retries > MAX_OUTER_RETRIES {
                                #[cfg(target_os = "android")]
                                android_log(&format!(
                                    "VISIO VIDEO: stream ended permanently track={track_sid} after {MAX_OUTER_RETRIES} outer retries"
                                ));
                                tracing::info!(track_sid = %track_sid, "video stream ended permanently after all retries exhausted");
                                break 'outer;
                            }

                            // Wait longer before outer retry — gives more time for
                            // unmute or track re-publication.
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                            // Check for cancellation after the wait.
                            if *cancel_rx.borrow() {
                                break 'outer;
                            }

                            // Reset inner retry counter and recreate stream.
                            stream_retries = 0;
                            stream = NativeVideoStream::new(rtc_track.clone());
                            #[cfg(target_os = "android")]
                            android_log(&format!(
                                "VISIO VIDEO: outer retry — re-created NativeVideoStream track={track_sid}"
                            ));
                            tracing::info!(track_sid = %track_sid, "outer retry — re-created NativeVideoStream");
                            break; // break inner loop to restart
                        }

                        // Wait before re-creating the stream — gives the track
                        // time to resume producing frames after an unmute.
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                        // Re-create NativeVideoStream from the same rtc_track.
                        // When a remote participant toggles their camera off and
                        // back on, the old stream is exhausted but the track
                        // object is still valid and will produce new frames once
                        // unmuted.
                        stream = NativeVideoStream::new(rtc_track.clone());
                        #[cfg(target_os = "android")]
                        android_log(&format!(
                            "VISIO VIDEO: re-created NativeVideoStream track={track_sid}"
                        ));
                        tracing::info!(track_sid = %track_sid, "re-created NativeVideoStream after None");
                    }
                }
            }
        }
    }
    } // 'outer

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

    // Paint the surface black immediately to replace the uninitialized green
    // TextureView buffer. This runs on the UI thread via JNI, before any
    // WebRTC frames are available.
    #[cfg(target_os = "android")]
    android::paint_surface_black(surface);

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
pub unsafe extern "C" fn visio_video_detach_surface(track_sid: *const c_char) -> i32 {
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
