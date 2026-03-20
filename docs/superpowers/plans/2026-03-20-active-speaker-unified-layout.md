# Active Speaker & Unified Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify speaker focus logic and tile layout across all adaptive modes and platforms, with speaker stabilization, visible pin support, and consistent behavior matching the design spec.

**Architecture:** Extract a pure layout decision function (LayoutEngine) per platform that takes participants, speakers, pin state, and mode as input and outputs which tiles to show where. Each platform renders the LayoutDecision differently based on adaptive mode, but the selection logic is shared. Pin becomes visible (pushpin icon) and uses long press on mobile.

**Tech Stack:** Kotlin/Compose (Android), Swift/SwiftUI (iOS), TypeScript/React (Desktop), Rust (visio-core for shared types if needed)

**Spec:** `docs/superpowers/specs/2026-03-20-active-speaker-unified-layout-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `android/.../ui/LayoutEngine.kt` | Create | Pure layout decision function for Android |
| `android/.../ui/CallScreen.kt` | Modify | Replace 3 mode branches with LayoutEngine + renderers, long press pin |
| `ios/.../Services/LayoutEngine.swift` | Create | Pure layout decision function for iOS |
| `ios/.../Views/CallView.swift` | Modify | Replace mode branches with LayoutEngine + renderers, long press pin |
| `crates/visio-desktop/frontend/src/layout-engine.ts` | Create | Pure layout decision function for Desktop |
| `crates/visio-desktop/frontend/src/App.tsx` | Modify | Replace focus logic with layout-engine, add stabilization |

---

### Task 1: Android — Create LayoutEngine

**Files:**
- Create: `android/app/src/main/kotlin/io/visio/mobile/ui/LayoutEngine.kt`

- [ ] **Step 1: Create LayoutEngine data classes and function signature**

```kotlin
package io.visio.mobile.ui

import uniffi.visio.AdaptiveMode
import uniffi.visio.ParticipantInfo

data class FocusItem(val participantSid: String, val source: String)

enum class LayoutMode { GRID, FOCUS }

data class LayoutDecision(
    val mode: LayoutMode,
    val mainTile: DisplayItem?,
    val secondaryTiles: List<DisplayItem>,
    val speakerIndicatorSid: String?,
    val pinnedIndicatorSid: String?,
)

data class LayoutState(
    val currentFocus: FocusItem?,
    val focusHoldStartMs: Long?,
    val lastRemoteSpeakerSid: String?,
)

private const val MIN_HOLD_MS = 2500L // 2.5 seconds
private const val SILENCE_TO_GRID_MS = 5000L // 5 seconds

