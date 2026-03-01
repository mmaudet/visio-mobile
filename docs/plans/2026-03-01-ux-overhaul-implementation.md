# UX/UI Overhaul Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring Visio Mobile v2 up to LaSuite Meet UX level across Desktop, Android, and iOS.

**Architecture:** Platform-by-platform approach (Rust core → Desktop → Android → iOS). Backend changes first (hand raise, unread count), then UI per platform.

**Tech Stack:** Rust (visio-core, visio-ffi), React/TypeScript + Remixicon (desktop), Kotlin/Compose (Android), SwiftUI (iOS), LiveKit Rust SDK 0.7.32.

**Design doc:** `docs/plans/2026-03-01-ux-overhaul-design.md`

---

## Phase 1: Rust Core — Hand Raise, Unread Count, FFI

### Task 1.1: Hand Raise — Core Logic

**Files:**
- Create: `crates/visio-core/src/hand_raise.rs`
- Modify: `crates/visio-core/src/lib.rs`
- Modify: `crates/visio-core/src/events.rs`

**Step 1: Add `HandRaisedChanged` event variant**

In `crates/visio-core/src/events.rs`, add to the `VisioEvent` enum (after `ConnectionQualityChanged`):

```rust
HandRaisedChanged {
    participant_sid: String,
    raised: bool,
    position: u32,
},
```

**Step 2: Create `hand_raise.rs` with tests**

```rust
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use livekit::Room;

use crate::events::{EventEmitter, VisioEvent};

/// Manages hand raise state for all participants.
/// Uses LiveKit participant attributes ({"handRaised": "<timestamp>"}) for interop with LaSuite Meet.
pub struct HandRaiseManager {
    room: Arc<Room>,
    emitter: Arc<EventEmitter>,
    /// timestamp → participant_sid, ordered by raise time
    raised_hands: Arc<Mutex<BTreeMap<i64, String>>>,
    auto_lower_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl HandRaiseManager {
    pub fn new(room: Arc<Room>, emitter: Arc<EventEmitter>) -> Self {
        Self {
            room,
            emitter,
            raised_hands: Arc::new(Mutex::new(BTreeMap::new())),
            auto_lower_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn raise_hand(&self) -> Result<(), crate::errors::VisioError> {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        self.room
            .local_participant()
            .set_attributes([("handRaised".to_string(), timestamp.clone())])
            .await
            .map_err(|e| crate::errors::VisioError::Room(e.to_string()))?;

        let ts: i64 = timestamp.parse().unwrap_or(0);
        let local_sid = self.room.local_participant().sid().to_string();
        let mut hands = self.raised_hands.lock().await;
        hands.insert(ts, local_sid.clone());
        let position = hands.values().position(|s| s == &local_sid).unwrap_or(0) as u32 + 1;
        drop(hands);

        self.emitter.emit(VisioEvent::HandRaisedChanged {
            participant_sid: local_sid,
            raised: true,
            position,
        });
        Ok(())
    }

    pub async fn lower_hand(&self) -> Result<(), crate::errors::VisioError> {
        self.room
            .local_participant()
            .set_attributes([("handRaised".to_string(), String::new())])
            .await
            .map_err(|e| crate::errors::VisioError::Room(e.to_string()))?;

        let local_sid = self.room.local_participant().sid().to_string();
        let mut hands = self.raised_hands.lock().await;
        hands.retain(|_, sid| sid != &local_sid);
        drop(hands);

        self.emitter.emit(VisioEvent::HandRaisedChanged {
            participant_sid: local_sid,
            raised: false,
            position: 0,
        });

        // Cancel auto-lower timer if running
        if let Some(handle) = self.auto_lower_handle.lock().await.take() {
            handle.abort();
        }
        Ok(())
    }

    pub async fn is_hand_raised(&self) -> bool {
        let local_sid = self.room.local_participant().sid().to_string();
        let hands = self.raised_hands.lock().await;
        hands.values().any(|sid| sid == &local_sid)
    }

    /// Called from event loop when a remote participant's attributes change.
    pub async fn handle_participant_attributes(
        &self,
        participant_sid: String,
        attributes: &std::collections::HashMap<String, String>,
    ) {
        let hand_raised_value = attributes.get("handRaised").cloned().unwrap_or_default();
        let is_raised = !hand_raised_value.is_empty();

        let mut hands = self.raised_hands.lock().await;
        if is_raised {
            let ts: i64 = hand_raised_value.parse().unwrap_or(0);
            if !hands.values().any(|s| s == &participant_sid) {
                hands.insert(ts, participant_sid.clone());
            }
        } else {
            hands.retain(|_, sid| sid != &participant_sid);
        }
        let position = if is_raised {
            hands.values().position(|s| s == &participant_sid).unwrap_or(0) as u32 + 1
        } else {
            0
        };
        drop(hands);

        self.emitter.emit(VisioEvent::HandRaisedChanged {
            participant_sid,
            raised: is_raised,
            position,
        });
    }

    /// Start auto-lower: if local participant speaks for 3 consecutive seconds with hand raised, lower automatically.
    pub fn start_auto_lower(&self, active_speakers: Vec<String>) {
        let local_sid = self.room.local_participant().sid().to_string();
        let is_speaking = active_speakers.contains(&local_sid);

        let raised_hands = self.raised_hands.clone();
        let auto_lower_handle = self.auto_lower_handle.clone();
        let room = self.room.clone();
        let emitter = self.emitter.clone();

        tokio::spawn(async move {
            // Cancel existing timer
            if let Some(handle) = auto_lower_handle.lock().await.take() {
                handle.abort();
            }

            if !is_speaking {
                return;
            }

            // Check if hand is raised
            let hand_raised = {
                let hands = raised_hands.lock().await;
                hands.values().any(|sid| sid == &local_sid)
            };

            if !hand_raised {
                return;
            }

            // Start 3-second timer
            let local_sid2 = local_sid.clone();
            let raised_hands2 = raised_hands.clone();
            let room2 = room.clone();
            let emitter2 = emitter.clone();

            let handle = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                // Re-check hand is still raised
                let still_raised = {
                    let hands = raised_hands2.lock().await;
                    hands.values().any(|sid| sid == &local_sid2)
                };

                if still_raised {
                    // Auto-lower
                    let _ = room2
                        .local_participant()
                        .set_attributes([("handRaised".to_string(), String::new())])
                        .await;

                    let mut hands = raised_hands2.lock().await;
                    hands.retain(|_, sid| sid != &local_sid2);
                    drop(hands);

                    emitter2.emit(VisioEvent::HandRaisedChanged {
                        participant_sid: local_sid2,
                        raised: false,
                        position: 0,
                    });
                }
            });

            *auto_lower_handle.lock().await = Some(handle);
        });
    }

    /// Clear all state (on disconnect)
    pub async fn clear(&self) {
        self.raised_hands.lock().await.clear();
        if let Some(handle) = self.auto_lower_handle.lock().await.take() {
            handle.abort();
        }
    }
}
```

**Step 3: Register module in `lib.rs`**

Add to `crates/visio-core/src/lib.rs`:

```rust
pub mod hand_raise;
pub use hand_raise::HandRaiseManager;
```

**Step 4: Run tests**

```bash
cd /Users/mmaudet/work/visio-mobile-v2 && cargo test -p visio-core
```

**Step 5: Commit**

```bash
git add crates/visio-core/src/hand_raise.rs crates/visio-core/src/events.rs crates/visio-core/src/lib.rs
git commit -m "feat(core): add HandRaiseManager with auto-lower and Meet interop"
```

---

### Task 1.2: Integrate Hand Raise into RoomManager

**Files:**
- Modify: `crates/visio-core/src/room.rs`

