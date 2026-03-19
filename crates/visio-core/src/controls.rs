use livekit::options::{TrackPublishOptions, VideoCodec, VideoEncoding};
use livekit::prelude::*;
use livekit::track::TrackSource as LkTrackSource;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::errors::VisioError;
use crate::events::{EventEmitter, VisioEvent};

/// Audio source options matching v1 settings.
const AUDIO_SAMPLE_RATE: u32 = 48_000;
const AUDIO_CHANNELS: u32 = 1;
const AUDIO_QUEUE_SIZE_MS: u32 = 100;

/// Resolve a resolution preset string to (width, height).
pub fn resolve_video_resolution(preset: &str) -> (u32, u32) {
    match preset {
        "360p" => (640, 360),
        "180p" => (320, 180),
        _ => (1280, 720), // "720p" or unknown → default
    }
}

/// Controls for local media (microphone, camera).
///
/// Manages local track creation, publishing, and mute/unmute.
/// Native UI shells feed captured audio/video frames into the
/// sources exposed by this struct.
pub struct MeetingControls {
    room: Arc<Mutex<Option<Arc<Room>>>>,
    emitter: EventEmitter,
    mic_enabled: Arc<Mutex<bool>>,
    camera_enabled: Arc<Mutex<bool>>,
    audio_source: Arc<Mutex<Option<NativeAudioSource>>>,
    video_source: Arc<Mutex<Option<NativeVideoSource>>>,
    screen_share_source: Arc<Mutex<Option<NativeVideoSource>>>,
    screen_share_audio_source: Arc<Mutex<Option<NativeAudioSource>>>,
}

/// Build TrackPublishOptions for the camera track.
///
/// Use VP8 on all platforms for maximum cross-platform decode compatibility.
/// VP9 was used on desktop but caused decode failures on some mobile clients
/// (the prebuilt WebRTC library for Android/iOS may not include VP9 decoder).
fn camera_publish_options() -> TrackPublishOptions {
    TrackPublishOptions {
        source: LkTrackSource::Camera,
        video_codec: VideoCodec::VP8,
        ..Default::default()
    }
}

impl MeetingControls {
    pub fn new(
        room: Arc<Mutex<Option<Arc<Room>>>>,
        emitter: EventEmitter,
        camera_enabled: Arc<Mutex<bool>>,
    ) -> Self {
        Self {
            room,
            emitter,
            mic_enabled: Arc::new(Mutex::new(false)),
            camera_enabled,
            audio_source: Arc::new(Mutex::new(None)),
            video_source: Arc::new(Mutex::new(None)),
            screen_share_source: Arc::new(Mutex::new(None)),
            screen_share_audio_source: Arc::new(Mutex::new(None)),
        }
    }

    /// Publish a microphone track to the room.
    ///
    /// Creates a NativeAudioSource and publishes an audio track.
    /// Returns the audio source so native code can feed PCM frames into it.
    pub async fn publish_microphone(&self) -> Result<NativeAudioSource, VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let source = NativeAudioSource::new(
            AudioSourceOptions {
                echo_cancellation: true,
                noise_suppression: true,
                auto_gain_control: true,
            },
            AUDIO_SAMPLE_RATE,
            AUDIO_CHANNELS,
            AUDIO_QUEUE_SIZE_MS,
        );

        let track = LocalAudioTrack::create_audio_track(
            "microphone",
            RtcAudioSource::Native(source.clone()),
        );

        room.local_participant()
            .publish_track(
                LocalTrack::Audio(track),
                TrackPublishOptions {
                    source: LkTrackSource::Microphone,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| VisioError::Room(format!("publish audio: {e}")))?;

        *self.mic_enabled.lock().await = true;
        *self.audio_source.lock().await = Some(source.clone());

        tracing::info!("microphone track published");
        self.emitter.emit(VisioEvent::TrackUnmuted {
            participant_sid: String::new(),
            source: crate::events::TrackSource::Microphone,
        });

        Ok(source)
    }

    /// Publish a camera track to the room.
    ///
    /// Creates a NativeVideoSource and publishes a video track.
    /// `resolution_preset` is "720p", "360p", or "180p".
    /// Returns the video source so native code can feed captured frames into it.
    pub async fn publish_camera(
        &self,
        resolution_preset: &str,
    ) -> Result<NativeVideoSource, VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let (width, height) = resolve_video_resolution(resolution_preset);
        let source = NativeVideoSource::new(
            VideoResolution { width, height },
            false, // not a screencast
        );

        let track =
            LocalVideoTrack::create_video_track("camera", RtcVideoSource::Native(source.clone()));

