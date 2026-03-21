# E2E Test Framework — Design Spec

## Goal

Build a professional, automated E2E test framework for VisioMobile that orchestrates multiple participants across platforms, validates UI state programmatically, captures screenshot evidence, and produces structured execution reports.

First milestone: automate the PR #71 unified speaker layout test plan.

## Participants (v1)

| Participant | Role | Piloting | Assertions | Evidence |
|-------------|------|----------|------------|----------|
| **Bot(s)** | Remote participant(s), controllable | Child process, stdin commands, stdout events | Event-based (source of truth) | Structured logs |
| **Desktop** | Local participant (Tauri dev server) | Playwright on Vite dev server (localhost:5173) | `data-testid` queries | Playwright screenshots |
| **Android** | Local participant (physical Pixel Fold Pro) | ADB + UIAutomator | `testTag` via XML parse | `adb screencap` |
| **iOS sim** | Observer (no programmatic assertions) | `xcrun simctl` (deep link + screenshot) | None | `simctl screenshot` |

Multiple bots can run simultaneously for 3+ participant scenarios. Each bot is a separate child process with its own identity and stdin/stdout channel.

Web user (Playwright on Meet server) is out of scope for v1.

**Desktop testing note:** Tests run against the Vite dev server (`localhost:5173`), not the Tauri native window. This means Playwright interacts with the same React frontend but without the native window chrome. This is acceptable for v1 because the layout engine logic lives entirely in the React layer.

## Architecture

```
e2e/
├── framework/
│   ├── cli.ts                  # Entry point: e2e run / e2e list
│   ├── runner.ts               # Suite discovery, execution loop, reporting
│   ├── orchestrator.ts         # Setup/teardown LiveKit, manage participants
│   ├── participants/
│   │   ├── types.ts            # Participant / BotParticipant / AndroidParticipant interfaces
│   │   ├── bot.ts              # Spawn visio-bot, stdin commands, stdout event parsing
│   │   ├── desktop.ts          # Playwright browser on localhost:5173
│   │   ├── android.ts          # ADB commands, UIAutomator dump, screencap
│   │   └── ios-sim.ts          # xcrun simctl: deep link, screenshot
│   ├── assertions/
│   │   └── poll.ts             # pollUntil() with timeout, assertTestTag, assertNotTestTag
│   ├── evidence/
│   │   └── screenshot.ts       # Per-platform screenshot capture, file naming
│   ├── reporting/
│   │   ├── html.ts             # HTML report with screenshot gallery
│   │   ├── json.ts             # Machine-readable results.json
│   │   └── terminal.ts         # Live terminal output during execution
│   └── utils/
│       ├── livekit.ts          # Docker start/stop, health check
│       ├── token.ts            # JWT generation (wraps visio-bot --token-only)
│       └── adb.ts              # ADB helpers: tap, longPress, swipe, dumpUi, screencap
├── scenarios/
│   ├── speaker-focus/
│   │   ├── suite.json
│   │   ├── 01-remote-speaks.ts
│   │   ├── 02-local-speaks.ts
│   │   ├── 03-stabilization.ts
│   │   └── 04-rapid-changes.ts
│   ├── silence-behavior/
│   │   ├── suite.json
│   │   ├── 01-office-grid-return.ts
│   │   └── 02-pedestrian-stay.ts
│   ├── pin-behavior/
│   │   ├── suite.json
│   │   ├── 01-long-press-pin.ts
│   │   ├── 02-unpin.ts
│   │   └── 03-pin-holds-on-speaker-change.ts
│   └── screen-share/
│       ├── suite.json
│       ├── 01-override.ts
│       └── 02-end-restore.ts
├── reports/                    # Generated per-run (gitignored)
│   └── 2026-03-21T14-30-00/
│       ├── report.html
│       ├── results.json
│       ├── summary.txt
│       ├── screenshots/
│       └── logs/               # Bot stderr + stdout captured per-bot
├── README.md                   # Full documentation
└── package.json                # Node.js deps: tsx, playwright
```

## Participant Interfaces