**Step 1: Add HandRaiseManager field to RoomManager**

In `RoomManager::new()`, create a `HandRaiseManager`. Add a `hand_raise()` accessor method.

**Step 2: Wire event loop**

In the event loop (`room.rs`), add handlers:

- `RoomEvent::ParticipantAttributesChanged { participant, changed_attributes, .. }` → call `hand_raise_manager.handle_participant_attributes()`
- In `ActiveSpeakersChanged` handler (already exists), call `hand_raise_manager.start_auto_lower(speaker_sids)`
- In `disconnect()`, call `hand_raise_manager.clear()`

**Step 3: Add hand raise methods to RoomManager**

```rust
pub async fn raise_hand(&self) -> Result<(), VisioError> {
    self.hand_raise.raise_hand().await
}

pub async fn lower_hand(&self) -> Result<(), VisioError> {
    self.hand_raise.lower_hand().await
}

pub async fn is_hand_raised(&self) -> bool {
    self.hand_raise.is_hand_raised().await
}
```

**Step 4: Run tests**

```bash
cargo test -p visio-core
```

**Step 5: Commit**

```bash
git add crates/visio-core/src/room.rs
git commit -m "feat(core): integrate HandRaiseManager into RoomManager event loop"
```

---

### Task 1.3: Unread Count Tracking

**Files:**
- Modify: `crates/visio-core/src/chat.rs`
- Modify: `crates/visio-core/src/events.rs`

**Step 1: Add `UnreadCountChanged` event**

In `events.rs`, add to `VisioEvent`:

```rust
UnreadCountChanged(u32),
```

**Step 2: Add unread tracking to ChatService**

In `chat.rs`, add fields:

```rust
unread_count: Arc<AtomicU32>,
chat_open: Arc<AtomicBool>,
```

Add methods:

```rust
pub fn set_chat_open(&self, open: bool) {
    self.chat_open.store(open, Ordering::Relaxed);
    if open {
        self.unread_count.store(0, Ordering::Relaxed);
        self.emitter.emit(VisioEvent::UnreadCountChanged(0));
    }
}

pub fn unread_count(&self) -> u32 {
    self.unread_count.load(Ordering::Relaxed)
}
```

In `handle_incoming()`, after storing the message:

```rust
if !self.chat_open.load(Ordering::Relaxed) {
    let count = self.unread_count.fetch_add(1, Ordering::Relaxed) + 1;
    self.emitter.emit(VisioEvent::UnreadCountChanged(count));
}
```

**Step 3: Run tests**

```bash
cargo test -p visio-core
```

**Step 4: Commit**

```bash
git add crates/visio-core/src/chat.rs crates/visio-core/src/events.rs
git commit -m "feat(core): add unread message count tracking with events"
```

---

### Task 1.4: FFI — Expose Hand Raise & Unread Count

**Files:**
- Modify: `crates/visio-ffi/src/visio.udl`
- Modify: `crates/visio-ffi/src/lib.rs`

**Step 1: Update UDL**

Add to `VisioEvent` enum:

```
HandRaisedChanged(string participant_sid, boolean raised, u32 position);
UnreadCountChanged(u32 count);
```

Add to `VisioClient` interface:

```
[Throws=VisioError]
void raise_hand();

[Throws=VisioError]
void lower_hand();

boolean is_hand_raised();

void set_chat_open(boolean open);

u32 unread_count();
```

**Step 2: Implement FFI bridge in `lib.rs`**

Add `VisioEvent::HandRaisedChanged` and `UnreadCountChanged` conversion in the event bridge (around line 260).

Add methods to `VisioClient` impl:

```rust
fn raise_hand(&self) -> Result<(), VisioError> {
    self.rt.block_on(self.room_manager.raise_hand())
        .map_err(|e| VisioError::Room { message: e.to_string() })
}

fn lower_hand(&self) -> Result<(), VisioError> {
    self.rt.block_on(self.room_manager.lower_hand())
        .map_err(|e| VisioError::Room { message: e.to_string() })
}

fn is_hand_raised(&self) -> bool {
    self.rt.block_on(self.room_manager.is_hand_raised())
}

fn set_chat_open(&self, open: bool) {
    self.chat_service.set_chat_open(open);
}

fn unread_count(&self) -> u32 {
    self.chat_service.unread_count()
}
```

**Step 3: Build to verify FFI compiles**

```bash
cargo build -p visio-ffi
```

**Step 4: Commit**

```bash
git add crates/visio-ffi/src/visio.udl crates/visio-ffi/src/lib.rs
git commit -m "feat(ffi): expose hand raise and unread count via UniFFI"
```

---

### Task 1.5: Desktop Tauri Commands for Hand Raise & Unread

**Files:**
- Modify: `crates/visio-desktop/src/lib.rs`

**Step 1: Add Tauri commands**

```rust
#[tauri::command]
async fn raise_hand(state: State<'_, VisioState>) -> Result<(), String> {
    let room = state.room_manager.lock().await;
    room.as_ref().ok_or("Not connected")?.raise_hand().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn lower_hand(state: State<'_, VisioState>) -> Result<(), String> {
    let room = state.room_manager.lock().await;
    room.as_ref().ok_or("Not connected")?.lower_hand().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn is_hand_raised(state: State<'_, VisioState>) -> Result<bool, String> {
    let room = state.room_manager.lock().await;
    Ok(room.as_ref().ok_or("Not connected")?.is_hand_raised().await)
}

#[tauri::command]
async fn set_chat_open(state: State<'_, VisioState>, open: bool) -> Result<(), String> {
    let chat = state.chat_service.lock().await;
    chat.as_ref().ok_or("Not connected")?.set_chat_open(open);
    Ok(())
}
```

**Step 2: Register commands in Tauri builder**

Add `raise_hand`, `lower_hand`, `is_hand_raised`, `set_chat_open` to the `.invoke_handler(tauri::generate_handler![...])` list.

**Step 3: Build to verify**

```bash
cargo build -p visio-desktop
```

**Step 4: Commit**

```bash
git add crates/visio-desktop/src/lib.rs
git commit -m "feat(desktop): add Tauri commands for hand raise and chat open state"
```

---

## Phase 2: Desktop — Full UI Overhaul

### Task 2.1: Install Remixicon & Dark Theme CSS

**Files:**
- Modify: `crates/visio-desktop/frontend/package.json`
- Rewrite: `crates/visio-desktop/frontend/src/App.css`

**Step 1: Install Remixicon**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend && npm install @remixicon/react
```

**Step 2: Rewrite `App.css` with Meet dark palette**

Replace the entire CSS with the Meet-inspired dark theme. Key CSS variables:

```css
:root {
  --color-primary-dark-50: #161622;
  --color-primary-dark-75: #222234;
  --color-primary-dark-100: #2D2D46;
  --color-primary-dark-300: #5A5A8F;
  --color-primary-500: #6A6AF4;
  --color-greyscale-400: #929292;
  --color-error-200: #6C302E;
  --color-error-500: #EF413D;
  --color-error-600: #EE6A66;
  --color-hand-raise: #fde047;
  --color-white: #FFFFFF;
}

* { margin: 0; padding: 0; box-sizing: border-box; }

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  background: var(--color-primary-dark-50);
  color: var(--color-white);
}
```

Full CSS covers: home form, control bar, buttons (grouped mic/cam + chevron), participant grid, tiles with metadata overlay, active speaker glow, chat sidebar, settings modal, avatar circles.

**Step 3: Verify it builds**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend && npm run build
```

**Step 4: Commit**

```bash
git add crates/visio-desktop/frontend/package.json crates/visio-desktop/frontend/package-lock.json crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): install Remixicon and rewrite CSS with Meet dark theme"
```

---

