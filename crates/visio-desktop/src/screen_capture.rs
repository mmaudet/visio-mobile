//! Cross-platform screen capture using the `xcap` crate.
//!
//! Lists available monitors and windows, captures frames at ~15 fps,
//! converts RGBA->I420, and feeds into a LiveKit NativeVideoSource.

use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine as _;
use image::DynamicImage;
use image::imageops::FilterType;
use livekit::webrtc::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use serde::Serialize;
use tokio::task::JoinHandle;

/// A capturable screen source (monitor or window).
#[derive(Debug, Clone, Serialize)]
pub struct ScreenSource {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub width: u32,
    pub height: u32,
    pub thumbnail: String, // "data:image/jpeg;base64,..." or "" on failure
}

const THUMBNAIL_WIDTH: u32 = 240;
const THUMBNAIL_QUALITY: u8 = 60;

/// Capture a screenshot, resize to thumbnail, encode as JPEG base64 data URI.
fn capture_thumbnail(img: xcap::XCapResult<image::RgbaImage>) -> String {
    let Ok(rgba) = img else {
        return String::new();
    };
    let dyn_img = DynamicImage::ImageRgba8(rgba);

    // Resize proportionally to THUMBNAIL_WIDTH
    let aspect = dyn_img.height() as f32 / dyn_img.width() as f32;
    let thumb_height = (THUMBNAIL_WIDTH as f32 * aspect).max(1.0) as u32;
    let thumb = dyn_img.resize(THUMBNAIL_WIDTH, thumb_height, FilterType::Triangle);

    // Encode as JPEG
    let rgb = thumb.to_rgb8();
    let mut buf = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, THUMBNAIL_QUALITY);
    if rgb.write_with_encoder(encoder).is_err() {
        return String::new();
    }

    // Base64 encode
    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    format!("data:image/jpeg;base64,{b64}")
}

/// List all available screen sources (monitors + windows).
pub fn list_sources() -> Vec<ScreenSource> {
    let mut sources = Vec::new();

    match xcap::Monitor::all() {
        Ok(monitors) => {
            tracing::info!("xcap found {} monitors", monitors.len());
            for (i, monitor) in monitors.iter().enumerate() {
                let width = monitor.width().unwrap_or(0);
                let height = monitor.height().unwrap_or(0);
                let is_primary = monitor.is_primary().unwrap_or(false);
                let name = monitor.name().unwrap_or_default();
                tracing::info!(
                    "  monitor {i}: name={name:?}, {width}x{height}, primary={is_primary}"
                );
                let label = if is_primary {
                    format!("Screen {} (primary)", i + 1)
                } else {
                    format!("Screen {}", i + 1)
                };
                let thumbnail = capture_thumbnail(monitor.capture_image());
                sources.push(ScreenSource {
                    id: format!("monitor-{i}"),
                    name: label,
                    source_type: "monitor".into(),
                    width,
                    height,
                    thumbnail,
                });
            }
        }
        Err(e) => tracing::error!("xcap::Monitor::all() failed: {e}"),
    }

    match xcap::Window::all() {
        Ok(windows) => {
            tracing::info!("xcap found {} windows total", windows.len());
            for window in &windows {
                let app_name = window.app_name().unwrap_or_default();
                let title = window.title().unwrap_or_default();
                let width = window.width().unwrap_or(0);
                let height = window.height().unwrap_or(0);
                let minimized = window.is_minimized().unwrap_or(false);
                let id_result = window.id();

                // Log every window for debugging
                tracing::debug!(
                    "  window: app={app_name:?}, title={title:?}, {width}x{height}, minimized={minimized}, id={id_result:?}"
                );

                if minimized {
                    tracing::debug!("    -> skipped (minimized)");
                    continue;
                }
                // Skip tiny windows (menu bar items, status icons, etc.)
                if width < 100 || height < 100 {
                    tracing::debug!("    -> skipped (too small: {width}x{height})");
                    continue;
                }
                let id = match id_result {
                    Ok(id) => id,
                    Err(ref e) => {
                        tracing::debug!("    -> skipped (id error: {e})");
                        continue;
                    }
                };
                // Build a user-friendly label: "App — Title" or just "App"
                let label = if title.is_empty() || title == app_name {
                    if app_name.is_empty() {
                        tracing::debug!("    -> skipped (no name/title)");
                        continue;
                    }
                    app_name
                } else if app_name.is_empty() {
                    title
                } else {
                    format!("{app_name} — {title}")
                };
                tracing::info!("  window accepted: id={id}, label={label:?}");
                let thumbnail = capture_thumbnail(window.capture_image());
                sources.push(ScreenSource {
                    id: format!("window-{id}"),
                    name: label,
                    source_type: "window".into(),
                    width,
                    height,
                    thumbnail,
                });
            }
        }
        Err(e) => tracing::error!("xcap::Window::all() failed: {e}"),
    }

    tracing::info!(
        "list_sources: returning {} sources ({} monitors + {} windows)",
        sources.len(),
        sources
            .iter()
            .filter(|s| s.source_type == "monitor")
            .count(),
        sources.iter().filter(|s| s.source_type == "window").count(),
    );
    sources
}

/// Manages a screen capture session.
pub struct ScreenCapture {
    stop_flag: Arc<AtomicBool>,
    task: Option<JoinHandle<()>>,
}

impl ScreenCapture {
    /// Start capturing from the given source ID.
    pub fn start(source_id: &str, video_source: NativeVideoSource) -> Result<Self, String> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag = stop_flag.clone();
        let sid = source_id.to_string();

