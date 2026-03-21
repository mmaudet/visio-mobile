# E2E Test Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a professional E2E test framework that orchestrates multiple participants (bots, Desktop, Android, iOS sim), validates UI state via semantic test tags, captures screenshot evidence, and produces HTML/JSON reports. First milestone: automate the PR #71 speaker layout test plan.

**Architecture:** TypeScript orchestrator spawns `visio-bot` child processes (Rust, interactive stdin/stdout protocol) as remote participants, drives Desktop via Playwright, Android via ADB+UIAutomator, and iOS sim via `xcrun simctl`. Scenarios are organized in suites (directories), discovered at runtime, and filtered by CLI args. Assertions use polling with timeout against semantic test tags (`testTag` on Android, `data-testid` on Desktop).

**Tech Stack:** TypeScript (tsx), Playwright, ADB, Rust (visio-bot), Kotlin/Compose (Android test tags), React/TSX (Desktop test tags), Swift/SwiftUI (iOS test tags)

**Spec:** `docs/superpowers/specs/2026-03-21-e2e-test-framework-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `e2e/visio-bot/src/main.rs` | Modify | Add `--interactive` mode with stdin commands and structured stdout events |
| `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt` | Modify | Add semantic `testTag` annotations for layout state |
| `android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt` | Modify | Add `testTagsAsResourceId` semantics at root |
| `crates/visio-desktop/frontend/src/App.tsx` | Modify | Add `data-testid` annotations for layout state |
| `ios/VisioMobile/Views/CallView.swift` | Modify | Add `accessibilityIdentifier` annotations for v2 readiness |
| `e2e/framework/cli.ts` | Create | CLI entry point: `e2e run` / `e2e list` |
| `e2e/framework/runner.ts` | Create | Suite discovery, scenario execution loop, ScenarioContext |
| `e2e/framework/orchestrator.ts` | Create | LiveKit Docker setup/teardown, participant lifecycle |
| `e2e/framework/participants/types.ts` | Create | Participant interfaces (Participant, BotParticipant, etc.) |
| `e2e/framework/participants/bot.ts` | Create | Spawn visio-bot, stdin/stdout protocol, event parsing |
| `e2e/framework/participants/desktop.ts` | Create | Playwright browser automation on localhost:5173 |
| `e2e/framework/participants/android.ts` | Create | ADB commands, UIAutomator dump, test tag assertions |
| `e2e/framework/participants/ios-sim.ts` | Create | xcrun simctl: deep link connect, screenshot |
| `e2e/framework/assertions/poll.ts` | Create | pollUntil(), assertion helpers with timeout |
| `e2e/framework/evidence/screenshot.ts` | Create | Per-platform screenshot capture, file naming |
| `e2e/framework/reporting/html.ts` | Create | HTML report generator with screenshot gallery |
| `e2e/framework/reporting/json.ts` | Create | results.json generator |
| `e2e/framework/reporting/terminal.ts` | Create | Live terminal output during execution |
| `e2e/framework/utils/livekit.ts` | Create | Docker start/stop, health check |
| `e2e/framework/utils/token.ts` | Create | JWT token generation (wraps visio-bot --token-only) |
| `e2e/framework/utils/adb.ts` | Create | ADB helpers: tap, longPress, dumpUi, screencap |
| `e2e/scenarios/speaker-focus/suite.json` | Create | Suite definition with bot/platform requirements |
| `e2e/scenarios/speaker-focus/01-remote-speaks.ts` | Create | Scenario: remote speaker → main tile |
| `e2e/scenarios/speaker-focus/02-local-speaks.ts` | Create | Scenario: local speaks → remote stays main |
| `e2e/scenarios/speaker-focus/03-stabilization.ts` | Create | Scenario: 2.5s stabilization hold |
| `e2e/scenarios/speaker-focus/04-rapid-changes.ts` | Create | Scenario: rapid speaker changes, no ping-pong |
| `e2e/scenarios/silence-behavior/suite.json` | Create | Suite definition |
| `e2e/scenarios/silence-behavior/01-office-grid-return.ts` | Create | Scenario: silence > 5s → grid in office mode |
| `e2e/scenarios/silence-behavior/02-pedestrian-stay.ts` | Create | Scenario: silence → stay on last speaker |
| `e2e/scenarios/pin-behavior/suite.json` | Create | Suite definition |
| `e2e/scenarios/pin-behavior/01-long-press-pin.ts` | Create | Scenario: long press → pin appears |
| `e2e/scenarios/pin-behavior/02-unpin.ts` | Create | Scenario: long press again → unpin |
| `e2e/scenarios/pin-behavior/03-pin-holds.ts` | Create | Scenario: pin holds despite speaker change |
| `e2e/scenarios/screen-share/suite.json` | Create | Suite definition |
| `e2e/scenarios/screen-share/01-override.ts` | Create | Scenario: screen share overrides focus |
| `e2e/scenarios/screen-share/02-end-restore.ts` | Create | Scenario: end screen share → restore |
| `e2e/package.json` | Create | Node.js deps: tsx, playwright |
| `e2e/tsconfig.json` | Create | TypeScript config |
| `e2e/README.md` | Create | Full documentation |

---

### Task 1: Bot interactive mode — stdin/stdout protocol

**Files:**
- Modify: `e2e/visio-bot/src/main.rs`

- [ ] **Step 1: Add `--interactive` flag to Args**

In the `Args` struct (~line 35), add:

```rust
/// Interactive mode: listen on stdin for commands, emit structured events on stdout.
/// Does NOT run the turn-based scenario. Audio/video tracks are published muted.
#[arg(long, default_value_t = false)]
interactive: bool,
```

- [ ] **Step 2: Add interactive stdin command handler function**

After the `generate_token()` function (~line 507), add:

```rust
/// Process a single stdin command in interactive mode.
/// Returns false if the bot should quit.
async fn handle_interactive_command(
    line: &str,
    controls: &visio_core::controls::MeetingControls,
    _rm: &RoomManager,
) -> bool {
    let cmd = line.trim();
    match cmd {
        "SPEAK" => {
            if let Err(e) = controls.set_microphone_enabled(true).await {
                tracing::error!("SPEAK failed: {e}");
            }
            println!("[ACK] SPEAK");
        }
        "MUTE" => {
            if let Err(e) = controls.set_microphone_enabled(false).await {
                tracing::error!("MUTE failed: {e}");
            }
            println!("[ACK] MUTE");
        }
        "VIDEO_ON" => {
            if let Err(e) = controls.set_camera_enabled(true).await {
                tracing::error!("VIDEO_ON failed: {e}");
            }
            println!("[ACK] VIDEO_ON");
        }
        "VIDEO_OFF" => {
            if let Err(e) = controls.set_camera_enabled(false).await {
                tracing::error!("VIDEO_OFF failed: {e}");
            }
            println!("[ACK] VIDEO_OFF");
        }
        "SCREEN_SHARE_START" => {
            match controls.publish_screen_share().await {
                Ok(source) => {
                    spawn_synthetic_video(source);
                    tracing::info!("Screen share started");
                }
                Err(e) => tracing::error!("SCREEN_SHARE_START failed: {e}"),
            }
            println!("[ACK] SCREEN_SHARE_START");
        }
        "SCREEN_SHARE_STOP" => {
            if let Err(e) = controls.stop_screen_share().await {
                tracing::error!("SCREEN_SHARE_STOP failed: {e}");
            }
            println!("[ACK] SCREEN_SHARE_STOP");
        }
        "QUIT" => {
            println!("[ACK] QUIT");
            return false;
        }
        "" => {} // ignore empty lines
        other => {
            tracing::warn!("Unknown command: {other}");
            println!("[ACK] UNKNOWN:{other}");
        }
    }
    true
}
```

- [ ] **Step 3: Update ActiveSpeakersChanged event to resolve identities**

In the `BotEventLogger::on_event` method, replace the `ActiveSpeakersChanged` handler (~line 443):

Note: The `participant_identities` map is populated by `TrackSubscribed` events. If `ActiveSpeakersChanged` fires before tracks are subscribed, identities will be unresolved — fall back to SIDs in that case. Also, the bot's own identity should be added to the map at connect time.

```rust
VisioEvent::ActiveSpeakersChanged(sids) => {
    let identities: Vec<String> = {
        let map = self.participant_identities.lock().unwrap();
        sids.iter()
            .map(|sid| map.get(sid).cloned().unwrap_or_else(|| sid.clone()))
            .collect()
    };
    let identity_str = identities.join(",");
    tracing::info!("[EVENT] ActiveSpeakers: {identity_str}");
}
```

- [ ] **Step 4: Add interactive mode branch in main()**

In the `main()` function, after the connect + publish section (~line 822), add an interactive mode branch. The key difference: in interactive mode, tracks are published but immediately muted, and stdin is read in a loop.

After `rm.connect_with_token(...)` succeeds (~line 822), add:

```rust
// Print connected message (always, for interactive mode parsing)
let my_sid = rm.local_participant_info().await.map(|p| p.sid).unwrap_or_default();
println!("[CONNECTED] identity={} sid={my_sid}", args.identity);

