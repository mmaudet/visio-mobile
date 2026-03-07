# Advanced Features Design — Visio Mobile

## Vision

Transform Visio Mobile from a solid video conferencing client into a disruptive, AI-powered communication tool. All AI processing runs on-device. No user data transits through third-party services. Compatible with any LiveKit server (not coupled to LaSuite).

## Positioning

Hybrid: enterprise-grade features with consumer-grade UX. Edge-first with data sovereignty as a core principle.

---

## Horizon 1 (short term)

### Feature 1: Live Subtitles (on-device)

**What:** Real-time speech-to-text displayed as an overlay on the video grid. Each participant chooses whether to enable subtitles and in which language they appear.

**Tech:**
- iOS/macOS: Whisper.cpp (Core ML accelerated)
- Android: ONNX Runtime with Whisper model (NNAPI GPU delegate)
- Desktop: Whisper.cpp (native)
- Model: Whisper small or base, quantized (int8), ~75MB on disk
- Input: local audio mix (microphone + received remote audio)
- Output: timestamped text segments

**UX:**
- Toggle button in control bar (CC icon)
- Subtitles appear at bottom of video grid, 2-3 lines max, auto-scroll
- Speaker attribution via voice activity detection (active speaker = current subtitle author)

**Data flow:**
```
Local audio mix -> Whisper on-device -> text segments -> subtitle overlay
                                            |
                                    [if scribe role]
                                            |
                                    buffer in memory
```

**Privacy:** Audio never leaves the device. Text stays in memory, not persisted.

---

### Feature 2: Background Blur / Replacement (on-device)

**What:** Real-time person segmentation to blur or replace the background before the video frame is sent to other participants.

**Tech:**
- iOS: Vision framework (PersonSegmentationRequest) or CoreML with custom model
- Android: MediaPipe Selfie Segmentation (TFLite, GPU delegate)
- Desktop: ONNX Runtime with MediaPipe model
- Processing: GPU-accelerated, integrated into visio-video pipeline
- Target: 30fps at 640x480 minimum on mid-range devices

**Pipeline integration:**
```
Camera frame (I420) -> segmentation model -> alpha mask
                                                |
                            original frame * mask + background * (1-mask)
                                                |
                                    composite frame -> NativeVideoSource
```

**UX:**
- Toggle in camera settings (in-call settings sheet)
- Options: Off / Blur / Image (2-3 preset images)
- Setting persisted across sessions

**Backgrounds:**
- Gaussian blur (sigma configurable, default 15)
- 2-3 built-in images shipped with the app (~200KB each)
- No custom image upload in v1 (YAGNI)

---

### Feature 3: Animated Reactions

**What:** Ephemeral emoji reactions that float over the video grid for 3 seconds, visible to all participants.

**Tech:**
- Transport: LiveKit participant attributes (same mechanism as hand raise)
- Attribute key: `reaction`, value: emoji string, TTL managed client-side
- No server-side logic needed

**Protocol:**
```
User taps reaction -> set participant attribute: { "reaction": "<emoji>" }
                          |
              LiveKit broadcasts attribute change to all participants
                          |
              Each client renders animated overlay for 3 seconds
                          |
              Client clears attribute after 3s
```

**UX:**
- Long-press or tap on a reaction bar (6-8 preset emojis: thumbs up, clap, heart, laugh, surprise, thinking, celebrate, thumbs down)
- Animation: emoji floats up from the participant's tile, fades out
- No sound effect (silent by design)
- Rate-limited: 1 reaction per participant per 2 seconds

**Emoji set:** Fixed, not customizable in v1.

---

### Feature 4: Silent Catch-up (late arrival summary)

**What:** When joining a meeting already in progress, the app generates a discreet summary of what was discussed before arrival.

**Architecture:**

**Scribe role:**
- The first participant using the native app (mobile or desktop) with subtitles enabled becomes the scribe automatically
- The scribe transcribes the full audio mix (all speakers) via Whisper on-device
- Transcript accumulates in a memory buffer on the scribe's device (not persisted to disk)
- If the scribe leaves, the next oldest native participant takes over (transcript restarts from that point)
- If no native participant is present, catch-up is unavailable

**Catch-up flow:**
```
Late participant joins
        |
Client sends "catch-up-request" via LiveKit data channel
        |
Scribe receives request
        |
Scribe sends transcript buffer via data channel (encrypted, peer-to-peer)
        |
Late participant's device runs summarization model on-device
        |
Summary displayed in discreet slide-up panel
```

**Summarization model:**
- Phi-3-mini (3.8B, 4-bit quantized, ~2GB) or Gemma 2B (4-bit, ~1.5GB)
- Downloaded on first use, cached locally
- Runs on device: CoreML (iOS), NNAPI/GPU (Android), ONNX (Desktop)
- Fallback: if device too slow, show raw transcript without summary

