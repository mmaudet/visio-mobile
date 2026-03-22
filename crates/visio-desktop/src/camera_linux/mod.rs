//! Linux camera capture — façade with runtime backend selection.
//!
//! Uses PipeWire Camera portal when available (Flatpak, modern desktops),
//! falls back to V4L2 direct access otherwise.

mod convert;
#[cfg(feature = "pipewire-camera")]
mod pipewire_backend;
mod v4l2;

#[cfg(feature = "pipewire-camera")]
use std::sync::OnceLock;

use livekit::webrtc::video_source::native::NativeVideoSource;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Public types (unchanged from original API)
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct VideoDeviceInfo {
    pub name: String,
    pub unique_id: String,
    pub is_default: bool,
}

// ---------------------------------------------------------------------------
// Runtime detection
// ---------------------------------------------------------------------------

/// Check if the PipeWire Camera portal is available.
/// Result is cached — tested once per process lifetime.
#[cfg(feature = "pipewire-camera")]
fn pipewire_portal_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        // Initialize PipeWire library (must be called once before any PW API)
        pipewire::init();

        // Spawn a dedicated thread with its own tokio runtime to avoid panicking
        // when called from within an existing tokio async context
        let result = std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio runtime: {e}"));
            match rt {
                Ok(rt) => rt.block_on(async {
                    match ashpd::desktop::camera::Camera::new().await {
                        Ok(camera) => camera.is_present().await.unwrap_or(false),
                        Err(_) => false,
                    }
                }),
                Err(_) => false,
            }
        })
        .join()
        .unwrap_or(false);

        if result {
            tracing::info!("PipeWire Camera portal available — using portal backend");
        } else {
            tracing::info!("PipeWire Camera portal not available — using V4L2 fallback");
        }

        result
    })
}

// ---------------------------------------------------------------------------
// Public API — delegates to the appropriate backend
// ---------------------------------------------------------------------------

pub fn list_cameras() -> Vec<VideoDeviceInfo> {
    #[cfg(feature = "pipewire-camera")]
    if pipewire_portal_available() {
        return pipewire_backend::list_cameras();
    }
    v4l2::list_cameras()
}

pub struct LinuxCameraCapture {
    inner: CaptureBackend,
}

enum CaptureBackend {
    V4l(v4l2::V4lCameraCapture),
    #[cfg(feature = "pipewire-camera")]
    Pipewire(pipewire_backend::PipewireCameraCapture),
}

impl LinuxCameraCapture {
    pub fn start(source: NativeVideoSource) -> Result<Self, String> {
        #[cfg(feature = "pipewire-camera")]
        if pipewire_portal_available() {
            let cap = pipewire_backend::PipewireCameraCapture::start(source)?;
            return Ok(Self {
                inner: CaptureBackend::Pipewire(cap),
            });
        }
        let cap = v4l2::V4lCameraCapture::start(0, source)?;
        Ok(Self {
            inner: CaptureBackend::V4l(cap),
        })
    }

    pub fn start_with_unique_id(
        unique_id: &str,
        source: NativeVideoSource,
    ) -> Result<Self, String> {
        #[cfg(feature = "pipewire-camera")]
        if unique_id == "pipewire:camera" {
            let cap = pipewire_backend::PipewireCameraCapture::start(source)?;
            return Ok(Self {
                inner: CaptureBackend::Pipewire(cap),
            });
        }
        // V4L2: parse "/dev/videoN" → index N
        let idx = if unique_id.starts_with("/dev/video") {
            unique_id
                .trim_start_matches("/dev/video")
                .parse::<usize>()
                .unwrap_or(0)
        } else {
            0
        };
        let cap = v4l2::V4lCameraCapture::start(idx, source)?;
        Ok(Self {
            inner: CaptureBackend::V4l(cap),
        })
    }

    pub fn stop(&mut self) {
        match &mut self.inner {
            CaptureBackend::V4l(cap) => cap.stop(),
            #[cfg(feature = "pipewire-camera")]
            CaptureBackend::Pipewire(cap) => cap.stop(),
        }
    }
}

impl Drop for LinuxCameraCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