### Task 2.2: Control Bar Component

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Import Remixicon icons**

```tsx
import {
  RiMicLine, RiMicOffLine,
  RiVideoOnLine, RiVideoOffLine,
  RiArrowUpSLine,
  RiHand, RiChat1Line, RiPhoneFill,
  RiCloseLine, RiSendPlane2Fill,
  RiSettings3Line, RiArrowLeftSLine,
  RiMicFill, RiMicOffFill,
} from '@remixicon/react';
```

**Step 2: Replace text-based control buttons**

Replace the current control bar (text buttons "Mic", "Cam", "Chat", "End") with icon buttons in a grouped layout:

```tsx
<div className="control-bar">
  {/* Mic group */}
  <div className="control-group">
    <button
      className={`control-btn ${isMicEnabled ? '' : 'control-btn-off'}`}
      onClick={toggleMic}
      style={{ borderRadius: '8px 0 0 8px' }}
    >
      {isMicEnabled ? <RiMicLine size={20} /> : <RiMicOffLine size={20} />}
    </button>
    <button
      className="control-btn control-chevron"
      onClick={() => setShowMicPicker(!showMicPicker)}
      style={{ borderRadius: '0 8px 8px 0' }}
    >
      <RiArrowUpSLine size={16} />
    </button>
  </div>

  {/* Camera group */}
  <div className="control-group">
    <button
      className={`control-btn ${isCamEnabled ? '' : 'control-btn-off'}`}
      onClick={toggleCam}
      style={{ borderRadius: '8px 0 0 8px' }}
    >
      {isCamEnabled ? <RiVideoOnLine size={20} /> : <RiVideoOffLine size={20} />}
    </button>
    <button
      className="control-btn control-chevron"
      onClick={() => setShowCamPicker(!showCamPicker)}
      style={{ borderRadius: '0 8px 8px 0' }}
    >
      <RiArrowUpSLine size={16} />
    </button>
  </div>

  {/* Hand raise */}
  <button
    className={`control-btn ${isHandRaised ? 'control-btn-hand' : ''}`}
    onClick={toggleHandRaise}
  >
    <RiHand size={20} />
  </button>

  {/* Chat */}
  <button className="control-btn" onClick={toggleChat}>
    <RiChat1Line size={20} />
    {unreadCount > 0 && <span className="unread-badge">{unreadCount > 9 ? '9+' : unreadCount}</span>}
  </button>

  {/* Hangup */}
  <button className="control-btn control-btn-hangup" onClick={disconnect}>
    <RiPhoneFill size={20} />
  </button>
</div>
```

**Step 3: Add state for hand raise, unread, device pickers**

```tsx
const [isHandRaised, setIsHandRaised] = useState(false);
const [unreadCount, setUnreadCount] = useState(0);
const [showMicPicker, setShowMicPicker] = useState(false);
const [showCamPicker, setShowCamPicker] = useState(false);
const [showChat, setShowChat] = useState(false);
```

**Step 4: Add hand raise invoke calls**

```tsx
const toggleHandRaise = async () => {
  if (isHandRaised) {
    await invoke('lower_hand');
  } else {
    await invoke('raise_hand');
  }
  setIsHandRaised(!isHandRaised);
};
```

**Step 5: Build and verify**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend && npm run build
```

**Step 6: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx
git commit -m "feat(desktop): replace text buttons with Remixicon control bar"
```

---

### Task 2.3: Device Picker Popover

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Add device enumeration**

```tsx
const [audioInputs, setAudioInputs] = useState<MediaDeviceInfo[]>([]);
const [audioOutputs, setAudioOutputs] = useState<MediaDeviceInfo[]>([]);
const [videoInputs, setVideoInputs] = useState<MediaDeviceInfo[]>([]);
const [selectedAudioInput, setSelectedAudioInput] = useState<string>('');
const [selectedVideoInput, setSelectedVideoInput] = useState<string>('');

const enumerateDevices = async () => {
  const devices = await navigator.mediaDevices.enumerateDevices();
  setAudioInputs(devices.filter(d => d.kind === 'audioinput'));
  setAudioOutputs(devices.filter(d => d.kind === 'audiooutput'));
  setVideoInputs(devices.filter(d => d.kind === 'videoinput'));
};

useEffect(() => {
  enumerateDevices();
  navigator.mediaDevices.addEventListener('devicechange', enumerateDevices);
  return () => navigator.mediaDevices.removeEventListener('devicechange', enumerateDevices);
}, []);
```

**Step 2: Add popover component**

```tsx
{showMicPicker && (
  <div className="device-picker" style={{ bottom: '80px' }}>
    <div className="device-section">
      <div className="device-section-title">Microphone</div>
      {audioInputs.map(d => (
        <label key={d.deviceId} className="device-option">
          <input
            type="radio"
            name="audioInput"
            checked={selectedAudioInput === d.deviceId}
            onChange={() => setSelectedAudioInput(d.deviceId)}
          />
          {d.label || 'Microphone'}
        </label>
      ))}
    </div>
    <div className="device-section">
      <div className="device-section-title">Speaker</div>
      {audioOutputs.map(d => (
        <label key={d.deviceId} className="device-option">
          <input type="radio" name="audioOutput" />
          {d.label || 'Speaker'}
        </label>
      ))}
    </div>
  </div>
)}
```

Note: device selection on desktop is informational for now — the actual audio device is managed by cpal in Rust. Full device switching would require passing device IDs to Rust. For v1, the popover shows available devices and the system default is used.

**Step 3: Same pattern for camera picker**

Camera popover lists `videoInputs` with radio buttons.

**Step 4: Click-outside-to-close**

```tsx
useEffect(() => {
  const handleClick = (e: MouseEvent) => {
    if (!(e.target as Element).closest('.device-picker, .control-chevron')) {
      setShowMicPicker(false);
      setShowCamPicker(false);
    }
  };
  document.addEventListener('click', handleClick);
  return () => document.removeEventListener('click', handleClick);
}, []);
```

**Step 5: Build and verify**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend && npm run build
```

**Step 6: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): add device picker popovers for mic and camera"
```

---

### Task 2.4: Video Grid + Focus Layout

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Add focus state**

```tsx
const [focusedParticipant, setFocusedParticipant] = useState<string | null>(null);
```

**Step 2: Replace participant list with video grid**

Replace the current text-based participant list with a grid layout:

```tsx
<div className={`video-container ${showChat ? 'video-container-with-chat' : ''}`}>
  {focusedParticipant ? (
    // Focus mode
    <div className="focus-layout">
      <div className="focus-main" onClick={() => setFocusedParticipant(null)}>
        <ParticipantTile participant={participants.find(p => p.sid === focusedParticipant)!} large />
      </div>
      <div className="focus-strip">
        {participants.filter(p => p.sid !== focusedParticipant).map(p => (
          <div key={p.sid} onClick={() => setFocusedParticipant(p.sid)}>
            <ParticipantTile participant={p} />
          </div>
        ))}
      </div>
    </div>
  ) : (
    // Grid mode
    <div className={`video-grid video-grid-${Math.min(participants.length, 9)}`}>
      {participants.map(p => (
        <div key={p.sid} onClick={() => setFocusedParticipant(p.sid)}>
          <ParticipantTile participant={p} />
        </div>
      ))}
    </div>
  )}
</div>
```

**Step 3: CSS grid classes**