**UX:**
- Panel slides up from bottom, semi-transparent background
- Default view: bullet points (3-7 key points)
- Expandable: tap to see condensed transcript with speaker names
- Dismissible: swipe down or tap X
- No notification to other participants (silent)
- Panel auto-appears only if meeting has been running for >2 minutes

**Data lifecycle:**
- Transcript lives in scribe's memory only, cleared on meeting end
- Summary lives on the late participant's device only, cleared on meeting end
- Nothing is persisted or sent to any server

---

## Horizon 2 (medium term)

### Feature 5: Live Subtitle Translation

**What:** Each participant sees subtitles translated into their preferred language in real-time, regardless of the speaker's language.

**Tech:**
- Model: NLLB (No Language Left Behind) by Meta, quantized for mobile
- ~300MB model supporting 200+ languages
- Translation runs on each participant's device independently
- Input: text segments from Whisper transcription
- Output: translated text segments in the participant's chosen language

**Flow:**
```
Whisper output (source language) -> language detection -> NLLB on-device -> translated subtitles
```

**UX:**
- Language picker in subtitle settings
- Auto-detect source language (Whisper provides this)
- If source = target language, no translation applied

**Dependency:** Requires Feature 1 (Live Subtitles) as prerequisite.

---

### Feature 6: Adaptive Context Modes (seamless mobility)

**What:** The app detects the user's physical context (office WiFi, walking on cellular, car Bluetooth) and adapts video quality, audio routing, and UI layout automatically — with manual override.

**Context detection signals:**
- Network type: WiFi vs cellular (Reachability / ConnectivityManager)
- Bluetooth profile: A2DP car kit detection vs headphones vs speaker
- Motion sensors: accelerometer/pedometer to detect walking (CoreMotion / SensorManager)
- Screen lock state: screen off = pocket mode

**Three adaptive modes:**

| Mode | Trigger | Video | Audio | UI |
|------|---------|-------|-------|----|
| **Office** | WiFi connected, stationary | Full quality (720p) | Device speaker/headset | Standard full UI |
| **Pedestrian** | Cellular + motion detected | Reduced quality (360p) or receive-only | Earpiece or wired headset | Compact floating bubble (PiP-style) with essential controls; tap to expand to full-screen with larger buttons, reduced info, high contrast for outdoor visibility |
| **Car** | Bluetooth car kit connected | Outgoing video off, incoming audio-only | Bluetooth car audio (automatic routing) | Same compact bubble; audio-only by default |

**Mode transitions:**
- Automatic detection with smooth transition (no interruption to the call)
- Small toast notification: "Switched to pedestrian mode" (dismissible)
- Manual override: long-press mode indicator to force a specific mode or return to auto
- Override persists until end of call or until user re-enables auto

**Compact bubble UI (pedestrian/car):**
- Floating mini-overlay (similar to existing PiP but with controls)
- Shows: active speaker name, call duration, mute state
- Controls: mute toggle, hang up, expand to full view
- Tap to expand: full-screen with reorganized layout (larger buttons, fewer elements, stronger contrast)

**Audio routing:**
- WiFi → no change (user's current device)
- Bluetooth car kit detected → audio routes to Bluetooth automatically (standard OS behavior, no custom logic needed)
- Wired headset plugged in → audio routes to headset (OS default)
- App does not override OS audio routing decisions, only adapts UI/video

**Network adaptation:**
- WiFi: publish 720p, subscribe to all video tracks
- Cellular: publish 360p (or disable outgoing video), request lower-quality simulcast layers from server
- Poor signal: graceful degradation — video off, audio priority, reconnect handling (already implemented in visio-core)

**Platform scope:** Mobile only (Android + iOS). Desktop stays in office mode (no motion sensors, no Bluetooth car kit).

**Dependency:** Builds on existing PiP (iOS) and Picture-in-Picture (Android) implementations.

---

## Cross-cutting Concerns

### Model Management
- Models downloaded on-demand (not bundled with app)
- Download UI with progress indicator
- Models cached in app's documents directory
- Total storage budget: ~2.5GB max (Whisper 75MB + segmentation 10MB + summarization 2GB + NLLB 300MB)
- Summarization model is optional (only needed for catch-up)

### Performance Budget
- Subtitle transcription: <500ms latency, <15% CPU on mid-range device
- Background blur: <33ms per frame (maintain 30fps), GPU-only
- Reactions: negligible (attribute change + CSS/SwiftUI animation)
- Summarization: acceptable to take 5-10 seconds on first load (one-time per catch-up)

### Data Sovereignty
- Zero cloud dependency for AI features
- All models run locally
- Transcript data transmitted peer-to-peer via LiveKit data channels (E2E encrypted if LiveKit E2EE enabled)
- No telemetry, no analytics on AI features
- Models sourced from open-source projects (Whisper: MIT, NLLB: CC-BY-NC, MediaPipe: Apache 2.0)

### Platform Parity
All horizon 1 features target all 3 platforms (Android, iOS, Desktop) except Feature 6 (Adaptive Context Modes) which is mobile-only. Platform-specific implementations share the same UX and data protocols.