if args.interactive {
    // Interactive mode: publish tracks muted, then listen for stdin commands
    let controls = rm.controls();

    // Publish audio (muted initially)
    if args.audio {
        match controls.publish_microphone().await {
            Ok(source) => {
                spawn_synthetic_audio(source);
                controls.set_microphone_enabled(false).await.ok();
                tracing::info!("Audio track published (muted)");
            }
            Err(e) => tracing::warn!("Failed to publish mic: {e}"),
        }
    }

    // Publish video (muted initially)
    if args.video {
        match controls.publish_camera("720p").await {
            Ok(source) => {
                spawn_synthetic_video(source);
                controls.set_camera_enabled(false).await.ok();
                tracing::info!("Video track published (muted)");
            }
            Err(e) => tracing::warn!("Failed to publish camera: {e}"),
        }
    }

    tracing::info!("Interactive mode: waiting for stdin commands...");

    // Read stdin line by line
    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    use tokio::io::AsyncBufReadExt;
    let mut lines = stdin.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if !handle_interactive_command(&line, &controls, &rm).await {
            break;
        }
    }

    rm.disconnect().await;
    tracing::info!("Interactive session ended");
    return;
}
```

Move the existing non-interactive publish + wait logic into an `else` block or after the early return.

- [ ] **Step 5: Verify bot builds**

Run: `cargo build -p visio-bot 2>&1 | tail -5`

- [ ] **Step 6: Commit**

```bash
git add e2e/visio-bot/src/main.rs
git commit -m "feat(e2e): add interactive stdin/stdout mode to visio-bot"
```

---

### Task 2: Android — Add semantic test tags

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt`

