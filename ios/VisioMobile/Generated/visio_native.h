#ifndef visio_native_h
#define visio_native_h

#include <stdint.h>

// Audio playout — pull decoded remote audio samples (i16 PCM, 48kHz mono).
// Returns the number of samples actually written (rest is silence).
int32_t visio_pull_audio_playback(int16_t *buffer, uint32_t capacity);

// Video callback registration — receives I420 frame planes from Rust.
typedef void (*VisioIosFrameCallback)(
    uint32_t width, uint32_t height,
    const uint8_t *y_ptr, uint32_t y_stride,
    const uint8_t *u_ptr, uint32_t u_stride,
    const uint8_t *v_ptr, uint32_t v_stride,
    const char *track_sid, void *user_data
);
void visio_video_set_ios_callback(VisioIosFrameCallback callback, void *user_data);

// Audio capture — push PCM i16 audio frame into LiveKit NativeAudioSource.
// Used by SyntheticAudioCapture for E2E testing on simulators.
void visio_push_ios_audio_frame(
    const int16_t *data, uint32_t num_samples,
    uint32_t sample_rate, uint32_t num_channels
);

// Preview frame — process I420 frame for local camera preview
void visio_video_process_preview_frame(
    const uint8_t *y_ptr, uint32_t y_stride,
    const uint8_t *u_ptr, uint32_t u_stride,
    const uint8_t *v_ptr, uint32_t v_stride,
    uint32_t width, uint32_t height, uint32_t rotation
);

// Camera capture — push I420 frame from AVCaptureSession into LiveKit
void visio_push_ios_camera_frame(
    const uint8_t *y_ptr, uint32_t y_stride,
    const uint8_t *u_ptr, uint32_t u_stride,
    const uint8_t *v_ptr, uint32_t v_stride,
    uint32_t width, uint32_t height
);

#endif /* visio_native_h */