```css
.video-grid { display: grid; gap: 8px; height: 100%; }
.video-grid-1 { grid-template-columns: 1fr; }
.video-grid-2 { grid-template-columns: 1fr 1fr; }
.video-grid-3, .video-grid-4 { grid-template-columns: 1fr 1fr; grid-template-rows: 1fr 1fr; }
.video-grid-5, .video-grid-6 { grid-template-columns: 1fr 1fr 1fr; grid-template-rows: 1fr 1fr; }
.video-grid-7, .video-grid-8, .video-grid-9 { grid-template-columns: 1fr 1fr 1fr; grid-template-rows: 1fr 1fr 1fr; }

.focus-layout { display: flex; flex-direction: column; height: 100%; }
.focus-main { flex: 1; cursor: pointer; }
.focus-strip { display: flex; gap: 8px; height: 120px; overflow-x: auto; }

.video-container-with-chat { margin-right: calc(358px + 1rem); transition: margin .5s cubic-bezier(0.4,0,0.2,1); }
```

**Step 4: Build and verify**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend && npm run build
```

**Step 5: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): add video grid + focus layout with click-to-focus"
```

---

### Task 2.5: Participant Tile Component

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Create ParticipantTile component**

```tsx
interface ParticipantTileProps {
  participant: Participant;
  large?: boolean;
  isActiveSpeaker?: boolean;
  handRaisePosition?: number;
}

function ParticipantTile({ participant, large, isActiveSpeaker, handRaisePosition }: ParticipantTileProps) {
  const initials = (participant.name || participant.identity || '?')
    .split(' ').map(w => w[0]).join('').toUpperCase().slice(0, 2);

  // Deterministic color from name
  const hue = [...(participant.name || participant.identity || '')].reduce((h, c) => h + c.charCodeAt(0), 0) % 360;

  return (
    <div className={`tile ${isActiveSpeaker ? 'tile-active-speaker' : ''}`}>
      {participant.videoTrackSid && videoFrames[participant.videoTrackSid] ? (
        <img className="tile-video" src={videoFrames[participant.videoTrackSid]} alt="" />
      ) : (
        <div className="tile-avatar" style={{ background: `hsl(${hue}, 50%, 35%)` }}>
          <span className="tile-initials">{initials}</span>
        </div>
      )}
      <div className="tile-metadata">
        {participant.isMuted && <RiMicOffFill size={14} />}
        {handRaisePosition != null && handRaisePosition > 0 && (
          <span className="tile-hand-badge">
            <RiHand size={14} /> {handRaisePosition}
          </span>
        )}
        <span className="tile-name">{participant.name || participant.identity}</span>
        <ConnectionQualityBars quality={participant.connectionQuality} />
      </div>
    </div>
  );
}

function ConnectionQualityBars({ quality }: { quality: string }) {
  const bars = quality === 'Excellent' ? 3 : quality === 'Good' ? 2 : quality === 'Poor' ? 1 : 0;
  return (
    <div className="connection-bars">
      {[1, 2, 3].map(i => (
        <div key={i} className={`bar ${i <= bars ? 'bar-active' : ''}`} style={{ height: `${i * 4 + 2}px` }} />
      ))}
    </div>
  );
}
```

**Step 2: CSS for tiles**

```css
.tile {
  position: relative; border-radius: 8px; overflow: hidden;
  background: var(--color-primary-dark-50); cursor: pointer;
  transition: box-shadow 200ms ease-in-out, border-color 200ms ease-in-out;
  border: 2px solid transparent;
}
.tile-active-speaker {
  border-color: var(--color-primary-500);
  box-shadow: 0 0 12px rgba(106, 106, 244, 0.5);
}
.tile-video { width: 100%; height: 100%; object-fit: cover; }
.tile-avatar { display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; }
.tile-initials { font-size: 2rem; font-weight: bold; color: white; }
.tile-metadata {
  position: absolute; bottom: 0; left: 0; right: 0;
  display: flex; align-items: center; gap: 6px;
  padding: 4px 8px; background: rgba(0,0,0,0.6); font-size: 12px;
}
.tile-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tile-hand-badge {
  display: flex; align-items: center; gap: 2px;
  background: var(--color-hand-raise); color: black;
  padding: 1px 6px; border-radius: 10px; font-size: 11px; font-weight: 600;
}
```

**Step 3: Build and verify**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend && npm run build
```

**Step 4: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): add ParticipantTile with avatar, active speaker glow, hand raise"
```

---

### Task 2.6: Chat Sidebar

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Convert chat from full-screen modal to sidebar**

Replace the full-screen chat view with a right sidebar panel:

```tsx
{showChat && (
  <div className="chat-sidebar">
    <div className="chat-header">
      <span>Chat</span>
      <button className="chat-close" onClick={toggleChat}>
        <RiCloseLine size={20} />
      </button>
    </div>
    <div className="chat-messages" ref={chatScrollRef}>
      {messages.map((msg, i) => {
        const isOwn = msg.participantIdentity === localIdentity;
        const showName = i === 0 || messages[i-1].participantIdentity !== msg.participantIdentity;
        return (
          <div key={i} className={`chat-bubble ${isOwn ? 'chat-bubble-own' : ''}`}>
            {showName && !isOwn && <div className="chat-sender">{msg.participantName}</div>}
            <div className="chat-text">{msg.text}</div>
            <div className="chat-time">{formatTime(msg.timestamp)}</div>
          </div>
        );
      })}
    </div>
    <div className="chat-input-bar">
      <input
        className="chat-input"
        value={chatInput}
        onChange={e => setChatInput(e.target.value)}
        onKeyDown={e => e.key === 'Enter' && sendMessage()}
        placeholder="Message"
      />
      <button className="chat-send" onClick={sendMessage} disabled={!chatInput.trim()}>
        <RiSendPlane2Fill size={20} />
      </button>
    </div>
  </div>
)}
```

**Step 2: Wire chat open state to backend**

```tsx
const toggleChat = async () => {
  const newState = !showChat;
  setShowChat(newState);
  await invoke('set_chat_open', { open: newState });
  if (newState) setUnreadCount(0);
};
```

**Step 3: CSS for sidebar**

```css
.chat-sidebar {
  position: absolute; right: 0; top: 0; bottom: 0;
  width: 358px; background: var(--color-primary-dark-75);
  display: flex; flex-direction: column;
  border-left: 1px solid var(--color-primary-dark-100);
  animation: slide-in 0.3s cubic-bezier(0.4,0,0.2,1);
}
@keyframes slide-in { from { transform: translateX(100%); } to { transform: translateX(0); } }

.chat-bubble { padding: 8px 12px; border-radius: 12px; max-width: 80%; margin: 2px 0; background: var(--color-primary-dark-100); }
.chat-bubble-own { background: var(--color-primary-500); align-self: flex-end; margin-left: auto; }
```

**Step 4: Build and verify**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend && npm run build
```

**Step 5: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): replace chat modal with 358px sidebar panel"
```

---

### Task 2.7: Settings Modal

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Add settings state and modal**

```tsx
const [showSettings, setShowSettings] = useState(false);
const [settingsForm, setSettingsForm] = useState({ displayName: '', language: 'fr', micOnJoin: true, cameraOnJoin: false });

// Load settings on mount
useEffect(() => {
  invoke('get_settings').then((s: any) => {
    setSettingsForm({
      displayName: s.display_name || '',
      language: s.language || 'fr',
      micOnJoin: s.mic_enabled_on_join ?? true,
      cameraOnJoin: s.camera_enabled_on_join ?? false,
    });
  });
}, []);
```

**Step 2: Settings modal JSX**