- [ ] **Step 1: Enable testTagsAsResourceId at the root composable**

In `MainActivity.kt`, find the root `setContent` block and wrap the content with semantics:

```kotlin
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId

// In setContent:
VisioTheme {
    CompositionLocalProvider(...) {
        Box(modifier = Modifier.semantics { testTagsAsResourceId = true }) {
            // existing navigation / content
        }
    }
}
```

- [ ] **Step 2: Add layout mode test tag**

In `CallScreen.kt`, on the Box that wraps the video area (~line 685 after the layoutDecision computation), add:

```kotlin
Box(
    modifier =
        Modifier
            .weight(1f)
            .fillMaxWidth()
            .padding(if (isFullscreenFocus) 0.dp else 8.dp)
            .testTag("layout-mode:${if (layoutDecision.mode == LayoutMode.FOCUS) "FOCUS" else "GRID"}"),
) {
```

- [ ] **Step 3: Add adaptive mode test tag**

On the outer call Box (~line 661):

```kotlin
.testTag("adaptive-mode:${effectiveAdaptiveMode.name}")
```

- [ ] **Step 4: Add main tile test tag in FOCUS mode**

In the OFFICE FOCUS rendering block, on the main tile Box, add:

```kotlin
.testTag("main-tile:${focusedDisplayItem.participant.sid}")
```