```typescript
interface Participant {
  readonly identity: string;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  screenshot(name: string): Promise<string>; // returns file path
}

interface BotParticipant extends Participant {
  readonly sid: string; // set after connect, from [CONNECTED] event
  speak(): Promise<void>;
  mute(): Promise<void>;
  videoOn(): Promise<void>;
  videoOff(): Promise<void>;
  screenShareStart(): Promise<void>;
  screenShareStop(): Promise<void>;
  waitForEvent(pattern: string | RegExp, timeout?: number): Promise<string>;
}

interface AndroidParticipant extends Participant {
  assertTestTag(tag: string, options?: { timeout?: number }): Promise<void>;
  assertNotTestTag(tag: string, options?: { timeout?: number }): Promise<void>;
  tap(tag: string): Promise<void>;
  longPress(tag: string): Promise<void>;
  dumpUiTree(): Promise<string>;
}

interface DesktopParticipant extends Participant {
  assertTestId(testId: string, options?: { timeout?: number }): Promise<void>;
  assertNotTestId(testId: string, options?: { timeout?: number }): Promise<void>;
  click(testId: string): Promise<void>;
}

interface IosSimParticipant extends Participant {
  // v1: connect + screenshot only, no programmatic assertions
}
```

## Scenario Context

The `ScenarioContext` provides access to participants and utilities:

```typescript
interface ScenarioContext {
  /** Get a bot by identity. Throws if bot identity not in suite.json. */
  bot(identity: string): BotParticipant;

  /** Get the Android participant. Throws if android not connected. */
  android(): AndroidParticipant;

  /** Get the Desktop participant. Throws if desktop not available. */
  desktop(): DesktopParticipant;

  /** Get the iOS sim participant. Throws if iOS sim not booted. */
  ios(): IosSimParticipant;

  /** Sleep for a duration (ms). Prefer waitForEvent when possible. */
  sleep(ms: number): Promise<void>;

  /** Log a step in the report (appears in terminal + HTML report). */
  log(message: string): void;

  /** Identity → SID mapping for all connected participants. */
  sidMap: ReadonlyMap<string, string>;
}
```

Scenarios that call `ctx.android()` when no device is connected will throw — the runner catches the error and marks the scenario as `fail`. Suites should declare their requirements in `suite.json` to get a clean `skip` instead.

## Suite Definition (`suite.json`)

