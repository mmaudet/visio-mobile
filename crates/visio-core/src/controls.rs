use std::sync::Arc;
use tokio::sync::Mutex;
use livekit::prelude::*;
use livekit::track::TrackSource as LkTrackSource;
use livekit::webrtc::prelude::*;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::options::TrackPublishOptions;

use crate::errors::VisioError;
use crate::events::{EventEmitter, VisioEvent};

/// Audio source options matching v1 settings.
const AUDIO_SAMPLE_RATE: u32 = 48_000;
const AUDIO_CHANNELS: u32 = 1;
const AUDIO_QUEUE_SIZE_MS: u32 = 100;

/// Default video resolution.
const VIDEO_WIDTH: u32 = 1280;
const VIDEO_HEIGHT: u32 = 720;

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
}

impl MeetingControls {
    pub fn new(room: Arc<Mutex<Option<Arc<Room>>>>, emitter: EventEmitter) -> Self {
        Self {
            room,
            emitter,
            mic_enabled: Arc::new(Mutex::new(false)),
            camera_enabled: Arc::new(Mutex::new(false)),
            audio_source: Arc::new(Mutex::new(None)),
            video_source: Arc::new(Mutex::new(None)),
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
        self.emitter
            .emit(VisioEvent::TrackMuted {
                participant_sid: String::new(),
                source: crate::events::TrackSource::Microphone,
            });

        Ok(source)
    }

    /// Publish a camera track to the room.
    ///
    /// Creates a NativeVideoSource and publishes a video track.
    /// Returns the video source so native code can feed captured frames into it.
    pub async fn publish_camera(&self) -> Result<NativeVideoSource, VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let source = NativeVideoSource::new(
            VideoResolution {
                width: VIDEO_WIDTH,
                height: VIDEO_HEIGHT,
            },
            false, // not a screencast
        );

        let track = LocalVideoTrack::create_video_track(
            "camera",
            RtcVideoSource::Native(source.clone()),
        );

        room.local_participant()
            .publish_track(
                LocalTrack::Video(track),
                TrackPublishOptions {
                    source: LkTrackSource::Camera,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| VisioError::Room(format!("publish video: {e}")))?;

        *self.camera_enabled.lock().await = true;
        *self.video_source.lock().await = Some(source.clone());

        tracing::info!("camera track published");
        Ok(source)
    }

    /// Toggle the microphone on/off.
    pub async fn set_microphone_enabled(&self, enabled: bool) -> Result<(), VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let local = room.local_participant();
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
        Ok(())
    }

    /// Toggle the camera on/off.
    pub async fn set_camera_enabled(&self, enabled: bool) -> Result<(), VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let local = room.local_participant();
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
}