- [ ] **Step 5: Add secondary tile test tags**

In the FOCUS thumbnail bar, on each thumbnail Box, add (using the `forEachIndexed` pattern):

```kotlin
thumbnailItems.forEachIndexed { index, item ->
    Box(
        modifier = Modifier
            .weight(1f)
            .fillMaxHeight()
            .clip(RoundedCornerShape(8.dp))
            .testTag("secondary-tile-$index:${item.participant.sid}"),
    ) {
```

(Replace `for (item in thumbnailItems)` with `thumbnailItems.forEachIndexed`)

- [ ] **Step 6: Add grid tile test tags**

In the GRID rendering block, add (wrapping the existing indexed loop):

```kotlin
.testTag("grid-tile-$idx:${item.participant.sid}")
```

- [ ] **Step 7: Add speaker border and pin indicator test tags to ParticipantTile**

In the `ParticipantTile` composable, add conditional test tags:

```kotlin
// After the borderMod computation:
val speakerTag = if (isActiveSpeaker && !isScreenShare) {
    Modifier.testTag("speaker-border:${participant.sid}")
} else Modifier

// After the isPinned check:
// Inside the pin indicator Box:
Modifier.testTag("pin-indicator:${participant.sid}")
```

- [ ] **Step 8: Add PEDESTRIAN main tile test tag**

In the PEDESTRIAN rendering block, add test tag to the main participant:

```kotlin
.testTag("main-tile:${mainParticipant.sid}")
```

- [ ] **Step 9: Run ktlint**

Run: `cd android && ./gradlew ktlintMainSourceSetFormat && ./gradlew ktlintMainSourceSetCheck`

- [ ] **Step 10: Commit**

```bash
git add android/
git commit -m "feat(android): add semantic test tags for E2E layout assertions"
```

---

### Task 3: Desktop — Add data-testid annotations

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

- [ ] **Step 1: Add layout-mode test id**

On the call-content div that wraps the focus/grid layout (~line 1616):

```tsx
<div className="call-content" data-testid={`layout-mode:${focusedDisplayItem ? "FOCUS" : "GRID"}`}>
```

- [ ] **Step 2: Add main-tile test id in focus mode**

On the focus-main div (~line 1618):

```tsx
<div className="focus-main" data-testid={`main-tile:${focusedDisplayItem.participant.sid}`}>
```

- [ ] **Step 3: Add secondary-tile test ids**

On each thumbnail in focus mode (~line 1639):

```tsx
<div key={d.key} className="tile" data-testid={`secondary-tile-${i}:${d.participant.sid}`} onClick={...}>
```

(Convert `.map((d) =>` to `.map((d, i) =>`)

- [ ] **Step 4: Add grid-tile test ids**

On each grid tile (~line 1658):

```tsx
<div key={d.key} data-testid={`grid-tile-${i}:${d.participant.sid}`} onClick={...}>
```

(Convert `.map((d) =>` to `.map((d, i) =>`)

- [ ] **Step 5: TypeScript check**

Run: `cd crates/visio-desktop/frontend && npx tsc --noEmit`