```tsx
{showSettings && (
  <div className="modal-overlay" onClick={() => setShowSettings(false)}>
    <div className="settings-modal" onClick={e => e.stopPropagation()}>
      <div className="settings-header">
        <span>Settings</span>
        <button onClick={() => setShowSettings(false)}><RiCloseLine size={20} /></button>
      </div>
      <div className="settings-body">
        <div className="settings-section">
          <label className="settings-label">Display name</label>
          <input className="settings-input" value={settingsForm.displayName}
            onChange={e => setSettingsForm({...settingsForm, displayName: e.target.value})} />
        </div>
        <div className="settings-section">
          <label className="settings-label">Mic on join</label>
          <input type="checkbox" checked={settingsForm.micOnJoin}
            onChange={e => setSettingsForm({...settingsForm, micOnJoin: e.target.checked})} />
        </div>
        <div className="settings-section">
          <label className="settings-label">Camera on join</label>
          <input type="checkbox" checked={settingsForm.cameraOnJoin}
            onChange={e => setSettingsForm({...settingsForm, cameraOnJoin: e.target.checked})} />
        </div>
        <div className="settings-section">
          <label className="settings-label">Language</label>
          <select value={settingsForm.language}
            onChange={e => setSettingsForm({...settingsForm, language: e.target.value})}>
            <option value="fr">Français</option>
            <option value="en">English</option>
          </select>
        </div>
      </div>
      <button className="settings-save" onClick={saveSettings}>Save</button>
    </div>
  </div>
)}
```

**Step 3: Save handler**

```tsx
const saveSettings = async () => {
  await invoke('set_display_name', { name: settingsForm.displayName });
  await invoke('set_language', { language: settingsForm.language });
  await invoke('set_mic_enabled_on_join', { enabled: settingsForm.micOnJoin });
  await invoke('set_camera_enabled_on_join', { enabled: settingsForm.cameraOnJoin });
  setShowSettings(false);
};
```

**Step 4: Add settings gear button on Home screen**

```tsx
<button className="settings-gear" onClick={() => setShowSettings(true)}>
  <RiSettings3Line size={24} />
</button>
```

**Step 5: Build and verify**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend && npm run build
```

**Step 6: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): add settings modal with display name, language, join prefs"
```

---

### Task 2.8: Event Listener for Hand Raise & Unread

**Files:**
- Modify: `crates/visio-desktop/src/lib.rs`
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Emit Tauri events for HandRaisedChanged and UnreadCountChanged**

In the desktop `VisioEventListener` impl (the `VideoAutoStarter` or a new listener), handle new events and emit them to the frontend:

```rust
VisioEvent::HandRaisedChanged { participant_sid, raised, position } => {
    let _ = app_handle.emit("hand-raised-changed", serde_json::json!({
        "participantSid": participant_sid,
        "raised": raised,
        "position": position,
    }));
}
VisioEvent::UnreadCountChanged(count) => {
    let _ = app_handle.emit("unread-count-changed", count);
}
```

**Step 2: Listen in frontend**

```tsx
useEffect(() => {
  const unlistenHand = listen('hand-raised-changed', (event: any) => {
    const { participantSid, raised, position } = event.payload;
    setHandRaisedMap(prev => ({ ...prev, [participantSid]: raised ? position : 0 }));
    // If it's our own hand being auto-lowered
    if (participantSid === localSid && !raised) {
      setIsHandRaised(false);
    }
  });

  const unlistenUnread = listen('unread-count-changed', (event: any) => {
    setUnreadCount(event.payload);
  });

  return () => { unlistenHand.then(f => f()); unlistenUnread.then(f => f()); };
}, []);
```

**Step 3: Build full stack**

```bash
cargo build -p visio-desktop && cd crates/visio-desktop/frontend && npm run build
```

**Step 4: Commit**

```bash
git add crates/visio-desktop/src/lib.rs crates/visio-desktop/frontend/src/App.tsx
git commit -m "feat(desktop): wire hand raise and unread count events to frontend"
```

---

### Task 2.9: App Title & Home Screen Polish

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`
- Modify: `crates/visio-desktop/frontend/index.html`
- Modify: `crates/visio-desktop/tauri.conf.json`

**Step 1: Update title to "Visio Mobile"**

In `tauri.conf.json`: set `"title": "Visio Mobile"`.

In `index.html`: `<title>Visio Mobile</title>`.

In `App.tsx` home screen header: replace "Visio" with "Visio Mobile".

**Step 2: Build and verify**

```bash
cargo build -p visio-desktop
```

**Step 3: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/index.html crates/visio-desktop/tauri.conf.json
git commit -m "feat(desktop): set app title to Visio Mobile"
```

---

## Phase 3: Android — UI Overhaul

### Task 3.1: Remixicon SVG Drawables

**Files:**
- Create: `android/app/src/main/res/drawable/ri_mic_line.xml` (and ~15 more)

**Step 1: Download SVGs from remixicon.com**

Download each icon SVG and convert to Android vector drawable XML using Android Studio's Vector Asset tool, or manually convert. Each file is an Android `<vector>` XML:

```xml
<!-- ri_mic_line.xml -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp" android:height="24dp"
    android:viewportWidth="24" android:viewportHeight="24">
    <path android:fillColor="#FFFFFF"
        android:pathData="M12,1a4,4,0,0,1,4,4v6a4,4,0,0,1,-8,0V5A4,4,0,0,1,12,1Z..." />
</vector>
```

Create drawables for: `ri_mic_line`, `ri_mic_off_line`, `ri_video_on_line`, `ri_video_off_line`, `ri_arrow_up_s_line`, `ri_camera_switch_line`, `ri_hand`, `ri_chat_1_line`, `ri_phone_fill`, `ri_mic_off_fill`, `ri_close_line`, `ri_send_plane_2_fill`, `ri_settings_3_line`, `ri_arrow_left_s_line`.

**Step 2: Commit**

```bash
git add android/app/src/main/res/drawable/ri_*.xml
git commit -m "feat(android): add Remixicon SVG vector drawables"
```

---

### Task 3.2: Dark Theme Colors

**Files:**
- Create: `android/app/src/main/kotlin/io/visio/mobile/ui/theme/Colors.kt`
- Create: `android/app/src/main/kotlin/io/visio/mobile/ui/theme/Theme.kt`

**Step 1: Create color constants**

```kotlin
package io.visio.mobile.ui.theme

import androidx.compose.ui.graphics.Color

object VisioColors {
    val PrimaryDark50 = Color(0xFF161622)
    val PrimaryDark75 = Color(0xFF222234)
    val PrimaryDark100 = Color(0xFF2D2D46)
    val PrimaryDark300 = Color(0xFF5A5A8F)
    val Primary500 = Color(0xFF6A6AF4)
    val Greyscale400 = Color(0xFF929292)
    val Error200 = Color(0xFF6C302E)
    val Error500 = Color(0xFFEF413D)
    val Error600 = Color(0xFFEE6A66)
    val HandRaise = Color(0xFFfde047)
    val White = Color(0xFFFFFFFF)
}
```

**Step 2: Create theme wrapper**

```kotlin
package io.visio.mobile.ui.theme

import androidx.compose.material3.*
import androidx.compose.runtime.Composable

private val DarkColorScheme = darkColorScheme(
    background = VisioColors.PrimaryDark50,
    surface = VisioColors.PrimaryDark75,
    primary = VisioColors.Primary500,
    error = VisioColors.Error500,
    onBackground = VisioColors.White,
    onSurface = VisioColors.White,
    onPrimary = VisioColors.White,
)

@Composable
fun VisioTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = DarkColorScheme, content = content)
}
```

**Step 3: Wrap the app in VisioTheme in MainActivity**

**Step 4: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/theme/
git commit -m "feat(android): add Meet dark theme with VisioColors"
```

---

### Task 3.3: Android Control Bar + Audio Bottom Sheet

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt`

**Step 1: Replace BottomAppBar with custom control bar using Remixicon drawables**

Replace Material Icons with `painterResource(R.drawable.ri_mic_line)` etc. Add grouped mic+chevron button, camera+switch button, hand raise, chat with badge, hangup.