```json
{
  "name": "speaker-focus",
  "description": "Active speaker focus and stabilization",
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

The `bots` array declares the **maximum set** of bots for the suite. Individual scenarios use only the bots they need — calling `ctx.bot("bot-a")` connects bot-a, unused bots are not spawned. This avoids wasting resources when `01-remote-speaks` needs 1 bot but `04-rapid-changes` needs 3.

The runner skips suites whose requirements cannot be met (no Android device connected, etc.) and reports them as `skip` with a reason.

## Scenario API

Each scenario file exports a function:

```typescript
import { ScenarioContext } from "../../framework/runner";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const bob = ctx.bot("bot-b");
  const android = ctx.android();
  const desktop = ctx.desktop();

  // Alice speaks
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/);

  // Verify Android shows Alice in main tile
  const aliceSid = alice.sid;
  await android.assertTestTag(`layout-mode:FOCUS`, { timeout: 5000 });
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });
  await android.screenshot("01-alice-in-main-tile");

  // Same on desktop
  await desktop.assertTestId(`main-tile:${aliceSid}`, { timeout: 5000 });
  await desktop.screenshot("01-alice-in-main-tile");
}
```

## Bot Interactive Mode

### Activation

New CLI flag: `--interactive`

When set, the bot:
1. Connects to the room (audio/video tracks published but **muted** by default)
2. Prints `[CONNECTED] identity=<id> sid=<sid>` on stdout
3. Listens on stdin for commands (one per line)
4. Acknowledges each command with `[ACK] <COMMAND>` on stdout
5. Continues emitting events on stdout
6. Does NOT run the turn-based scenario

The existing non-interactive mode (turn-based) remains the default for manual testing.

### Stdin Commands

| Command | Effect |
|---------|--------|
| `SPEAK` | Unmute audio track (starts publishing 440Hz sine wave → triggers ActiveSpeaker) |
| `MUTE` | Mute audio track (stops appearing as active speaker) |
| `VIDEO_ON` | Unmute video track |
| `VIDEO_OFF` | Mute video track |
| `SCREEN_SHARE_START` | Publish screen share track |
| `SCREEN_SHARE_STOP` | Unpublish screen share track |
| `QUIT` | Disconnect and exit cleanly |

**Implementation note:** `SPEAK`/`MUTE` toggle the audio track's `enabled` state (mute/unmute), not start/stop the source. The 440Hz sine wave source runs continuously but is only transmitted when unmuted. This aligns with LiveKit's track muting mechanism and ensures `ActiveSpeakersChanged` events fire correctly.

### Stdout Events

The interactive mode maintains an internal `identity → SID` mapping for all participants in the room. Events use **identity** (not SID) to simplify scenario matching. The bot resolves SIDs to identities using the participant list from the LiveKit SDK.

```
[CONNECTED] identity=bot-a sid=PA_abc123
[ACK] SPEAK
[EVENT] ParticipantJoined: identity=android-user sid=PA_def456
[EVENT] ParticipantLeft: identity=android-user sid=PA_def456
[EVENT] ActiveSpeakers: bot-a
[EVENT] ActiveSpeakers: bot-a,bot-b
[EVENT] ActiveSpeakers:
[EVENT] TrackSubscribed: identity=android-user kind=audio sid=TR_xyz
[EVENT] TrackMuted: identity=bot-b kind=audio
[EVENT] TrackUnmuted: identity=bot-b kind=audio
[EVENT] ChatMessage: from=android-user text=hello
```

**Note on ActiveSpeakers:** The LiveKit SDK fires `ActiveSpeakersChanged(Vec<String>)` with SIDs. The interactive mode resolves these to identities and emits a comma-separated list (empty list = silence). This matches the plural nature of the SDK event while providing human-readable identities.

## Synchronization Strategy

**Event-based sync + polling assertions.**

1. The orchestrator sends a command to a bot (e.g., `SPEAK`)
2. It waits for the `[ACK] SPEAK` confirmation on stdout
3. It then waits for the corresponding event (e.g., `ActiveSpeakers: bot-a`)
4. Then it polls the UI on Android/Desktop with a timeout:
   - `pollUntil(() => android.hasTestTag("main-tile:PA_abc"), { interval: 500, timeout: 5000 })`
5. If the tag appears within timeout → assertion passes
6. If timeout expires → assertion fails with a descriptive error
7. Screenshot is taken after each assertion (pass or fail) for evidence

## Timeouts and Configuration

Default timeouts (configurable via environment variables):

| Setting | Default | Env var |
|---------|---------|---------|
| Bot connect timeout | 10s | `E2E_BOT_CONNECT_TIMEOUT` |
| Assertion poll timeout | 5s | `E2E_ASSERT_TIMEOUT` |
| Assertion poll interval | 500ms | `E2E_POLL_INTERVAL` |
| Scenario timeout | 60s | `E2E_SCENARIO_TIMEOUT` |
| Bot command ACK timeout | 3s | `E2E_ACK_TIMEOUT` |

If a scenario exceeds its timeout, it is killed and reported as `fail` with "timeout" reason.

## Test Tags — Prerequisite Native Code Changes

**These test tags do not exist today and must be added before the framework can validate UI state.** This is an implementation prerequisite, not documentation of existing tags.

### Android — `CallScreen.kt`

Requires `Modifier.semantics { testTagsAsResourceId = true }` at the root composable so that `testTag` values appear as `resource-id` in UIAutomator XML dumps.

```kotlin
// On the video area container:
Modifier.testTag("layout-mode:${if (layoutDecision.mode == LayoutMode.FOCUS) "FOCUS" else "GRID"}")

// Adaptive mode:
Modifier.testTag("adaptive-mode:${effectiveAdaptiveMode.name}")

// FOCUS mode — main tile:
Modifier.testTag("main-tile:${focusedDisplayItem.participant.sid}")

// FOCUS mode — each secondary tile (indexed):
Modifier.testTag("secondary-tile-$index:${item.participant.sid}")