- [ ] **Step 6: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx
git commit -m "feat(desktop): add data-testid annotations for E2E layout assertions"
```

---

### Task 4: iOS — Add accessibility identifiers (v2 readiness)

**Files:**
- Modify: `ios/VisioMobile/Views/CallView.swift`

- [ ] **Step 1: Add layout-mode identifier in the unified layout section**

In the body where `computeLayout` is called (~line 154), add an accessibility identifier on the enclosing view:

```swift
.accessibilityIdentifier("layout-mode:\(layoutDecision.mode == .focus ? "FOCUS" : "GRID")")
```

- [ ] **Step 2: Add main-tile identifier in focus/pedestrian modes**

In `focusLayout` and `pedestrianSingleTile`, add to the main ParticipantTile:

```swift
.accessibilityIdentifier("main-tile:\(focused.participant.sid)")
```

- [ ] **Step 3: Commit**

```bash
git add ios/VisioMobile/Views/CallView.swift
git commit -m "feat(ios): add accessibility identifiers for future E2E assertions"
```

---

### Task 5: Framework — Package setup and types

**Files:**
- Create: `e2e/package.json`
- Create: `e2e/tsconfig.json`
- Create: `e2e/framework/participants/types.ts`

- [ ] **Step 1: Create package.json**

```json
{
  "name": "visio-e2e",
  "private": true,
  "type": "module",
  "scripts": {
    "e2e": "tsx framework/cli.ts",
    "e2e:list": "tsx framework/cli.ts list"
  },
  "dependencies": {
    "tsx": "^4.19.0",
    "playwright": "^1.50.0"
  }
}
```

- [ ] **Step 2: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "outDir": "dist",
    "rootDir": ".",
    "skipLibCheck": true
  },
  "include": ["framework/**/*.ts", "scenarios/**/*.ts"]
}
```

- [ ] **Step 3: Create types.ts**

```typescript
// e2e/framework/participants/types.ts

export interface Participant {
  readonly identity: string;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  screenshot(name: string): Promise<string>;
}

export interface BotParticipant extends Participant {
  readonly sid: string;
  speak(): Promise<void>;
  mute(): Promise<void>;
  videoOn(): Promise<void>;
  videoOff(): Promise<void>;
  screenShareStart(): Promise<void>;
  screenShareStop(): Promise<void>;
  waitForEvent(pattern: string | RegExp, timeout?: number): Promise<string>;
}

export interface AndroidParticipant extends Participant {
  assertTestTag(tag: string, options?: { timeout?: number }): Promise<void>;
  assertNotTestTag(tag: string, options?: { timeout?: number }): Promise<void>;
  tap(tag: string): Promise<void>;
  longPress(tag: string): Promise<void>;
  dumpUiTree(): Promise<string>;
}

export interface DesktopParticipant extends Participant {
  assertTestId(testId: string, options?: { timeout?: number }): Promise<void>;
  assertNotTestId(testId: string, options?: { timeout?: number }): Promise<void>;
  click(testId: string): Promise<void>;
}

export interface IosSimParticipant extends Participant {
  // v1: connect + screenshot only
}

export interface ScenarioContext {
  bot(identity: string): BotParticipant;
  android(): AndroidParticipant;
  desktop(): DesktopParticipant;
  ios(): IosSimParticipant;
  sleep(ms: number): Promise<void>;
  log(message: string): void;
  sidMap: ReadonlyMap<string, string>;
}

export type ScenarioFn = (ctx: ScenarioContext) => Promise<void>;

export interface SuiteConfig {
  name: string;
  description: string;
  bots: Array<{ identity: string; name: string }>;
  requires: {
    android?: boolean;
    desktop?: boolean;
    ios?: boolean;
  };
}

export interface AssertionResult {
  description: string;
  platform: string;
  status: "pass" | "fail";
  durationMs: number;
  error?: string;
}

export interface ScenarioResult {
  name: string;
  status: "pass" | "fail" | "skip";
  duration: number;
  assertions: AssertionResult[];
  screenshots: string[];
  error?: string;
}

export interface SuiteResult {
  name: string;
  status: "pass" | "fail" | "skip";
  scenarios: ScenarioResult[];
  skipReason?: string;
}

export interface RunResult {
  timestamp: string;
  duration: number;
  participants: {
    bots: string[];
    desktop: boolean;
    android: boolean;
    ios: boolean;
  };
  suites: SuiteResult[];
  summary: { pass: number; fail: number; skip: number; total: number };
}
```

- [ ] **Step 4: Install deps**

