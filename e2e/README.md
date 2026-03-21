# VisioMobile E2E Test Framework

## Overview

Professional end-to-end test framework for VisioMobile. It orchestrates multiple participants (bots, desktop, Android, iOS simulator), validates UI state via semantic test tags, captures screenshot evidence, and produces HTML/JSON reports.

The framework spins up a LiveKit server, launches participants into a room, executes scenario scripts that drive interactions and assertions, then tears everything down and generates a report.

## Prerequisites

| Dependency | Purpose |
|---|---|
| **Docker** | Runs the LiveKit server container |
| **Node.js 18+** | Runs the test framework |
| **ADB** | Android testing -- Android device must be connected via USB |
| **ffmpeg** | Bot media streaming (synthetic audio/video) |
| **Rust toolchain** | Building `visio-bot` |
| **Xcode + iOS Simulator** | Optional, for iOS screenshot capture |

## Quick Start

```bash
# Build the bot
cargo build -p visio-bot --release

# Install framework dependencies
cd e2e && npm install

# List available test suites
npm run e2e -- list

# Run all tests
npm run e2e -- run

# Run a specific suite
npm run e2e -- run speaker-focus

# Run a single scenario
npm run e2e -- run speaker-focus/01-remote-speaks
```

## Architecture

The framework is composed of four layers:

1. **Orchestrator** -- Manages the LiveKit server lifecycle (Docker container) and detects available platforms (is ADB connected? is the desktop app built? is an iOS simulator booted?).

2. **Runner** -- Discovers test suites under `e2e/scenarios/`, reads each `suite.json`, filters by platform availability and CLI arguments, then executes scenarios sequentially within each suite.

3. **Participants** -- Abstractions over the different actors in a call:
   - `BotParticipant` -- a `visio-bot` process controlled via stdin commands.
   - `DesktopParticipant` -- the Tauri desktop app, queried via Playwright-style test IDs.
   - `AndroidParticipant` -- the Android app on a physical device, queried via `adb shell` for Compose test tags.
   - `IosSimParticipant` -- the iOS app on a simulator, queried via `xcrun simctl` and accessibility identifiers.

4. **Reporting** -- Collects pass/fail results, screenshots, and timing data. Outputs to the terminal during the run, then writes a `results.json` and a visual `report.html` with embedded screenshots.

### Sync Strategy

Bot commands are sent via stdin. The bot writes an `ACK` line to stdout when it has executed the command. After ACK, the framework uses event-based sync (waiting for LiveKit room events like `ActiveSpeakers`) and polling UI assertions with configurable timeouts to verify that the client under test has reacted.

```
Framework  --stdin-->  Bot (command)
Framework  <--stdout-- Bot (ACK)
Framework  <--stdout-- Bot (room events)
Framework  --adb/pw--> Client (poll test tags until match or timeout)
```

## Writing Scenarios

### Suite structure

Each suite lives in its own directory under `e2e/scenarios/`:

```
e2e/scenarios/
  speaker-focus/
    suite.json
    01-remote-speaks.ts
    02-two-speakers.ts
```

### suite.json

Declares the suite metadata, bot participants, and platform requirements:

```json
{
  "name": "my-suite",
  "description": "What this suite tests",
  "bots": [{ "identity": "bot-a", "name": "Alice" }],
  "requires": { "android": true, "desktop": false, "ios": false }
}
```

- `bots` -- each entry spawns a `visio-bot` process that joins the room with the given identity and display name.
- `requires` -- which platform clients must be available for the suite to run. If a required platform is missing, the suite is skipped.

### Scenario files

Each scenario is a TypeScript file that exports a default async function:

```typescript
import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const android = ctx.android();

  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await android.assertTestTag("layout-mode:FOCUS", { timeout: 5000 });
  await android.screenshot("focus-mode-active");
}
```

Scenarios run sequentially within a suite. Each scenario gets a fresh room (all participants are disconnected and reconnected between scenarios).

## API Reference

### ScenarioContext

Obtained as the argument to each scenario function.

| Method | Returns | Description |
|---|---|---|
| `ctx.bot(identity)` | `BotParticipant` | Get a bot by its identity (as declared in `suite.json`) |
| `ctx.android()` | `AndroidParticipant` | Get the Android participant |
| `ctx.desktop()` | `DesktopParticipant` | Get the desktop participant |
| `ctx.ios()` | `IosSimParticipant` | Get the iOS simulator participant |
| `ctx.sleep(ms)` | `Promise<void>` | Wait for a fixed duration (use sparingly) |
| `ctx.log(message)` | `void` | Log a message to the report |

### BotParticipant

Controls a `visio-bot` process in the room.

| Method | Description |
|---|---|
| `speak()` | Start streaming synthetic audio (triggers active speaker detection) |
| `mute()` | Mute the audio track |
| `videoOn()` | Start publishing a synthetic video track |
| `videoOff()` | Stop publishing the video track |
| `screenShareStart()` | Start a screen share track |
| `screenShareStop()` | Stop the screen share track |
| `waitForEvent(pattern, timeoutMs)` | Wait for a room event matching the regex pattern |

