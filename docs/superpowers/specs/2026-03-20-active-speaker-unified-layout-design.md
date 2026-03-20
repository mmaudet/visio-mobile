# Active Speaker & Unified Layout Design

**Goal:** Unify the speaker focus logic and tile layout across all adaptive modes (Office, Pedestrian, Car) and all platforms (Android, iOS, Desktop), with visible pin support and speaker stabilization.

**Date:** 2026-03-20

---

## 1. Speaker Focus Rules

### Who sees what

When participant X speaks:
- **Everyone else** sees X in the main tile (large)
- **X (the speaker)** sees the last remote speaker in the main tile — not themselves

This is a mobile-friendly variant of Meet's behavior (Meet shows the speaker to everyone including themselves).

### Activation threshold

Focus mode activates **from 2 participants** onwards. In a 1-to-1 call, the remote participant is always in the main tile.

### Stabilization

Two mechanisms prevent distracting "ping-pong" when multiple people speak:

1. **Minimum hold time (2-3 seconds):** The focus stays on the current speaker for at least 2-3 seconds before switching, even if someone else starts talking.
2. **Volume threshold:** A new speaker must have a significantly higher audio level than the current speaker to take over focus. Brief interjections ("mhm", "ok") don't steal focus.

Both conditions must be met to switch: enough time elapsed AND sufficient volume difference.

### Silence behavior

After ~5 seconds with no active speaker:
- **Office mode:** Transition back to the equal grid layout (animated resize, ~300ms)
- **Pedestrian mode:** Stay on the last speaker (no grid on mobile — one tile is the only view)
- **Car mode:** Stay on the last speaker name display

The silence-to-grid transition does NOT apply when a pin is active.

---

## 2. Priority Hierarchy

```
Screen share > Pin (manual) > Auto-focus (speaker) > Grid (silence)
```

| State | Main tile shows | Overrides |
|---|---|---|
| Screen share active | Screen share content | Everything |
| User pinned someone | Pinned participant | Auto-focus and grid |
| Someone is speaking | Active speaker (stabilized) | Grid |
| Silence (no speaker) | Grid layout (Office) / Last speaker (Pedestrian/Car) | Nothing |

---

## 3. Pin Manual

### Current state

Pin is implemented on all 3 platforms but **invisible on mobile**:
- Android/iOS: triggered by tap (conflicts with show/hide controls)
- Desktop: click + pushpin icon in toolbar
- Adaptive modes (Pedestrian, Car): no pin support at all

### New behavior

**Gesture:**
- **Mobile (all modes except Car):** Long press on a tile to pin. Long press again or tap the pin icon to unpin.
- **Car mode:** No pin (audio-only, no video to pin)
- **Desktop:** Click on a tile (unchanged) + pushpin icon visible

**Visual indicator:**
- Pin icon overlay (pushpin) in the top-right corner of the pinned tile
- Visible in all modes: Office grid, Office focus, Pedestrian

**Unpin:**
- Mobile: long press on pinned tile, or tap the pin icon overlay
- Desktop: click the unpin button in the focus toolbar

**Behavior when pinned:**
- The pinned participant stays in the main tile regardless of who speaks
- A blue border (speaker indicator) shows on the active speaker's thumbnail so the user knows who is talking
- Silence-to-grid does NOT trigger while a pin is active
- Screen share still overrides the pin (priority hierarchy)

---

## 4. Unified Layout Engine

### Problem

Today, each adaptive mode has its own inline layout logic in CallScreen:
- Office: `buildDisplayItems()` + grid/focus with carousel (~200 lines)
- Pedestrian: ad-hoc inline (~60 lines)
- Car: ad-hoc inline (~40 lines)

This causes inconsistencies in speaker detection, pin support, and transition behavior.

### Solution: Single layout engine

A pure function that takes inputs and produces a layout decision:

**Inputs:**
- `participants: List<ParticipantInfo>`
- `activeSpeakers: List<String>` (SIDs ordered by volume)
- `pinnedItem: FocusItem?` (user pin, or null)
- `screenShare: FocusItem?` (active screen share, or null)
- `adaptiveMode: AdaptiveMode` (Office / Pedestrian / Car)
- `localParticipantSid: String`
- `timeSinceLastSpeaker: Duration`
- `currentFocus: FocusItem?` (for stabilization — who is currently focused)
- `focusHoldStartTime: Instant?` (when current focus was set — for minimum hold)

**Output:**
```
LayoutDecision {
    mode: Grid | Focus
    mainTile: ParticipantInfo?       // who goes in the big tile (null = grid)
    mainTileTrackSid: String?        // which track to show
    secondaryTiles: List<DisplayItem> // carousel / thumbnails
    speakerIndicatorSid: String?     // who has the blue border (active speaker)
    pinnedIndicatorSid: String?      // who has the pin icon
}
```

### Adaptation by mode

The layout engine produces the same `LayoutDecision` for all modes. The **rendering** differs:

| | Main tile | Secondary tiles | Controls |
|---|---|---|---|
| **Office** | 70% of screen | Horizontal carousel below | Full control bar |
| **Pedestrian** | Fullscreen | None (or 1 small overlay) | XXL buttons |
| **Car** | Fullscreen, avatar + name (no video) | None | Audio-only, XXL buttons |

### Speaker selection logic (inside the engine)

```
1. If screenShare is active → mainTile = screenShare participant
2. Else if pinnedItem is set → mainTile = pinned participant
3. Else if activeSpeaker exists:
   a. If stabilization allows switch (hold time elapsed AND volume threshold met):
      - If speaker is local → mainTile = last remote speaker (don't show self)
      - Else → mainTile = speaker
   b. Else → mainTile = currentFocus (keep current, don't switch yet)
4. Else if timeSinceLastSpeaker > 5s AND no pin:
   - Office → mode = Grid, mainTile = null
   - Pedestrian/Car → mainTile = last speaker (keep showing)
5. Else → mainTile = currentFocus (maintain current state)
```

---

## 5. Visual Indicators

### Speaker indicator
- **Blue border** (solid, 2px) around the active speaker's tile
- Applied in both grid mode and carousel thumbnails
- Style matches Meet web client — simple, no animation

### Pin indicator
- **Pushpin icon** (filled) in the top-right corner of the pinned tile
- Semi-transparent background for readability
- Visible in main tile and in thumbnails

### Transitions
- **Speaker switch:** Crossfade (200ms) with stabilization delay
- **Focus → Grid** (silence): Animated resize (~300ms)
- **Grid → Focus** (someone speaks): Immediate on first speaker

---

## 6. Platform-specific notes

### Android
- Replace current tap-to-pin with long press (tap keeps show/hide controls)
- Unify the 3 `when (adaptiveMode)` branches in CallScreen into a single layout engine call + mode-specific renderer
- The `userPinnedItem` state remains but is now set via long press

### iOS
- Same long press change for pin
- Unify the mode-specific branches in CallView
- `userPinned: Bool` + `focusedItem` remain but are driven by the engine

### Desktop
- Pin via click is unchanged (no long press needed — no conflict with controls)
- Pushpin/unpin icons already exist (`RiPushpinLine`, `RiUnpinFill`)
- Unify the focus logic with the same engine (TypeScript equivalent)

---

## 7. Files involved

| File | Changes |
|---|---|
| `android/.../ui/CallScreen.kt` | Extract layout engine, unify mode branches, long press pin |
| `ios/.../Views/CallView.swift` | Same refactor as Android |
| `crates/visio-desktop/frontend/src/App.tsx` | Unify focus logic, add speaker stabilization |
| New: `android/.../ui/LayoutEngine.kt` | Pure layout decision function |
| New: `ios/.../Services/LayoutEngine.swift` | Same for iOS |
| New: `crates/visio-desktop/frontend/src/layout-engine.ts` | Same for Desktop |

---

## 8. Out of scope

- Grid layout algorithm (number of columns/rows) — unchanged
- Screen share display — unchanged (already has absolute priority)
- Adaptive mode detection (motion, bluetooth) — unchanged
- Audio routing — unchanged