**Step 2: Add audio device bottom sheet**

```kotlin
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AudioDeviceSheet(onDismiss: () -> Unit, onSelect: (AudioDeviceInfo) -> Unit) {
    val context = LocalContext.current
    val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
    val devices = audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
        .filter { it.type in listOf(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER, AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, AudioDeviceInfo.TYPE_BLUETOOTH_SCO, AudioDeviceInfo.TYPE_WIRED_HEADSET) }

    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = VisioColors.PrimaryDark75) {
        Text("Audio source", modifier = Modifier.padding(16.dp), style = MaterialTheme.typography.titleMedium)
        devices.forEach { device ->
            Row(modifier = Modifier.fillMaxWidth().clickable { onSelect(device) }.padding(16.dp)) {
                Text(device.productName.toString(), color = VisioColors.White)
            }
        }
        Spacer(modifier = Modifier.height(32.dp))
    }
}
```

**Step 3: Wire device selection**

```kotlin
val onSelectAudioDevice: (AudioDeviceInfo) -> Unit = { device ->
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        audioManager.setCommunicationDevice(device)
    }
    showAudioSheet = false
}
```

**Step 4: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt
git commit -m "feat(android): Remixicon control bar with audio device bottom sheet"
```

---

### Task 3.4: Android Video Grid + Participant Tiles

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt`

**Step 1: Replace LazyColumn participant list with grid layout**

Use `LazyVerticalGrid` for grid mode, custom layout for focus mode.

**Step 2: Create ParticipantTile composable**

With video surface or initials avatar, metadata bar (muted icon, hand raise badge, name, connection bars), active speaker border glow.

**Step 3: Active speaker glow**

```kotlin
val borderColor = if (isActiveSpeaker) VisioColors.Primary500 else Color.Transparent
Box(modifier = Modifier
    .border(2.dp, borderColor, RoundedCornerShape(8.dp))
    .then(if (isActiveSpeaker) Modifier.shadow(8.dp, RoundedCornerShape(8.dp), ambientColor = VisioColors.Primary500) else Modifier)
)
```

**Step 4: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt
git commit -m "feat(android): video grid layout with participant tiles and active speaker"
```

---

### Task 3.5: Android Hand Raise + Chat Badge + Settings

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt`
- Create: `android/app/src/main/kotlin/io/visio/mobile/ui/SettingsScreen.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt`

**Step 1: Add hand raise state to VisioManager**

```kotlin
private val _handRaisedMap = MutableStateFlow<Map<String, Int>>(emptyMap())
val handRaisedMap: StateFlow<Map<String, Int>> = _handRaisedMap.asStateFlow()

private val _unreadCount = MutableStateFlow(0)
val unreadCount: StateFlow<Int> = _unreadCount.asStateFlow()
```

Handle `HandRaisedChanged` and `UnreadCountChanged` events in the listener.

**Step 2: Wire hand raise button in CallScreen**

```kotlin
IconButton(onClick = {
    scope.launch(Dispatchers.IO) {
        if (isHandRaised) client.lowerHand() else client.raiseHand()
    }
}) {
    Icon(painterResource(R.drawable.ri_hand), "Hand raise",
        tint = if (isHandRaised) VisioColors.HandRaise else VisioColors.White)
}
```

**Step 3: Chat badge**

```kotlin
BadgedBox(badge = {
    if (unreadCount > 0) Badge { Text(if (unreadCount > 9) "9+" else "$unreadCount") }
}) {
    Icon(painterResource(R.drawable.ri_chat_1_line), "Chat")
}
```

**Step 4: Create SettingsScreen**

Compose screen with `OutlinedTextField` for display name, `Switch` for mic/camera on join, radio buttons for language. Save calls UniFFI methods.

**Step 5: Add settings gear on HomeScreen and navigation**

**Step 6: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/
git commit -m "feat(android): hand raise, chat badge, settings screen"
```

---

### Task 3.6: Android PiP

**Files:**
- Modify: `android/app/src/main/AndroidManifest.xml`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt`

**Step 1: AndroidManifest — add PiP support**

```xml
<activity
    android:name=".MainActivity"
    android:supportsPictureInPicture="true"
    android:configChanges="screenSize|smallestScreenSize|screenLayout|orientation"
    ...>
```

**Step 2: MainActivity — enter PiP on leave**

```kotlin
override fun onUserLeaveHint() {
    super.onUserLeaveHint()
    if (VisioManager.connectionState.value.name == "Connected") {
        enterPictureInPictureMode(
            PictureInPictureParams.Builder()
                .setAspectRatio(Rational(16, 9))
                .build()
        )
    }
}
```

**Step 3: CallScreen — simplified PiP layout**

```kotlin
val isInPiP = LocalContext.current.findActivity()?.isInPictureInPictureMode == true

if (isInPiP) {
    // Show only active speaker tile, no controls
    Box(modifier = Modifier.fillMaxSize().background(VisioColors.PrimaryDark50)) {
        activeSpeaker?.let { ParticipantTile(it, large = true) }
    }
} else {
    // Normal call screen layout
    ...
}
```

**Step 4: Remote actions (mute/hangup)**

```kotlin
val muteAction = RemoteAction(
    Icon.createWithResource(this, R.drawable.ri_mic_off_line),
    "Mute", "Toggle mute",
    PendingIntent.getBroadcast(this, 0, Intent("io.visio.mobile.TOGGLE_MIC"), PendingIntent.FLAG_IMMUTABLE)
)
val hangupAction = RemoteAction(
    Icon.createWithResource(this, R.drawable.ri_phone_fill),
    "Hangup", "End call",
    PendingIntent.getBroadcast(this, 1, Intent("io.visio.mobile.HANGUP"), PendingIntent.FLAG_IMMUTABLE)
)
params.setActions(listOf(muteAction, hangupAction))
```

**Step 5: BroadcastReceiver for remote actions**

```kotlin
class PiPActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            "io.visio.mobile.TOGGLE_MIC" -> VisioManager.toggleMicFromPiP()
            "io.visio.mobile.HANGUP" -> VisioManager.disconnectFromPiP()
        }
    }
}
```

Register in manifest.

**Step 6: Commit**

```bash
git add android/app/src/main/AndroidManifest.xml android/app/src/main/kotlin/io/visio/mobile/
git commit -m "feat(android): PiP mode with remote actions for mute and hangup"
```

---

### Task 3.7: Android App Title

**Files:**
- Modify: `android/app/src/main/res/values/strings.xml`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt`

**Step 1: Update app_name**

```xml
<string name="app_name">Visio Mobile</string>
```

**Step 2: Update HomeScreen header**

Replace `Text("Visio", ...)` with `Text("Visio Mobile", ...)`.

**Step 3: Commit**

```bash
git add android/app/src/main/res/values/strings.xml android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt
git commit -m "feat(android): set app title to Visio Mobile"
```

---

## Phase 4: iOS — UI Overhaul

### Task 4.1: Remixicon SVG Assets

**Files:**
- Create: `ios/VisioMobile/Assets.xcassets/Icons/` — SVG assets for each Remixicon icon

**Step 1: Add SVGs to asset catalog**

Same 15 icons as Android. Add as PDF or SVG in asset catalog with `Preserve Vector Data` checked. Name them: `ri-mic-line`, `ri-mic-off-line`, `ri-video-on-line`, etc.

**Step 2: Commit**

```bash
git add ios/VisioMobile/Assets.xcassets/
git commit -m "feat(ios): add Remixicon SVG icon assets"
```

---

### Task 4.2: iOS Dark Theme

**Files:**
- Create: `ios/VisioMobile/Theme.swift`

```swift
import SwiftUI