All commands return a `Promise` that resolves after the bot ACKs.

### AndroidParticipant

Interacts with the Android app on a USB-connected device via ADB.

| Method | Description |
|---|---|
| `assertTestTag(tag, opts?)` | Poll until the Compose test tag is present on screen. Options: `{ timeout?: number }` |
| `assertNotTestTag(tag, opts?)` | Poll until the Compose test tag is absent from screen |
| `tap(tag)` | Tap on the element identified by the test tag |
| `longPress(tag)` | Long-press on the element identified by the test tag |
| `screenshot(name)` | Capture a screenshot and save it to the report |

### DesktopParticipant

Interacts with the Tauri desktop app.

| Method | Description |
|---|---|
| `assertTestId(testId, opts?)` | Poll until the element with `data-testid` is present. Options: `{ timeout?: number }` |
| `assertNotTestId(testId, opts?)` | Poll until the element with `data-testid` is absent |
| `click(testId)` | Click on the element identified by `data-testid` |
| `screenshot(name)` | Capture a screenshot and save it to the report |

### IosSimParticipant

Interacts with the iOS app on a booted simulator.

| Method | Description |
|---|---|
| `assertAccessibilityId(id, opts?)` | Poll until the accessibility identifier is present. Options: `{ timeout?: number }` |
| `assertNotAccessibilityId(id, opts?)` | Poll until the accessibility identifier is absent |
| `tap(id)` | Tap on the element identified by the accessibility identifier |
| `screenshot(name)` | Capture a screenshot and save it to the report |

## Test Tag Conventions

Test tags provide a semantic, stable interface for assertions. They follow the `name:value` format.

| Tag | Values | Description |
|---|---|---|
| `layout-mode:<mode>` | `FOCUS`, `GRID` | Current layout mode |
| `adaptive-mode:<mode>` | `OFFICE`, `PEDESTRIAN`, `CAR` | Current adaptive mode (Android only) |
| `main-tile:<sid>` | participant SID | The focused/main participant in focus layout |
| `grid-tile-<index>:<sid>` | participant SID | Participant at the given grid position |
| `secondary-tile-<index>:<sid>` | participant SID | Thumbnail at the given position in focus layout |
| `speaker-border:<sid>` | participant SID | Active speaker visual indicator |
| `pin-indicator:<sid>` | participant SID | Pinned participant visual indicator |

### Adding new test tags

Each platform uses a different mechanism:

- **Android (Compose):** `Modifier.testTag("name:value")`
- **Desktop (Tauri/HTML):** `data-testid="name:value"`
- **iOS (SwiftUI):** `.accessibilityIdentifier("name:value")`

When adding a new assertion, first add the test tag on the relevant platform(s), then use the corresponding assertion method in your scenario.

## Configuration

All configuration is via environment variables. Defaults are tuned for local development.

| Variable | Default | Description |
|---|---|---|
| `E2E_ASSERT_TIMEOUT` | `5000` | Assertion poll timeout in ms |
| `E2E_POLL_INTERVAL` | `500` | Interval between assertion polls in ms |
| `E2E_SCENARIO_TIMEOUT` | `60000` | Maximum duration for a single scenario in ms |
| `E2E_BOT_CONNECT_TIMEOUT` | `10000` | Timeout for bot to connect to the room in ms |
| `E2E_ACK_TIMEOUT` | `3000` | Timeout for bot command ACK in ms |

Example:

```bash
E2E_ASSERT_TIMEOUT=10000 E2E_SCENARIO_TIMEOUT=120000 npm run e2e -- run
```

## Reports

After a test run, reports are written to a timestamped directory:

```
e2e/reports/<timestamp>/
  report.html      # Visual report with embedded screenshots
  results.json     # Machine-readable results (pass/fail, durations, errors)
  screenshots/     # Per-suite/scenario PNG screenshots
```

- `report.html` -- Open in a browser to review results visually. Each scenario shows its status, duration, log messages, and any captured screenshots.
- `results.json` -- Suitable for CI integration. Contains structured data for each suite and scenario.
- `screenshots/` -- Organized as `<suite>/<scenario>/<name>.png`.

The `e2e/reports/` directory is gitignored.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| "No Android device connected" | ADB cannot find a device | Run `adb devices` and verify a device is listed. Check USB connection and USB debugging is enabled. |
| "Bot build failed" | `visio-bot` binary not found | Run `cargo build -p visio-bot --release` from the project root. |
| "LiveKit not starting" | Docker daemon not running or port conflict | Check that Docker is running (`docker ps`). Check that port 7880 is not in use. |
| "Assertion timeout" | UI did not reach expected state in time | Increase `E2E_ASSERT_TIMEOUT`, verify the test tag exists in the app code, or check if the scenario logic is correct. |
| "Bot command timeout" | Bot process crashed or is unresponsive | Check bot logs in the report output. Rebuild the bot if needed. |
| "Suite skipped" | Required platform not available | Check `suite.json` `requires` field. Ensure the required device/simulator is connected and detected. |
| "Screenshot failed" | ADB or simctl error | For Android: check device is unlocked. For iOS: check simulator is booted (`xcrun simctl list`). |