// GRID mode — each cell (indexed):
Modifier.testTag("grid-tile-$idx:${item.participant.sid}")

// Speaker border (on ParticipantTile when isActiveSpeaker=true):
Modifier.testTag("speaker-border:${participant.sid}")

// Pin indicator (when isPinned=true):
Modifier.testTag("pin-indicator:${participant.sid}")
```

**UIAutomator note:** The `adb.ts` helper will parse the XML dump using XPath to find elements by `resource-id`. Example: `//node[@resource-id="main-tile:PA_abc123"]`.

### Desktop — `App.tsx`

Uses consistent **uppercase** casing to match Android (normalized across platforms):

```tsx
// On focus/grid container:
data-testid={`layout-mode:${focusedDisplayItem ? "FOCUS" : "GRID"}`}

// Main tile:
data-testid={`main-tile:${focusedDisplayItem.participant.sid}`}

// Grid tiles:
data-testid={`grid-tile-${idx}:${d.participant.sid}`}

// Secondary tiles in focus mode:
data-testid={`secondary-tile-${idx}:${d.participant.sid}`}
```

### iOS — `CallView.swift`

For v2 readiness (no assertions in v1, but tags in place):

```swift
.accessibilityIdentifier("layout-mode:\(layoutDecision.mode == .focus ? "FOCUS" : "GRID")")
.accessibilityIdentifier("main-tile:\(participant.sid)")
```

## CLI

```bash
# Run all suites
npx tsx e2e/framework/cli.ts run

# Run specific suites
npx tsx e2e/framework/cli.ts run speaker-focus pin-behavior

# Run a single scenario
npx tsx e2e/framework/cli.ts run speaker-focus/01-remote-speaks

# List available suites with requirements
npx tsx e2e/framework/cli.ts list

# Convenience aliases (via package.json scripts)
npm run e2e                          # run all
npm run e2e -- run speaker-focus     # run one suite
npm run e2e -- list                  # list suites
```

## Reports

### Terminal (live during execution)

```
[speaker-focus] 01-remote-speaks ............ PASS (12.4s)
[speaker-focus] 02-local-speaks ............. PASS (8.1s)
[speaker-focus] 03-stabilization ............ PASS (15.2s)
[pin-behavior]  01-long-press-pin ........... PASS (10.8s)
[screen-share]  01-override ................. SKIP (no desktop)

4 passed, 0 failed, 1 skipped
Report: e2e/reports/2026-03-21T14-30-00/report.html
```

### `results.json`

```json
{
  "timestamp": "2026-03-21T14:30:00Z",
  "duration": 142,
  "participants": {
    "bots": ["bot-a", "bot-b"],
    "desktop": true,
    "android": true,
    "ios": false
  },
  "suites": [
    {
      "name": "speaker-focus",
      "status": "pass",
      "scenarios": [
        {
          "name": "01-remote-speaks",
          "status": "pass",
          "duration": 12400,
          "assertions": [
            { "description": "layout-mode:FOCUS on android", "status": "pass", "durationMs": 1200 },
            { "description": "main-tile:PA_abc123 on android", "status": "pass", "durationMs": 800 }
          ],
          "screenshots": [
            "screenshots/speaker-focus/01-remote-speaks/01-alice-speaking.android.png",
            "screenshots/speaker-focus/01-remote-speaks/01-alice-speaking.desktop.png"
          ]
        }
      ]
    }
  ],
  "summary": { "pass": 4, "fail": 0, "skip": 1, "total": 5 }
}
```

### `report.html`

Static HTML page (no external dependencies):
- Summary bar: pass/fail/skip counts with color coding
- Each suite as a collapsible section
- Each scenario shows: status, duration, assertions list, screenshot thumbnails
- Click screenshot to view full size
- Filter by status (pass/fail/skip)
- CSS inline, images as relative paths

### Log capture

Bot stdout and stderr are captured to `logs/<bot-identity>.stdout.log` and `logs/<bot-identity>.stderr.log` in the report directory. On failure, relevant log excerpts are included in the HTML report for debugging.

## PR #71 Scenarios (First Milestone)

### Suite: `speaker-focus`