enum VisioColors {
    static let primaryDark50 = Color(hex: 0x161622)
    static let primaryDark75 = Color(hex: 0x222234)
    static let primaryDark100 = Color(hex: 0x2D2D46)
    static let primaryDark300 = Color(hex: 0x5A5A8F)
    static let primary500 = Color(hex: 0x6A6AF4)
    static let greyscale400 = Color(hex: 0x929292)
    static let error200 = Color(hex: 0x6C302E)
    static let error500 = Color(hex: 0xEF413D)
    static let handRaise = Color(hex: 0xfde047)
}

extension Color {
    init(hex: UInt, alpha: Double = 1) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255,
            opacity: alpha
        )
    }
}
```

**Step 1: Apply theme to all views**

Replace `.blue`, `.red`, SF Symbols with Remixicon assets and `VisioColors.*` across HomeView, CallView, ChatView.

**Step 2: Commit**

```bash
git add ios/VisioMobile/Theme.swift
git commit -m "feat(ios): add Meet dark theme colors"
```

---

### Task 4.3: iOS Control Bar + Audio Bottom Sheet

**Files:**
- Modify: `ios/VisioMobile/Views/CallView.swift`

**Step 1: Replace bottom toolbar with custom control bar**

Replace SF Symbols with `Image("ri-mic-line")` etc. Add mic chevron for audio device picker, camera switch button, hand raise button with yellow tint, chat with badge, hangup.

**Step 2: Audio device sheet**

```swift
struct AudioDeviceSheet: View {
    @State private var availableInputs: [AVAudioSessionPortDescription] = []
    @State private var currentInput: AVAudioSessionPortDescription?

    var body: some View {
        NavigationStack {
            List(availableInputs, id: \.uid) { port in
                Button(action: { selectInput(port) }) {
                    HStack {
                        Text(port.portName)
                        Spacer()
                        if port.uid == currentInput?.uid {
                            Image(systemName: "checkmark")
                        }
                    }
                }
            }
            .navigationTitle("Audio Source")
        }
        .onAppear { loadDevices() }
    }

    private func loadDevices() {
        let session = AVAudioSession.sharedInstance()
        availableInputs = session.availableInputs ?? []
        currentInput = session.currentRoute.inputs.first
    }

    private func selectInput(_ port: AVAudioSessionPortDescription) {
        try? AVAudioSession.sharedInstance().setPreferredInput(port)
        currentInput = port
    }
}
```

**Step 3: Commit**

```bash
git add ios/VisioMobile/Views/CallView.swift
git commit -m "feat(ios): Remixicon control bar with audio device picker sheet"
```

---

### Task 4.4: iOS Video Grid + Participant Tiles

**Files:**
- Modify: `ios/VisioMobile/Views/CallView.swift`

**Step 1: Replace ScrollView + List with LazyVGrid**

```swift
@State private var focusedParticipant: String? = nil

let columns = Array(repeating: GridItem(.flexible(), spacing: 8), count: participants.count <= 2 ? 1 : 2)

if let focused = focusedParticipant {
    // Focus layout
    VStack(spacing: 8) {
        ParticipantTile(participant: participants.first { $0.sid == focused }!, large: true)
            .onTapGesture { focusedParticipant = nil }
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(participants.filter { $0.sid != focused }, id: \.sid) { p in
                    ParticipantTile(participant: p)
                        .frame(width: 160, height: 120)
                        .onTapGesture { focusedParticipant = p.sid }
                }
            }
        }.frame(height: 120)
    }
} else {
    // Grid layout
    LazyVGrid(columns: columns, spacing: 8) {
        ForEach(participants, id: \.sid) { p in
            ParticipantTile(participant: p)
                .aspectRatio(16/9, contentMode: .fit)
                .onTapGesture { focusedParticipant = p.sid }
        }
    }
}
```

**Step 2: ParticipantTile view**

```swift
struct ParticipantTile: View {
    let participant: ParticipantInfo
    var large: Bool = false
    var isActiveSpeaker: Bool = false
    var handRaisePosition: Int = 0

    var body: some View {
        ZStack(alignment: .bottom) {
            // Video or avatar
            if let trackSid = participant.videoTrackSid {
                VideoLayerView(trackSid: trackSid)
            } else {
                Circle()
                    .fill(Color(hue: nameHue, saturation: 0.5, brightness: 0.35))
                    .frame(width: 64, height: 64)
                    .overlay(Text(initials).font(.title2).bold().foregroundColor(.white))
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background(VisioColors.primaryDark50)
            }
            // Metadata bar
            HStack(spacing: 6) {
                if participant.isMuted { Image("ri-mic-off-fill").resizable().frame(width: 14, height: 14) }
                if handRaisePosition > 0 {
                    HStack(spacing: 2) {
                        Image("ri-hand").resizable().frame(width: 14, height: 14)
                        Text("\(handRaisePosition)").font(.caption2).bold()
                    }
                    .padding(.horizontal, 6).padding(.vertical, 1)
                    .background(VisioColors.handRaise).cornerRadius(10)
                    .foregroundColor(.black)
                }
                Spacer()
                Text(participant.name ?? participant.identity).font(.caption).lineLimit(1)
            }
            .padding(.horizontal, 8).padding(.vertical, 4)
            .background(Color.black.opacity(0.6))
        }
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isActiveSpeaker ? VisioColors.primary500 : .clear, lineWidth: 2)
                .shadow(color: isActiveSpeaker ? VisioColors.primary500.opacity(0.5) : .clear, radius: 6)
        )
    }
}
```

**Step 3: Commit**

```bash
git add ios/VisioMobile/Views/CallView.swift
git commit -m "feat(ios): video grid + focus layout with participant tiles and active speaker"
```

---

### Task 4.5: iOS Hand Raise, Chat Badge, Settings

**Files:**
- Modify: `ios/VisioMobile/VisioManager.swift`
- Modify: `ios/VisioMobile/Views/CallView.swift`
- Modify: `ios/VisioMobile/Views/ChatView.swift`
- Create: `ios/VisioMobile/Views/SettingsView.swift`
- Modify: `ios/VisioMobile/Views/HomeView.swift`

**Step 1: Add published properties to VisioManager**

```swift
@Published var handRaisedMap: [String: Int] = [:]
@Published var unreadCount: Int = 0
@Published var isHandRaised: Bool = false
```

Handle `HandRaisedChanged` and `UnreadCountChanged` in the event listener.

**Step 2: Hand raise button + chat badge in CallView**

**Step 3: Settings view**

```swift
struct SettingsView: View {
    @EnvironmentObject var manager: VisioManager
    @State private var displayName = ""
    @State private var micOnJoin = true
    @State private var cameraOnJoin = false
    @State private var language = "fr"
    @Environment(\.dismiss) var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section("Profile") {
                    TextField("Display name", text: $displayName)
                }
                Section("Join meeting") {
                    Toggle("Mic enabled", isOn: $micOnJoin)
                    Toggle("Camera enabled", isOn: $cameraOnJoin)
                }
                Section("Language") {
                    Picker("Language", selection: $language) {
                        Text("Français").tag("fr")
                        Text("English").tag("en")
                    }.pickerStyle(.inline)
                }
            }
            .navigationTitle("Settings")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save(); dismiss() }
                }
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
        .onAppear { load() }
    }
}
```

**Step 4: Settings gear on HomeView + NavigationLink**

**Step 5: Commit**

```bash
git add ios/VisioMobile/
git commit -m "feat(ios): hand raise, chat badge, settings view"
```

---

### Task 4.6: iOS CallKit

**Files:**
- Create: `ios/VisioMobile/Services/CallKitManager.swift`

**Step 1: Implement CallKitManager**

```swift
import CallKit
import AVFoundation

