# UX/UI Overhaul Design — Visio Mobile v2

**Date:** 2026-03-01
**Scope:** Desktop (Tauri/React), Android (Kotlin/Compose), iOS (SwiftUI)
**Reference:** LaSuite Meet ([suitenumerique/meet](https://github.com/suitenumerique/meet))
**Approach:** Platform-by-platform (Rust core → Desktop → Android → iOS)

---

## 1. Icons — Remixicon

Shared icon set across all 3 platforms using [Remixicon](https://remixicon.com) (open-source, MIT).

### Icon inventory

| Icon | Usage |
|------|-------|
| `RiMicLine` / `RiMicOffLine` | Mic on/off (control bar) |
| `RiVideoOnLine` / `RiVideoOffLine` | Camera on/off (control bar) |
| `RiArrowUpSLine` | Chevron device picker |
| `RiCameraSwitchLine` | Camera front/back switch (mobile) |
| `RiHand` | Hand raise (control bar + tile overlay) |
| `RiChat1Line` | Chat toggle (control bar) |
| `RiPhoneFill` | Hangup (control bar) |
| `RiMicFill` / `RiMicOffFill` | Mic state indicator (participant tile) |
| `RiSettings3Line` | Settings |
| `RiCloseLine` | Close modal/panel |
| `RiSendPlane2Fill` | Send message (chat input) |
| `RiArrowLeftSLine` | Back navigation |

### Integration per platform

- **Desktop:** `npm install @remixicon/react` — React components
- **Android:** SVG exports in `res/drawable/` — `painterResource(R.drawable.ri_mic_line)`
- **iOS:** SVG exports in Asset Catalog — `Image("ri-mic-line")`

---

## 2. Control Bar

### Layout per platform

**Desktop:** `[Mic][▾] [Cam][▾] [✋] [💬] [📞]` — 5 buttons + 2 chevrons

**Android/iOS:** `[Mic][▾] [Cam] [⟳] [✋] [💬] [📞]` — 6 buttons + 1 audio chevron

No camera switch on desktop (single webcam). No camera chevron on mobile (front/back switch instead). Audio chevron on mobile for Bluetooth/speaker selection.

### Button styling

| Button | Normal | Active/Disabled | Size |
|--------|--------|-----------------|------|
| Mic toggle | `primaryDark.100` (#2D2D46) | `error.200` (#6C302E) when muted | 44x44px |
| Mic chevron | `primaryDark.100`, grouped right | — | 28x44px |
| Camera toggle | same as mic | same | 44x44px |
| Camera chevron (desktop) | same | — | 28x44px |
| Camera switch (mobile) | `primaryDark.100` | — | 44x44px |
| Hand raise | `primaryDark.100` | `#fde047` (yellow) when raised | 44x44px |
| Chat | `primaryDark.100` | red badge if unread messages | 44x44px |
| Hangup | `error.500` (#EF413D) | hover `error.600` | 44x44px |

### Bar styling

- Background: `primaryDark.75` (#222234)
- Border radius: 16px
- Padding: 12px
- Gap: 8px between buttons
- Position: centered bottom, absolute

### Mic/Camera + Chevron grouping

Toggle and chevron visually joined (1px gap, inner border-radius removed). Chevron opens:
- **Desktop:** popover listing devices (mic input + speaker output)
- **Mobile:** bottom sheet listing audio sources (speaker, Bluetooth, AirPods, etc.)

### Device picker — Desktop popover

```
┌─────────────────────────┐
│ 🎤 Microphone            │
│ ○ MacBook Pro Mic        │
│ ● EarPods                │
│                          │
│ 🔊 Speaker               │
│ ○ MacBook Pro Speakers   │
│ ● EarPods                │
└─────────────────────────┘
```

Desktop: `navigator.mediaDevices.enumerateDevices()`

### Device picker — Mobile bottom sheet

```
┌─────────────────────────────────┐
│ Audio source                     │
│                                  │
│ ● iPhone Speaker                 │
│ ○ Car Bluetooth                  │
│ ○ AirPods Pro                    │
│                                  │
│         [ Close ]                │
└─────────────────────────────────┘
```

- **Android:** `AudioManager.getDevices(GET_DEVICES_OUTPUTS)` + `setCommunicationDevice()`. Compose `ModalBottomSheet`.
- **iOS:** `AVAudioSession.availableInputs` + `setPreferredInput()`. SwiftUI `.sheet(presentationDetents([.medium]))`.

---

## 3. Participant Tiles & Active Speaker

### Grid + Focus layout

**Grid mode** (default): all participants equal size, auto-adaptive grid.

| Participants | Desktop | Mobile |
|-------------|---------|--------|
| 1 | Full screen | Full screen |
| 2 | 2 columns | 2 rows stacked |
| 3-4 | 2x2 | 2x2 |
| 5-6 | 3x2 | 2x3 |
| 7+ | 3x3 + scroll | 2xN + scroll |

**Focus mode** (tap on a participant): selected participant full-size, others in horizontal scrollable strip at bottom. Tap on focused participant returns to grid.

### Tile anatomy

```
┌────────────────────────────┐
│                            │
│     [Video feed or         │
│      initials avatar]      │
│                            │
│  🔇  ✋        Alice  ●●●  │
└────────────────────────────┘
```

- **Metadata bar:** bottom of tile, `rgba(0,0,0,0.6)` background, 4px 8px padding, white text 12px
- **Avatar fallback** (no video): circle with initials, color derived from name hash, centered on `primaryDark.50` background
- **Mic muted:** `RiMicOffFill` icon in metadata bar
- **Hand raised:** `RiHand` icon on yellow `#fde047` pill with queue position number
- **Connection quality:** 3-bar indicator in metadata bar

### Active speaker highlight

- Border: `2px solid #6A6AF4` (primary.500)
- Box shadow: `0 0 12px rgba(106, 106, 244, 0.5)` — subtle glow
- Transition: `200ms ease-in-out`
- In focus mode: active speaker does NOT auto-switch — tap only (avoids jitter)
- Data source: `ActiveSpeakersChanged` event from Rust core (already exists)

### Hand raise on tiles

- Yellow `#fde047` pill with position number (1, 2, 3...)
- Wave animation on appear: rotation -20° → +20°, 300ms, 2 iterations

---

## 4. Hand Raise — Backend & UX

### Backend (visio-core)

**LiveKit mechanism:** participant attributes. Same pattern as LaSuite Meet — interoperable.

```
set_attributes({"handRaised": "1709312400"})   // raise (timestamp)
set_attributes({"handRaised": ""})              // lower
```

### New API

| Method/Event | Direction | Description |
|-------------|-----------|-------------|
| `raise_hand()` | UI → Core | Set attribute with timestamp |
| `lower_hand()` | UI → Core | Clear attribute |
| `is_hand_raised() → bool` | UI ← Core | Local state |
| `VisioEvent::HandRaisedChanged { participant_sid, raised, position }` | Core → UI | Remote/local hand state change |

### Queue

`BTreeMap<i64, String>` (timestamp → participant_sid) in core. Position computed from insertion order.

### Auto-lower

Core monitors `ActiveSpeakersChanged`: if local participant is active speaker for **3 consecutive seconds** AND has hand raised → automatic `lower_hand()`.

- Timer: `tokio::time::sleep(Duration::from_secs(3))`, resets if participant stops speaking before 3s
- Event `HandRaisedChanged { raised: false }` emitted normally
- UI shows toast: "Hand lowered automatically" (Desktop: in-app notification, Android: Snackbar, iOS: temporary overlay)

### Meet interop

Same attribute mechanism → Visio Mobile users see Meet users' raised hands and vice-versa.

---

## 5. Chat Panel

### Layout

- **Desktop:** right sidebar, 358px wide, slide-in animation. Video grid reduces width to accommodate.
- **Mobile:** full screen (push navigation or modal sheet).

### Changes from current state

| Aspect | Current | New |
|--------|---------|-----|
| Desktop | Full-screen modal | 358px sidebar, cohabits with video |
| Icon | Text "Chat" | `RiChat1Line` |
| Unread badge | None | Red dot on chat button |
| Send | Text button | `RiSendPlane2Fill` |
| Close | "Back" button | `RiCloseLine` top-right |
| Theme | Light background | Dark `primaryDark.50` |

### Unread badge

- Core Rust: `unread_count: AtomicU32`, incremented on `ChatMessageReceived` when chat panel is closed
- Reset to 0 when panel opens
- New event: `VisioEvent::UnreadCountChanged(u32)`
- UI: red dot (or number if ≤ 9)

### Message bubble

- Others' messages: left-aligned, `primaryDark.100` (#2D2D46) background
- Own messages: right-aligned, `primary.500` (#6A6AF4) background
- Sender name + timestamp: 12px caption, `greyscale.400`
- Consecutive messages from same author (<60s): no name repeat
- Border radius: 12px, padding: 8px 12px

---

## 6. Settings UI

### Access

Settings gear icon (`RiSettings3Line`) on Home screen, next to Join button.

### Content

4 existing settings from `SettingsStore`:

```
┌─────────────────────────────────┐
│ ← Settings                      │
│─────────────────────────────────│
│ Profile                         │
│ ┌─────────────────────────────┐ │
│ │ Display name                │ │
│ │ [  Matthieu              ]  │ │
│ └─────────────────────────────┘ │
│ Join meeting                    │
│ ┌─────────────────────────────┐ │
│ │ Mic enabled          [ON ] │ │
│ │ Camera enabled       [OFF] │ │
│ └─────────────────────────────┘ │
│ Language                        │
│ ┌─────────────────────────────┐ │
│ │ ○ Français                  │ │
│ │ ○ English                   │ │
│ └─────────────────────────────┘ │
│         [ Save ]                │
└─────────────────────────────────┘
```

### Implementation per platform

| Platform | Component | Navigation |
|----------|-----------|------------|
| Desktop | Modal dialog (React), `primaryDark.50` bg, max-width 480px | Gear button → modal |
| Android | Full Compose screen (`SettingsScreen`), Material 3 switches | `NavHost` navigation |
| iOS | SwiftUI `Form` with `Section`, native toggles | `NavigationLink` from Home |

### Persistence

Existing FFI methods: `set_display_name`, `set_language`, `set_mic_enabled_on_join`, `set_camera_enabled_on_join`. No new backend.

---

## 7. iOS — CallKit & Picture-in-Picture

### CallKit

**File:** `ios/VisioMobile/Services/CallKitManager.swift` (~150 lines)

**Architecture:**
- `CXProvider` receives system actions (hold, mute, end)
- `CXCallController` declares calls to the system
- `AVAudioSession` configured for `.playAndRecord` / `.voiceChat`

**Flow:**
1. `connect()` → `requestTransaction(.startCall)` → iOS shows call indicator (green bar / Dynamic Island)
2. Incoming phone call → `performSetHeldCallAction` → Visio auto-mutes mic
3. `disconnect()` → `requestTransaction(.endCall)` → indicator removed
4. Lock screen: native mute/hangup buttons → actions relayed to `VisioManager`

### iOS Picture-in-Picture

**File:** `ios/VisioMobile/Services/PiPManager.swift` (~120 lines)

**Mechanism:** `AVPictureInPictureController` on `AVSampleBufferDisplayLayer`. Rust core continues sending I420 frames of active speaker → Swift converts to `CVPixelBuffer` → `CMSampleBuffer` → PiP layer.

**Flow:**
1. App backgrounds (`scenePhase == .background`) AND call active → start PiP
2. PiP shows active speaker video (or last focused participant)
3. Tap PiP → return to app
4. Close PiP → audio-only (call continues, PiP closes)
5. App foregrounds → PiP closes, back to CallView

**Info.plist:** `UIBackgroundModes` → add `audio` and `voip`.

---

## 8. Android — Picture-in-Picture

**Mechanism:** Activity enters PiP mode directly. Compose UI continues rendering in reduced window.

**Flow:**
1. User presses Home during call → `onUserLeaveHint()` → `enterPictureInPictureMode()`
2. Activity shrinks to ~240x135dp floating window
3. `CallScreen` detects `isInPictureInPictureMode` → shows only active speaker tile, hides controls
4. Tap PiP → return to full screen
5. Close PiP → `finish()` triggers disconnect

**AndroidManifest.xml:** `android:supportsPictureInPicture="true"` + `android:configChanges="screenSize|smallestScreenSize|screenLayout|orientation"`

**Remote Actions** (buttons on PiP window):
- Mute/unmute → `PendingIntent` → `BroadcastReceiver` → `VisioManager.toggleMic()`
- Hangup → `PendingIntent` → disconnect + finish

---

## 9. Dark Theme — Meet Palette

| Token | Hex | Usage |
|-------|-----|-------|
| `primaryDark.50` | `#161622` | Main background |
| `primaryDark.75` | `#222234` | Control bar, chat sidebar |
| `primaryDark.100` | `#2D2D46` | Buttons, cards |
| `primaryDark.300` | `#5A5A8F` | Button hover |
| `primary.500` | `#6A6AF4` | Accent (active speaker glow, own chat bubbles) |
| `greyscale.400` | `#929292` | Secondary text, timestamps |
| `error.500` | `#EF413D` | Hangup button, mic muted |
| `error.200` | `#6C302E` | Mic off button background |
| `handRaise` | `#fde047` | Hand raise yellow |
| `white` | `#FFFFFF` | Primary text, icons |

### Per platform

- **Desktop:** CSS variables `--color-primary-dark-50` etc. in `App.css`
- **Android:** `Color(0xFF161622)` in `Theme.kt` / `Colors.kt`
- **iOS:** `Color(hex: 0x161622)` in `Theme.swift` or Asset Catalog named colors

---

## 10. App Identity

### Title

"Visio Mobile" everywhere in UI (Home screen header, window title).

- Desktop window title: "Visio Mobile"
- Android `app_name`: "Visio Mobile"
- iOS `CFBundleDisplayName`: "Visio Mobile"

### App icon — Bleu-blanc-rouge

Tricolor variant inspired by LaSuite Visio logo:
- Background: bleu République `#000091`
- Video symbol: white
- Accent: rouge République `#E1000F`
- Shape: iOS superellipse, Android adaptive icon
- Formats: SVG source → PNG exports for all densities

---

## Implementation Order (Approach A)

1. **Rust core** — hand raise (attributes + auto-lower), unread count event, expose active speaker/connection quality via FFI
2. **Desktop (React/Tauri)** — Remixicon, control bar, device picker popovers, grid+focus layout, active speaker glow, chat sidebar, settings modal, dark theme
3. **Android (Kotlin/Compose)** — Remixicon SVG drawables, control bar + audio bottom sheet, grid+focus layout, tiles, chat, settings screen, PiP, dark theme
4. **iOS (SwiftUI)** — Remixicon SVG assets, control bar + audio bottom sheet, grid+focus layout, tiles, chat, settings, CallKit, PiP, dark theme
5. **App identity** — icon bleu-blanc-rouge, title "Visio" across all platforms