        // Log available video codecs for diagnostics (VP9 preference debugging)
        let opts = camera_publish_options();
        tracing::info!("publish_camera: requesting codec {:?}", opts.video_codec);

        room.local_participant()
            .publish_track(LocalTrack::Video(track), opts)
            .await
            .map_err(|e| VisioError::Room(format!("publish video: {e}")))?;

        *self.camera_enabled.lock().await = true;
        *self.video_source.lock().await = Some(source.clone());

        tracing::info!("camera track published");
        Ok(source)
    }

    /// Toggle the microphone on/off.
    ///
    /// If enabling and no microphone track has been published yet,
    /// automatically publishes one first.
    pub async fn set_microphone_enabled(&self, enabled: bool) -> Result<(), VisioError> {
        {
            let room = self.room.lock().await;
            let room = room
                .as_ref()
                .ok_or_else(|| VisioError::Room("not connected".into()))?;

            let local = room.local_participant();
            let has_mic_track = local
                .track_publications()
                .values()
                .any(|p| p.source() == LkTrackSource::Microphone);

            if has_mic_track {
                for (_, pub_) in local.track_publications() {
                    if pub_.source() == LkTrackSource::Microphone {
                        if enabled {
                            pub_.unmute();
                        } else {
                            pub_.mute();
                        }
                        break;
                    }
                }
                *self.mic_enabled.lock().await = enabled;
                tracing::info!("microphone enabled: {enabled}");
                return Ok(());
            }
        }
        // No mic track yet — publish if enabling, otherwise just update state.
        if enabled {
            self.publish_microphone().await?;
        } else {
            *self.mic_enabled.lock().await = false;
        }
        Ok(())
    }

    /// Toggle the camera on/off.
    ///
    /// If enabling and no camera track has been published yet,
    /// automatically publishes one first.
    pub async fn set_camera_enabled(&self, enabled: bool) -> Result<(), VisioError> {
        {
            let room = self.room.lock().await;
            let room = room
                .as_ref()
                .ok_or_else(|| VisioError::Room("not connected".into()))?;

            let local = room.local_participant();
            let has_camera_track = local
                .track_publications()
                .values()
                .any(|p| p.source() == LkTrackSource::Camera);

            if has_camera_track {
                for (_, pub_) in local.track_publications() {
                    if pub_.source() == LkTrackSource::Camera {
                        if enabled {
                            pub_.unmute();
                        } else {
                            pub_.mute();
                        }
                        break;
                    }
                }
                *self.camera_enabled.lock().await = enabled;
                tracing::info!("camera enabled: {enabled}");
                return Ok(());
            }
        }
        // No camera track yet — publish if enabling, otherwise just update state.
        if enabled {
            self.publish_camera("720p").await?;
        } else {
            *self.camera_enabled.lock().await = false;
        }
        Ok(())
    }

    /// Check if microphone is currently enabled.
    pub async fn is_microphone_enabled(&self) -> bool {
        *self.mic_enabled.lock().await
    }

    /// Check if camera is currently enabled.
    pub async fn is_camera_enabled(&self) -> bool {
        *self.camera_enabled.lock().await
    }

    /// Get the audio source for feeding PCM frames from native capture.
    pub async fn audio_source(&self) -> Option<NativeAudioSource> {
        self.audio_source.lock().await.clone()
    }

    /// Get the video source for feeding video frames from native capture.
    pub async fn video_source(&self) -> Option<NativeVideoSource> {
        self.video_source.lock().await.clone()
    }

    /// Publish a screen share track to the room.
    ///
    /// Returns the `NativeVideoSource` that the caller must feed with
    /// captured screen frames (platform-specific capture code).
    pub async fn publish_screen_share(&self) -> Result<NativeVideoSource, VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let source = NativeVideoSource::new(
            VideoResolution {
                width: 1920,
                height: 1080,
            },
            true, // is_screencast
        );

        let track = LocalVideoTrack::create_video_track(
            "screen_share",
            RtcVideoSource::Native(source.clone()),
        );

        room.local_participant()
            .publish_track(
                LocalTrack::Video(track),
                TrackPublishOptions {
                    source: LkTrackSource::Screenshare,
                    video_encoding: Some(VideoEncoding {
                        max_bitrate: 3_000_000,
                        max_framerate: 15.0,
                    }),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| VisioError::Room(format!("publish screen share: {e}")))?;

        *self.screen_share_source.lock().await = Some(source.clone());
        tracing::info!("screen share track published");
        Ok(source)
    }

    /// Stop publishing the screen share track.
    pub async fn stop_screen_share(&self) -> Result<(), VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let local = room.local_participant();
        for (_sid, pub_) in local.track_publications() {
            if pub_.source() == LkTrackSource::Screenshare {
                local
                    .unpublish_track(&pub_.sid())
                    .await
                    .map_err(|e| VisioError::Room(format!("unpublish screen share: {e}")))?;
                *self.screen_share_source.lock().await = None;
                tracing::info!("screen share track unpublished");
                break;
            }
        }
        Ok(())
    }

    /// Get the screen share video source for feeding captured frames.
    pub async fn screen_share_source(&self) -> Option<NativeVideoSource> {
        self.screen_share_source.lock().await.clone()
    }

    /// Publish an audio track for screen share audio (system/loopback audio).
    /// Returns the NativeAudioSource to feed captured system audio into.
    pub async fn publish_screen_share_audio(&self) -> Result<NativeAudioSource, VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let source = NativeAudioSource::new(
            AudioSourceOptions {
                echo_cancellation: false,
                noise_suppression: false,
                auto_gain_control: false,
            },
            AUDIO_SAMPLE_RATE,
            AUDIO_CHANNELS,
            AUDIO_QUEUE_SIZE_MS,
        );

        let track = LocalAudioTrack::create_audio_track(
            "screen_share_audio",
            RtcAudioSource::Native(source.clone()),
        );

        room.local_participant()
            .publish_track(
                LocalTrack::Audio(track),
                TrackPublishOptions {
                    source: LkTrackSource::ScreenshareAudio,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| VisioError::Room(format!("publish screen share audio: {e}")))?;

        *self.screen_share_audio_source.lock().await = Some(source.clone());
        tracing::info!("screen share audio track published");
        Ok(source)
    }

    /// Stop publishing the screen share audio track.
    pub async fn stop_screen_share_audio(&self) -> Result<(), VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let local = room.local_participant();
        for (_sid, pub_) in local.track_publications() {
            if pub_.source() == LkTrackSource::ScreenshareAudio {
                local
                    .unpublish_track(&pub_.sid())
                    .await
                    .map_err(|e| VisioError::Room(format!("unpublish screen share audio: {e}")))?;
                *self.screen_share_audio_source.lock().await = None;
                tracing::info!("screen share audio track unpublished");
                break;
            }
        }
        Ok(())
    }

    /// Get the screen share audio source.
    pub async fn screen_share_audio_source(&self) -> Option<NativeAudioSource> {
        self.screen_share_audio_source.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventEmitter;

    fn make_controls() -> (MeetingControls, Arc<Mutex<bool>>) {
        let room = Arc::new(Mutex::new(None));
        let emitter = EventEmitter::new();
        let camera_enabled = Arc::new(Mutex::new(false));
        let controls = MeetingControls::new(room, emitter, camera_enabled.clone());
        (controls, camera_enabled)
    }

    #[tokio::test]
    async fn camera_enabled_initial_state() {
        let (controls, _) = make_controls();
        assert!(!controls.is_camera_enabled().await);
    }

    #[tokio::test]
    async fn shared_camera_enabled_flag() {
        let (controls, camera_enabled) = make_controls();

        // Modify from outside
        *camera_enabled.lock().await = true;
        assert!(controls.is_camera_enabled().await);

        // Modify back
        *camera_enabled.lock().await = false;
        assert!(!controls.is_camera_enabled().await);
    }

    #[tokio::test]
    async fn set_camera_disabled_without_room() {
        let (controls, camera_enabled) = make_controls();

        // Start with camera enabled
        *camera_enabled.lock().await = true;
        assert!(controls.is_camera_enabled().await);

        // set_camera_enabled(false) without a room returns an error
        // (because it tries to lock the room and finds None),
        // but the `else` branch sets camera_enabled = false
        // when there is no camera track to mute.
        let result = controls.set_camera_enabled(false).await;
        // Without a connected room, this returns Err
        assert!(result.is_err());
    }

    #[test]
    fn resolve_video_resolution_presets() {
        assert_eq!(super::resolve_video_resolution("720p"), (1280, 720));
        assert_eq!(super::resolve_video_resolution("360p"), (640, 360));
        assert_eq!(super::resolve_video_resolution("180p"), (320, 180));
        // Unknown falls back to 720p
        assert_eq!(super::resolve_video_resolution("unknown"), (1280, 720));
    }

    #[test]
    fn camera_publish_options_codec() {
        use livekit::options::VideoCodec;
        let opts = super::camera_publish_options();
        // VP8 on all platforms for cross-platform decode compatibility
        assert!(
            matches!(opts.video_codec, VideoCodec::VP8),
            "expected VP8, got {:?}",
            opts.video_codec
        );
    }

    #[tokio::test]
    async fn mic_enabled_initial_state() {
        let (controls, _) = make_controls();
        assert!(!controls.is_microphone_enabled().await);
    }
}