class CallKitManager: NSObject, CXProviderDelegate {
    static let shared = CallKitManager()
    private let provider: CXProvider
    private let callController = CXCallController()
    private var currentCallUUID: UUID?

    override init() {
        let config = CXProviderConfiguration()
        config.supportsVideo = true
        config.maximumCallsPerCallGroup = 1
        config.supportedHandleTypes = [.generic]
        provider = CXProvider(configuration: config)
        super.init()
        provider.setDelegate(self, queue: nil)
    }

    func reportCallStarted(roomName: String) {
        let uuid = UUID()
        currentCallUUID = uuid
        let handle = CXHandle(type: .generic, value: roomName)
        let action = CXStartCallAction(call: uuid, handle: handle)
        action.isVideo = true
        callController.request(CXTransaction(action: action)) { error in
            if error == nil {
                self.provider.reportOutgoingCall(with: uuid, connectedAt: Date())
            }
        }
    }

    func reportCallEnded() {
        guard let uuid = currentCallUUID else { return }
        let action = CXEndCallAction(call: uuid)
        callController.request(CXTransaction(action: action), completion: { _ in })
        currentCallUUID = nil
    }

    func providerDidReset(_ provider: CXProvider) {
        // Clean up
    }

    func provider(_ provider: CXProvider, perform action: CXEndCallAction) {
        VisioManager.shared.disconnect()
        action.fulfill()
    }

    func provider(_ provider: CXProvider, perform action: CXSetMutedCallAction) {
        VisioManager.shared.toggleMic()
        action.fulfill()
    }

    func provider(_ provider: CXProvider, perform action: CXSetHeldCallAction) {
        // Mute mic when held (phone call incoming)
        if action.isOnHold {
            VisioManager.shared.setMicEnabled(false)
        }
        action.fulfill()
    }
}
```

**Step 2: Wire to VisioManager**

In `connect()`: `CallKitManager.shared.reportCallStarted(roomName: url)`
In `disconnect()`: `CallKitManager.shared.reportCallEnded()`

**Step 3: Commit**

```bash
git add ios/VisioMobile/Services/CallKitManager.swift ios/VisioMobile/VisioManager.swift
git commit -m "feat(ios): CallKit integration for system call UI and interruptions"
```

---

### Task 4.7: iOS PiP

**Files:**
- Create: `ios/VisioMobile/Services/PiPManager.swift`
- Modify: `ios/VisioMobile/Info.plist`

**Step 1: Add background modes**

In `Info.plist`, add `UIBackgroundModes`: `audio`, `voip`.

**Step 2: Implement PiPManager**

```swift
import AVKit

class PiPManager: NSObject, AVPictureInPictureControllerDelegate {
    static let shared = PiPManager()
    private var pipController: AVPictureInPictureController?
    private let displayLayer = AVSampleBufferDisplayLayer()

    func setup() {
        guard AVPictureInPictureController.isPictureInPictureSupported() else { return }
        let source = AVPictureInPictureController.ContentSource(sampleBufferDisplayLayer: displayLayer, playbackDelegate: self)
        pipController = AVPictureInPictureController(contentSource: source)
        pipController?.delegate = self
    }

    func pushFrame(_ pixelBuffer: CVPixelBuffer, timestamp: CMTime) {
        var sampleBuffer: CMSampleBuffer?
        var formatDesc: CMFormatDescription?
        CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil, imageBuffer: pixelBuffer, formatDescriptionOut: &formatDesc)
        guard let format = formatDesc else { return }
        var timingInfo = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: timestamp, decodeTimeStamp: .invalid)
        CMSampleBufferCreateForImageBuffer(allocator: nil, imageBuffer: pixelBuffer, dataReady: true, makeDataReadyCallback: nil, refcon: nil, formatDescription: format, sampleTiming: &timingInfo, sampleBufferOut: &sampleBuffer)
        if let sb = sampleBuffer {
            displayLayer.enqueue(sb)
        }
    }

    func startIfNeeded() {
        pipController?.startPictureInPicture()
    }

    func stop() {
        pipController?.stopPictureInPicture()
    }

    func pictureInPictureControllerDidStopPictureInPicture(_ controller: AVPictureInPictureController) {
        // PiP closed — keep call active (audio only)
    }
}

extension PiPManager: AVPictureInPictureSampleBufferPlaybackDelegate {
    func pictureInPictureController(_ controller: AVPictureInPictureController, setPlaying playing: Bool) {}
    func pictureInPictureControllerTimeRangeForPlayback(_ controller: AVPictureInPictureController) -> CMTimeRange {
        CMTimeRange(start: .negativeInfinity, duration: .positiveInfinity)
    }
    func pictureInPictureControllerIsPlaybackPaused(_ controller: AVPictureInPictureController) -> Bool { false }
    func pictureInPictureController(_ controller: AVPictureInPictureController, didTransitionToRenderSize newRenderSize: CMVideoDimensions) {}
    func pictureInPictureController(_ controller: AVPictureInPictureController, skipByInterval skipInterval: CMTime, completion completionHandler: @escaping () -> Void) { completionHandler() }
}
```

**Step 3: Wire PiP to scene lifecycle**

In the app's scene delegate or `CallView`, detect `scenePhase == .background` → `PiPManager.shared.startIfNeeded()`. On foreground → `PiPManager.shared.stop()`.

Feed active speaker video frames to PiPManager from the video callback.

**Step 4: Commit**

```bash
git add ios/VisioMobile/Services/PiPManager.swift ios/VisioMobile/Info.plist
git commit -m "feat(ios): PiP with AVSampleBufferDisplayLayer for background video"
```

---

### Task 4.8: iOS App Title

**Files:**
- Modify: `ios/VisioMobile/Info.plist`
- Modify: `ios/VisioMobile/Views/HomeView.swift`

**Step 1: Set CFBundleDisplayName to "Visio Mobile"**

**Step 2: Update HomeView header text**

Replace `Text("Visio")` with `Text("Visio Mobile")`.

**Step 3: Commit**

```bash
git add ios/VisioMobile/Info.plist ios/VisioMobile/Views/HomeView.swift
git commit -m "feat(ios): set app title to Visio Mobile"
```

---

## Phase 5: App Icon

### Task 5.1: Bleu-Blanc-Rouge Icon

**Files:**
- Create: `assets/icon-source.svg` — master SVG
- Update: `crates/visio-desktop/icons/icon.png`
- Create: Android adaptive icon resources
- Update: iOS asset catalog AppIcon

**Step 1: Create SVG source**

Design a video camera icon on bleu République (#000091) background with white symbol and rouge République (#E1000F) accent. Export as SVG.

**Step 2: Generate platform assets**

- Desktop: 512x512 PNG, 128x128, 64x64, 32x32
- Android: `ic_launcher.xml` (adaptive icon with foreground/background layers), `mipmap-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/ic_launcher.png`
- iOS: AppIcon set in asset catalog (1024x1024, 180x180, 120x120, 87x87, 80x80, 76x76, 60x60, 58x58, 40x40, 29x29, 20x20)

**Step 3: Commit**

```bash
git add assets/ crates/visio-desktop/icons/ android/app/src/main/res/mipmap-*/ ios/VisioMobile/Assets.xcassets/AppIcon.appiconset/
git commit -m "feat: add bleu-blanc-rouge app icon across all platforms"
```

---

## Summary

| Phase | Tasks | Estimated commits |
|-------|-------|-------------------|
| 1. Rust Core | 1.1–1.5 | 5 |
| 2. Desktop | 2.1–2.9 | 9 |
| 3. Android | 3.1–3.7 | 7 |
| 4. iOS | 4.1–4.8 | 8 |
| 5. Icon | 5.1 | 1 |
| **Total** | **30 tasks** | **30 commits** |