Run: `cd e2e && npm install`

- [ ] **Step 5: Commit**

```bash
git add e2e/package.json e2e/tsconfig.json e2e/framework/participants/types.ts
git commit -m "feat(e2e): add framework package setup and type definitions"
```

---

### Task 6: Framework — Utility modules (ADB, LiveKit, tokens, polling)

**Files:**
- Create: `e2e/framework/utils/adb.ts`
- Create: `e2e/framework/utils/livekit.ts`
- Create: `e2e/framework/utils/token.ts`
- Create: `e2e/framework/assertions/poll.ts`
- Create: `e2e/framework/evidence/screenshot.ts`

This task creates all the low-level utility modules. Each is a standalone module with no dependency on the others (except `adb.ts` is used by `screenshot.ts`).

- [ ] **Step 1: Create `adb.ts`** — ADB helpers for UIAutomator dump, tap, longPress, screencap. The `dumpUi()` function runs `adb shell uiautomator dump /dev/tty` and parses the XML to find elements by `resource-id` (which is where Compose testTags appear when `testTagsAsResourceId = true`). The `hasTestTag(tag)` function returns `true` if a node with `resource-id` matching the tag is found.

- [ ] **Step 2: Create `livekit.ts`** — Docker start/stop for `livekit/livekit-server --dev`. Uses `child_process.execSync` for Docker commands. Health check polls `http://localhost:7880` until responsive.

- [ ] **Step 3: Create `token.ts`** — Wraps `visio-bot --token-only` to generate JWT tokens. Spawns the bot binary and captures stdout.

- [ ] **Step 4: Create `poll.ts`** — `pollUntil(fn, { interval, timeout })` generic polling helper. Returns when `fn()` returns truthy or throws on timeout. Used by assertion methods.

- [ ] **Step 5: Create `screenshot.ts`** — Per-platform screenshot capture. Android: `adb exec-out screencap -p`. Desktop: Playwright `page.screenshot()`. iOS: `xcrun simctl io booted screenshot`. Files saved to the report's screenshots directory.

- [ ] **Step 6: Commit**

```bash
git add e2e/framework/utils/ e2e/framework/assertions/ e2e/framework/evidence/
git commit -m "feat(e2e): add utility modules (adb, livekit, token, polling, screenshot)"
```

---

### Task 7: Framework — Participant implementations

**Files:**
- Create: `e2e/framework/participants/bot.ts`
- Create: `e2e/framework/participants/desktop.ts`
- Create: `e2e/framework/participants/android.ts`
- Create: `e2e/framework/participants/ios-sim.ts`

- [ ] **Step 1: Create `bot.ts`** — Spawns `visio-bot --interactive --url <url> --token <token> --identity <id> --name <name>`. Pipes stdin/stdout. Parses `[CONNECTED]` to extract SID. `speak()` writes `SPEAK\n` to stdin and waits for `[ACK] SPEAK`. `waitForEvent(pattern)` buffers stdout lines and matches against the pattern. `screenshot()` is a no-op for bots (returns empty string).

- [ ] **Step 2: Create `desktop.ts`** — Launches Playwright browser on `http://localhost:5173`. The Vite dev server must be started separately (by the orchestrator). `connect()` navigates to the page with auto-connect params (`?livekit_url=...&token=...`). `assertTestId(id)` uses `page.locator([data-testid="${id}"])`. `screenshot()` uses `page.screenshot()`.

- [ ] **Step 3: Create `android.ts`** — Uses `adb.ts` helpers. `connect()` launches the app via deep link: `adb shell am start -a android.intent.action.VIEW -d "visio-test://connect?..."`. `assertTestTag(tag)` uses `pollUntil(() => adb.hasTestTag(tag), opts)`. `longPress(tag)` finds element coordinates via UIAutomator dump then `adb shell input swipe x y x y 1500`. `screenshot()` uses `adb exec-out screencap -p`.