| Scenario | Bots | Android | Desktop | Description |
|----------|------|---------|---------|-------------|
| `01-remote-speaks` | 1 (speaks) | assert main-tile | assert main-tile | Remote speaker in main tile |
| `02-local-speaks` | 1 (mute) | assert main-tile stays remote | assert main-tile stays remote | Never show self as focused |
| `03-stabilization` | 2 (alternate) | assert main-tile holds 2.5s | assert main-tile holds 2.5s | Focus doesn't switch before MIN_HOLD |
| `04-rapid-changes` | 3 (rapid) | assert no ping-pong | assert no ping-pong | Rapid alternation doesn't cause flicker |

### Suite: `silence-behavior`

| Scenario | Bots | Android | Desktop | Description |
|----------|------|---------|---------|-------------|
| `01-office-grid-return` | 1 (speaks then mutes) | assert layout-mode:GRID after 6s | assert layout-mode:GRID | Silence > 5s returns to grid in office |
| `02-pedestrian-stay` | 1 (speaks then mutes) | assert main-tile stays (pedestrian mode) | N/A (pedestrian is mobile-only) | Pedestrian keeps last speaker |

### Suite: `pin-behavior`

Pin is tested on Android only for v1 (long press gesture). Desktop has click-to-pin in the participant list menu but is not tested in this milestone.

| Scenario | Bots | Android | Desktop | Description |
|----------|------|---------|---------|-------------|
| `01-long-press-pin` | 2 | long press tile, assert pin-indicator | N/A | Pin appears on long press |
| `02-unpin` | 2 | long press again, assert no pin-indicator | N/A | Pin removed on second long press |
| `03-pin-holds` | 2 (alternate) | assert main-tile stays pinned | N/A | Pin holds despite speaker change |

### Suite: `screen-share`

| Scenario | Bots | Android | Desktop | Description |
|----------|------|---------|---------|-------------|
| `01-override` | 1 (screen share) | assert main-tile is screen share | assert main-tile is screen share | Screen share overrides speaker focus |
| `02-end-restore` | 1 (screen share then stop) | assert return to previous state | assert return to previous state | Stopping screen share restores layout |

## Pre-Join Lobby (Future Extension)

The upcoming pre-join screen (audio/video configuration before entering a room) will impact the framework:

1. **Deep link bypass**: the `visio-test://connect` deep link will accept `&skip_prejoin=true` to bypass the pre-join screen for automated tests. Without this, all scenarios would block on the pre-join UI.

2. **Dedicated test suite**: a future `e2e/scenarios/pre-join/` suite will test the pre-join screen itself (camera preview, mic/cam toggles, join button).

The framework architecture supports this without changes — it is just a new suite with new scenarios.

## Documentation (`e2e/README.md`)

The README will cover:

1. **Getting started**: prerequisites (Docker, ADB, Node.js, ffmpeg), setup steps, running the first test
2. **Architecture overview**: orchestrator, participants, sync strategy, evidence capture
3. **Writing scenarios**: `suite.json` format, scenario API, available assertions, example walkthrough
4. **CLI reference**: `e2e run`, `e2e list`, options, environment variables
5. **Test tag conventions**: naming scheme (`main-tile:<sid>`, `layout-mode:FOCUS`), how to add new tags
6. **Reports**: location, format, how to read the HTML report
7. **Troubleshooting**: common issues (device not detected, LiveKit timeout, assertion timeout, bot crash)
8. **Adding a new platform**: how to implement the `Participant` interface for a new target

## Dependencies

New:
- `tsx` — run TypeScript directly (zero-config, already used in the project)
- `playwright` — desktop browser automation (already in devDependencies for existing Playwright tests)

No new dependency for Android (ADB) or iOS (xcrun simctl).

The bot (`visio-bot`) is built from the existing Rust crate with the addition of `--interactive` mode.

## Out of Scope (v1)

- Web user participant (Playwright on Meet server)
- iOS programmatic assertions (XCUITest)
- Network condition simulation (packet loss, bandwidth)
- Performance benchmarking (FPS, latency)
- Visual diff / pixel comparison
- CI integration (manual trigger only for v1)
- Parallel suite execution (sequential in v1)