        let task = tokio::spawn(async move {
            capture_loop(&sid, video_source, flag).await;
        });

        tracing::info!("screen capture started for source {source_id}");
        Ok(Self {
            stop_flag,
            task: Some(task),
        })
    }

    /// Stop the capture loop.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(task) = self.task.take() {
            task.abort();
        }
        tracing::info!("screen capture stopped");
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Convert an RGBA image to an I420 buffer.
/// `rgba_stride` is the number of pixels per row in the source RGBA buffer
/// (may differ from `width` when dimensions are masked to even).
fn rgba_to_i420(rgba: &[u8], width: u32, height: u32, rgba_stride: usize) -> I420Buffer {
    let w = width as usize;
    let h = height as usize;
    let mut buf = I420Buffer::new(width, height);

    let strides = buf.strides();
    let (y_dst, u_dst, v_dst) = buf.data_mut();

    for row in 0..h {
        for col in 0..w {
            let px = (row * rgba_stride + col) * 4;
            let r = rgba[px] as f32;
            let g = rgba[px + 1] as f32;
            let b = rgba[px + 2] as f32;
            let y = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
            y_dst[row * strides.0 as usize + col] = y;
        }
    }

    let chroma_h = h / 2;
    let chroma_w = w / 2;
    for row in 0..chroma_h {
        for col in 0..chroma_w {
            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let px = ((row * 2 + dy) * rgba_stride + (col * 2 + dx)) * 4;
                    r_sum += rgba[px] as u32;
                    g_sum += rgba[px + 1] as u32;
                    b_sum += rgba[px + 2] as u32;
                }
            }
            let r = (r_sum / 4) as f32;
            let g = (g_sum / 4) as f32;
            let b = (b_sum / 4) as f32;

            let u = (-0.169 * r - 0.331 * g + 0.500 * b + 128.0).clamp(0.0, 255.0) as u8;
            let v = (0.500 * r - 0.419 * g - 0.081 * b + 128.0).clamp(0.0, 255.0) as u8;

            u_dst[row * strides.1 as usize + col] = u;
            v_dst[row * strides.2 as usize + col] = v;
        }
    }

    buf
}

/// Capture + convert on a blocking thread, return the I420 buffer.
fn capture_and_convert(
    capturer: &(dyn Fn() -> Result<image::DynamicImage, String> + Send + Sync),
) -> Result<(I420Buffer, u32, u32), String> {
    let img = capturer()?;
    let rgba_img = img.to_rgba8();
    let actual_width = rgba_img.width() as usize;
    let width = rgba_img.width() & !1;
    let height = rgba_img.height() & !1;
    if width == 0 || height == 0 {
        return Err("zero dimensions".into());
    }
    let i420 = rgba_to_i420(&rgba_img, width, height, actual_width);
    Ok((i420, width, height))
}

/// The main capture loop.
async fn capture_loop(source_id: &str, video_source: NativeVideoSource, stop: Arc<AtomicBool>) {
    let capturer: Arc<dyn Fn() -> Result<image::DynamicImage, String> + Send + Sync> =
        if let Some(idx_str) = source_id.strip_prefix("monitor-") {
            let idx: usize = match idx_str.parse() {
                Ok(i) => i,
                Err(_) => {
                    tracing::error!("invalid monitor index: {idx_str}");
                    return;
                }
            };
            Arc::new(move || {
                let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
                let monitor = monitors
                    .into_iter()
                    .nth(idx)
                    .ok_or_else(|| format!("monitor {idx} not found"))?;
                let img = monitor.capture_image().map_err(|e| e.to_string())?;
                Ok(DynamicImage::ImageRgba8(img))
            })
        } else if let Some(id_str) = source_id.strip_prefix("window-") {
            let win_id: u32 = match id_str.parse() {
                Ok(i) => i,
                Err(_) => {
                    tracing::error!("invalid window id: {id_str}");
                    return;
                }
            };
            Arc::new(move || {
                let windows = xcap::Window::all().map_err(|e| e.to_string())?;
                let window = windows
                    .into_iter()
                    .find(|w| w.id().ok() == Some(win_id))
                    .ok_or_else(|| format!("window {win_id} not found"))?;
                let img = window.capture_image().map_err(|e| e.to_string())?;
                Ok(DynamicImage::ImageRgba8(img))
            })
        } else {
            tracing::error!("unknown source_id format: {source_id}");
            return;
        };

    let mut interval = tokio::time::interval(Duration::from_millis(67));

    loop {
        interval.tick().await;

        if stop.load(Ordering::Relaxed) {
            break;
        }

        // Run capture + RGBA→I420 conversion on a blocking thread
        // so we don't starve the tokio runtime (audio playout, etc.)
        let cap = capturer.clone();
        let result = tokio::task::spawn_blocking(move || capture_and_convert(cap.as_ref())).await;

        match result {
            Ok(Ok((i420, _w, _h))) => {
                let frame = VideoFrame {
                    rotation: VideoRotation::VideoRotation0,
                    timestamp_us: 0,
                    buffer: i420,
                };
                video_source.capture_frame(&frame);
                // Self-view: render locally so the user sees their own screen share
                visio_video::render_local_i420(&frame.buffer, "local-screen");
            }
            Ok(Err(e)) => {
                tracing::warn!("screen capture failed: {e}");
            }
            Err(e) => {
                tracing::warn!("capture task panicked: {e}");
                break;
            }
        }
    }

    tracing::info!("screen capture loop ended for {source_id}");
}