- [ ] **Step 4: Create `ios-sim.ts`** — `connect()` uses `xcrun simctl openurl booted "visio-test://connect?..."`. `screenshot()` uses `xcrun simctl io booted screenshot <path>`. No assertion methods in v1.

- [ ] **Step 5: Commit**

```bash
git add e2e/framework/participants/
git commit -m "feat(e2e): add participant implementations (bot, desktop, android, ios-sim)"
```

---

### Task 8: Framework — Runner, orchestrator, and CLI

**Files:**
- Create: `e2e/framework/runner.ts`
- Create: `e2e/framework/orchestrator.ts`
- Create: `e2e/framework/cli.ts`
- Create: `e2e/framework/reporting/terminal.ts`
- Create: `e2e/framework/reporting/json.ts`
- Create: `e2e/framework/reporting/html.ts`

- [ ] **Step 1: Create `runner.ts`** — Discovers suites by scanning `e2e/scenarios/*/suite.json`. For each suite, checks requirements against available devices. For each scenario, creates a `ScenarioContext`, spawns required bots on-demand, executes the scenario function, collects results. The `ScenarioContext` implementation lazily connects bots/participants on first access.

- [ ] **Step 2: Create `orchestrator.ts`** — Top-level setup/teardown: starts LiveKit Docker, detects Android device (`adb devices`), detects iOS sim (`xcrun simctl list devices booted`), starts Vite dev server for Desktop, builds visio-bot if needed. Returns an availability map used by the runner.

- [ ] **Step 3: Create `terminal.ts`** — Live terminal output during execution. Prints `[suite] scenario ....... PASS/FAIL/SKIP (duration)` for each scenario. Prints summary at the end.

- [ ] **Step 4: Create `json.ts`** — Generates `results.json` from `RunResult`.

- [ ] **Step 5: Create `html.ts`** — Generates `report.html` with inline CSS, collapsible suites, assertion lists, screenshot thumbnails. No external dependencies.

- [ ] **Step 6: Create `cli.ts`** — Entry point. Parses args (`run [suites...]`, `list`). Calls orchestrator setup, then runner, then reporting. Environment variable overrides for timeouts.

- [ ] **Step 7: Verify `npx tsx e2e/framework/cli.ts list` works**

Run: `cd e2e && npx tsx framework/cli.ts list`

- [ ] **Step 8: Commit**

```bash
git add e2e/framework/
git commit -m "feat(e2e): add runner, orchestrator, CLI, and reporting modules"
```

---

### Task 9: Scenarios — speaker-focus suite

**Files:**
- Create: `e2e/scenarios/speaker-focus/suite.json`
- Create: `e2e/scenarios/speaker-focus/01-remote-speaks.ts`
- Create: `e2e/scenarios/speaker-focus/02-local-speaks.ts`
- Create: `e2e/scenarios/speaker-focus/03-stabilization.ts`
- Create: `e2e/scenarios/speaker-focus/04-rapid-changes.ts`

- [ ] **Step 1: Create suite.json**

```json
{
  "name": "speaker-focus",
  "description": "Active speaker focus and stabilization (PR #71)",
  "bots": [
    { "identity": "bot-a", "name": "Alice (Remote)" },
    { "identity": "bot-b", "name": "Bob (Remote)" },
    { "identity": "bot-c", "name": "Charlie (Remote)" }
  ],
  "requires": {
    "android": true,
    "desktop": true,
    "ios": false
  }
}
```

- [ ] **Step 2: Create 01-remote-speaks.ts**

Bot-A speaks → verify main-tile shows bot-A on Android and Desktop.

- [ ] **Step 3: Create 02-local-speaks.ts**

Bot-A mute, Desktop/Android local mic on → verify main-tile still shows bot-A (never focus self).

- [ ] **Step 4: Create 03-stabilization.ts**

Bot-A speaks 3s → bot-A mutes, bot-B speaks → verify main-tile holds bot-A for 2.5s before switching.

- [ ] **Step 5: Create 04-rapid-changes.ts**