fun computeLayout(
    participants: List<ParticipantInfo>,
    activeSpeakers: List<String>,
    pinnedItem: FocusItem?,
    screenShare: FocusItem?,
    adaptiveMode: AdaptiveMode,
    localParticipantSid: String,
    previousState: LayoutState,
    nowMs: Long,
): Pair<LayoutDecision, LayoutState> {
    val displayItems = buildDisplayItems(participants)

    // 1. Screen share has absolute priority
    if (screenShare != null) {
        val main = displayItems.find {
            it.participant.sid == screenShare.participantSid && it.source == screenShare.source
        }
        val secondary = displayItems.filter { it.key != main?.key }
        val speakerSid = activeSpeakers.firstOrNull()
        return Pair(
            LayoutDecision(LayoutMode.FOCUS, main, secondary, speakerSid, pinnedItem?.participantSid),
            previousState.copy(currentFocus = screenShare),
        )
    }

    // 2. Pin has priority over auto-focus
    if (pinnedItem != null) {
        val main = displayItems.find {
            it.participant.sid == pinnedItem.participantSid && it.source == pinnedItem.source
        }
        val secondary = displayItems.filter { it.key != main?.key }
        val speakerSid = activeSpeakers.firstOrNull()
        return Pair(
            LayoutDecision(LayoutMode.FOCUS, main, secondary, speakerSid, pinnedItem.participantSid),
            previousState.copy(currentFocus = pinnedItem),
        )
    }

    // 3. Active speaker logic with stabilization
    val currentSpeakerSid = activeSpeakers.firstOrNull()
    val isLocalSpeaking = currentSpeakerSid == localParticipantSid

    if (currentSpeakerSid != null) {
        val newLastRemote = if (!isLocalSpeaking) currentSpeakerSid else previousState.lastRemoteSpeakerSid

        // Determine who to show in main tile
        val targetSid = if (isLocalSpeaking) {
            // Speaker sees last remote speaker, not themselves
            previousState.lastRemoteSpeakerSid ?: participants.drop(1).firstOrNull()?.sid
        } else {
            currentSpeakerSid
        }

        if (targetSid != null) {
            val targetFocus = FocusItem(targetSid, "camera")

            // Stabilization: check if we should switch
            val shouldSwitch = if (previousState.currentFocus == null) {
                true // No current focus, switch immediately
            } else if (previousState.currentFocus == targetFocus) {
                false // Same target, no switch needed
            } else {
                // Check minimum hold time
                val holdElapsed = previousState.focusHoldStartMs?.let { nowMs - it } ?: Long.MAX_VALUE
                holdElapsed >= MIN_HOLD_MS
            }

            if (shouldSwitch) {
                val main = displayItems.find { it.participant.sid == targetSid && it.source == "camera" }
                val secondary = displayItems.filter { it.key != main?.key }
                return Pair(
                    LayoutDecision(LayoutMode.FOCUS, main, secondary, currentSpeakerSid, null),
                    LayoutState(targetFocus, nowMs, newLastRemote),
                )
            } else {
                // Keep current focus (stabilization)
                val currentMain = displayItems.find {
                    it.participant.sid == previousState.currentFocus?.participantSid
                        && it.source == previousState.currentFocus?.source
                }
                val secondary = displayItems.filter { it.key != currentMain?.key }
                return Pair(
                    LayoutDecision(LayoutMode.FOCUS, currentMain, secondary, currentSpeakerSid, null),
                    previousState.copy(lastRemoteSpeakerSid = newLastRemote),
                )
            }
        }
    }

    // 4. No speaker — check silence timeout
    val silenceElapsed = previousState.focusHoldStartMs?.let { nowMs - it } ?: Long.MAX_VALUE
    if (silenceElapsed > SILENCE_TO_GRID_MS && adaptiveMode == AdaptiveMode.OFFICE) {
        return Pair(
            LayoutDecision(LayoutMode.GRID, null, displayItems, null, null),
            previousState.copy(currentFocus = null, focusHoldStartMs = null),
        )
    }

    // Keep current state (Pedestrian/Car stay on last speaker, or Office within silence window)
    val currentMain = previousState.currentFocus?.let { focus ->
        displayItems.find { it.participant.sid == focus.participantSid && it.source == focus.source }
    }
    if (currentMain != null) {
        val secondary = displayItems.filter { it.key != currentMain.key }
        return Pair(
            LayoutDecision(LayoutMode.FOCUS, currentMain, secondary, null, null),
            previousState,
        )
    }

    // Default: grid
    return Pair(
        LayoutDecision(LayoutMode.GRID, null, displayItems, null, null),
        previousState.copy(currentFocus = null),
    )
}
```

- [ ] **Step 2: Verify ktlint**

Run: `cd android && ./gradlew ktlintMainSourceSetCheck`

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/LayoutEngine.kt
git commit -m "feat(android): add LayoutEngine for unified speaker focus logic"
```

---

### Task 2: Android — Integrate LayoutEngine into CallScreen

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt`

- [ ] **Step 1: Replace the 3 `when (adaptiveMode)` branches**

In CallScreen composable:
- Add `LayoutState` as a `remember` state
- Replace the `LaunchedEffect(activeSpeakers, ...)` auto-focus block with a call to `computeLayout()`
- Replace the `when (adaptiveMode) { OFFICE -> ..., PEDESTRIAN -> ..., CAR -> ... }` with:
  - Compute `LayoutDecision` from `computeLayout()`
  - Render based on `adaptiveMode` + `LayoutDecision`:
    - Office + GRID → existing grid renderer
    - Office + FOCUS → existing focus renderer (main + carousel)
    - Pedestrian + FOCUS → fullscreen single tile
    - Car + FOCUS → avatar + name display
- Pass `decision.speakerIndicatorSid` to tiles for blue border
- Pass `decision.pinnedIndicatorSid` to tiles for pin icon

- [ ] **Step 2: Add long press for pin**

Replace `onClick` on tiles with:
```kotlin
Modifier
    .clickable(onClick = onClick) // show/hide controls
    .pointerInput(Unit) {
        detectTapGestures(
            onLongPress = {
                // Toggle pin
                if (userPinnedItem?.participantSid == item.participant.sid) {
                    userPinnedItem = null
                } else {
                    userPinnedItem = FocusItem(item.participant.sid, item.source)
                }
            }
        )
    }
```

- [ ] **Step 3: Add pin icon overlay to ParticipantTile**

In `ParticipantTile` composable, add a parameter `isPinned: Boolean` and render a pushpin icon overlay in the top-right corner when true.

- [ ] **Step 4: Add speaker border indicator**

In `ParticipantTile`, add a parameter `isActiveSpeaker: Boolean` (already exists) and ensure the blue border is rendered in all modes (grid, focus carousel, pedestrian).

- [ ] **Step 5: Verify ktlint and test**

Run: `cd android && ./gradlew ktlintMainSourceSetCheck`
Manual test: join a room, verify speaker focus, pin, mode transitions.

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt
git commit -m "feat(android): integrate LayoutEngine, long press pin, speaker indicators"
```

---

### Task 3: iOS — Create LayoutEngine

**Files:**
- Create: `ios/VisioMobile/Services/LayoutEngine.swift`

- [ ] **Step 1: Create LayoutEngine**

Port the same logic from the Android LayoutEngine to Swift. Same data structures (`LayoutDecision`, `LayoutState`), same `computeLayout()` function, same stabilization constants.

Key differences from Kotlin:
- Use `struct` instead of `data class`
- Use `enum LayoutMode { case grid, focus }`
- Use `TimeInterval` for durations
- Use `Date().timeIntervalSince1970 * 1000` for timestamps

- [ ] **Step 2: Commit**

```bash
git add ios/VisioMobile/Services/LayoutEngine.swift
git commit -m "feat(ios): add LayoutEngine for unified speaker focus logic"
```

---

### Task 4: iOS — Integrate LayoutEngine into CallView

**Files:**
- Modify: `ios/VisioMobile/Views/CallView.swift`

- [ ] **Step 1: Replace mode-specific branches with LayoutEngine**

Same approach as Android:
- Add `@State var layoutState = LayoutState()`
- Replace the `switch effectiveAdaptiveMode` branches with `computeLayout()` + mode-specific renderers
- Replace `onChange(of: activeSpeakers)` auto-focus block

- [ ] **Step 2: Add long press for pin**

Replace tap gesture on tiles with `.onLongPressGesture` for pin toggle.

- [ ] **Step 3: Add pin icon and speaker border**

Add pin icon overlay and ensure blue border for active speaker is visible in all modes.

- [ ] **Step 4: Commit**

```bash
git add ios/VisioMobile/Views/CallView.swift
git commit -m "feat(ios): integrate LayoutEngine, long press pin, speaker indicators"
```

---

### Task 5: Desktop — Create layout-engine.ts

**Files:**
- Create: `crates/visio-desktop/frontend/src/layout-engine.ts`

- [ ] **Step 1: Create TypeScript LayoutEngine**

Port the same logic. TypeScript equivalent of the Kotlin/Swift versions.

```typescript
export interface FocusItem { participantSid: string; source: string; }
export type LayoutMode = "grid" | "focus";
export interface LayoutDecision {
  mode: LayoutMode;
  mainTile: DisplayItem | null;
  secondaryTiles: DisplayItem[];
  speakerIndicatorSid: string | null;
  pinnedIndicatorSid: string | null;
}
export interface LayoutState {
  currentFocus: FocusItem | null;
  focusHoldStartMs: number | null;
  lastRemoteSpeakerSid: string | null;
}

export function computeLayout(...): [LayoutDecision, LayoutState] { ... }
```

- [ ] **Step 2: Commit**

```bash
git add crates/visio-desktop/frontend/src/layout-engine.ts
git commit -m "feat(desktop): add layout-engine for unified speaker focus logic"
```

---

### Task 6: Desktop — Integrate layout-engine into App.tsx

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

- [ ] **Step 1: Replace focus logic with layout-engine**

- Import `computeLayout` from `layout-engine.ts`
- Replace `useEffect` auto-focus logic (~lines 1452-1470) with `computeLayout()` call
- Use `LayoutDecision` to drive the grid/focus rendering
- Add `layoutState` as `useRef`

- [ ] **Step 2: Add speaker border and pin icon**

- Pass `speakerIndicatorSid` to tile rendering for blue border CSS
- Pushpin icons already exist (`RiPushpinLine`, `RiUnpinFill`) — ensure they're shown on the pinned tile

- [ ] **Step 3: TypeScript check**

Run: `cd crates/visio-desktop/frontend && npx tsc --noEmit`

- [ ] **Step 4: Commit**

```bash
git add crates/visio-desktop/frontend/src/layout-engine.ts crates/visio-desktop/frontend/src/App.tsx
git commit -m "feat(desktop): integrate layout-engine, speaker indicators"
```

---

### Task 7: Cross-platform testing

- [ ] **Step 1: Build all platforms**

```bash
cargo test -p visio-core --lib
cd android && ./gradlew ktlintMainSourceSetCheck
cd crates/visio-desktop/frontend && npx tsc --noEmit
```

- [ ] **Step 2: Manual test matrix**

| Scenario | Expected |
|---|---|
| 2 participants, remote speaks | Remote in main tile |
| 2 participants, I speak | Remote stays in main tile (don't show self) |
| 3+ participants, speaker changes | Focus switches after 2.5s stabilization |
| Rapid speaker changes | No ping-pong (stabilization holds) |
| Silence > 5s (Office) | Return to grid |
| Silence > 5s (Pedestrian) | Stay on last speaker |
| Long press tile (mobile) | Pin icon appears, focus locked |
| Long press again | Unpin, return to auto-focus |
| Pin active + someone speaks | Pin holds, blue border on speaker thumbnail |
| Screen share starts | Overrides everything |
| Screen share ends | Return to previous state (pin or auto-focus) |

- [ ] **Step 3: Create PR**

```bash
git push -u origin feat/unified-speaker-layout
gh pr create --title "feat: unified speaker focus & layout engine (all platforms)"
```