Bot-A/B/C alternate speaking < 2.5s each → verify main-tile stays stable (no rapid changes).

- [ ] **Step 6: Commit**

```bash
git add e2e/scenarios/speaker-focus/
git commit -m "feat(e2e): add speaker-focus test suite (4 scenarios)"
```

---

### Task 10: Scenarios — silence-behavior, pin-behavior, screen-share suites

**Files:**
- Create: `e2e/scenarios/silence-behavior/suite.json`
- Create: `e2e/scenarios/silence-behavior/01-office-grid-return.ts`
- Create: `e2e/scenarios/silence-behavior/02-pedestrian-stay.ts`
- Create: `e2e/scenarios/pin-behavior/suite.json`
- Create: `e2e/scenarios/pin-behavior/01-long-press-pin.ts`
- Create: `e2e/scenarios/pin-behavior/02-unpin.ts`
- Create: `e2e/scenarios/pin-behavior/03-pin-holds.ts`
- Create: `e2e/scenarios/screen-share/suite.json`
- Create: `e2e/scenarios/screen-share/01-override.ts`
- Create: `e2e/scenarios/screen-share/02-end-restore.ts`

- [ ] **Step 1: Create silence-behavior suite**

`01-office-grid-return`: Bot speaks then mutes → wait 6s → assert layout-mode:GRID.
`02-pedestrian-stay`: Set Android to pedestrian mode → bot speaks then mutes → wait 6s → assert main-tile still present.

- [ ] **Step 2: Create pin-behavior suite**

`01-long-press-pin`: Bot-A and Bot-B connected. Long press bot-A tile on Android → assert pin-indicator.
`02-unpin`: Long press again → assert no pin-indicator.
`03-pin-holds`: Pin bot-A, bot-B speaks → assert main-tile stays bot-A, speaker-border on bot-B.

- [ ] **Step 3: Create screen-share suite**

`01-override`: Bot starts screen share → assert main-tile is screen share track.
`02-end-restore`: Bot stops screen share → assert previous layout restored.

- [ ] **Step 4: Commit**

```bash
git add e2e/scenarios/
git commit -m "feat(e2e): add silence, pin, and screen-share test suites"
```

---

### Task 11: Documentation

**Files:**
- Create: `e2e/README.md`

- [ ] **Step 1: Write README.md**

Cover:
1. Getting started: prerequisites, setup, first run
2. Architecture: orchestrator, participants, sync, evidence
3. Writing scenarios: suite.json format, ScenarioContext API, assertion methods, examples
4. CLI reference: `e2e run`, `e2e list`, env vars for timeouts
5. Test tag conventions: naming (`main-tile:<sid>`, `layout-mode:FOCUS`), how to add new tags
6. Reports: location, format, HTML report features
7. Troubleshooting: device not detected, LiveKit timeout, assertion timeout, bot crash
8. Adding a new platform: implementing the Participant interface

- [ ] **Step 2: Add e2e/reports to .gitignore**

Append `e2e/reports/` to the root `.gitignore`.

- [ ] **Step 3: Commit**

```bash
git add e2e/README.md .gitignore
git commit -m "docs: add E2E test framework documentation"
```

---

### Task 12: End-to-end smoke test

- [ ] **Step 1: Run the framework with speaker-focus/01-remote-speaks**

Prerequisites:
- Docker running
- Android device connected via USB
- visio-bot built (`cargo build -p visio-bot`)
- Android debug APK installed

```bash
cd e2e && npx tsx framework/cli.ts run speaker-focus/01-remote-speaks
```

- [ ] **Step 2: Verify report generated**

Check `e2e/reports/<timestamp>/report.html` exists and contains:
- The scenario result (pass or fail)
- At least one screenshot
- The assertions list

- [ ] **Step 3: Fix any issues found during smoke test**

- [ ] **Step 4: Run the full suite**

```bash
cd e2e && npx tsx framework/cli.ts run
```

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix(e2e): smoke test fixes"
```
