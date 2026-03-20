# iOS Xcode 26 + Swift 6 Migration — Design Spec

**Issue:** [#41 — iOS Target iOS version & compilation version](https://github.com/mmaudet/visio-mobile/issues/41)
**Date:** 2026-03-20
**Status:** Draft

## Goal

Compile the iOS app with Xcode 26 in Swift 6 language mode with strict concurrency, while keeping iOS 16.0 as the deployment target. Zero concurrency errors and zero concurrency warnings across all Swift files.

## Non-Goals

- GlassEffect / Liquid Glass iOS 26 visual changes (separate ticket)

## Context

- Current state: Xcode 15 project settings, Swift 5.0 language mode, iOS 16.0 deployment target
- Xcode 26.3 is already installed on the build machine
- ~20 Swift source files (excluding generated), 10 of which use async/await/Task
- UniFFI-generated `visio.swift` exposes types that are not yet `Sendable`
- No `@available` checks exist in the codebase today

## Approach: Progressive Migration (Approach A)

Activate Swift 6 language mode and migrate all files to full strict concurrency compliance. No temporary suppressions left in the final PR.

## Migration Phasing

1. **Phase 1 — Warnings mode:** Set `SWIFT_STRICT_CONCURRENCY = complete` while still in Swift 5 language mode. This surfaces all concurrency issues as warnings without breaking the build.
2. **Phase 2 — Fix all warnings:** Migrate all files (P0 through P5) until zero warnings remain.
3. **Phase 3 — Flip to Swift 6:** Set `SWIFT_VERSION = 6.0`. Since all warnings are already resolved, the build should compile cleanly.

**Rollback:** If a blocker is hit (e.g., UniFFI-generated code produces unresolvable errors), revert `SWIFT_VERSION` to 5.0. The `SWIFT_STRICT_CONCURRENCY = complete` setting can remain to keep warnings visible.

## Design

### 1. Project Configuration

Changes in `project.pbxproj`:
- `SWIFT_VERSION`: 5.0 → 6.0 (Phase 3)
- `LastUpgradeCheck` / `LastSwiftUpdateCheck`: 1500 → 2630
- `IPHONEOS_DEPLOYMENT_TARGET`: unchanged at 16.0
- `SWIFT_STRICT_CONCURRENCY = complete` (Phase 1 — redundant once in Swift 6 mode, but harmless as safety belt)

Rust/FFI build configuration: assumed unchanged, but must be validated on first Xcode 26 build attempt (Xcode 26 may change default clang flags).

### 2. VisioManager (P0 — Central File)

The most impacted file: 28 `@Published` properties, multi-queue access via `DispatchQueue`.

**MainActor annotation:**
- Annotate the class `@MainActor` — all `@Published` properties and UI methods are already called from the main thread
- Replace `DispatchQueue.main.async { }` with direct code (already on MainActor)

**FFI call pattern:**
- `client` (VisioClient) is declared as `nonisolated let` — safe because UniFFI types get `@unchecked Sendable` conformance (see section 5), and the Rust side manages its own thread safety
- Blocking FFI calls (connect, publish, etc.) use `Task.detached { [client] in ... }` to run off-main-thread, capturing `client` by value
- Results are brought back to MainActor via `await MainActor.run { self.somePublished = result }`

**VisioEventListener protocol:**
- `onEvent(event:)` is called from Rust threads — the method must be declared `nonisolated` to satisfy the protocol conformance, then dispatch UI updates via `Task { @MainActor in }`

**Semaphore removal:**
- `ensureMediaPermissions()` in VisioManager uses `DispatchSemaphore.wait()` which is incompatible with Swift concurrency (deadlock risk inside a Task). Convert to an `async` function using `withCheckedContinuation`. This cascades into making `connect()` and `connectWithToken()` async as well.
- `CameraCapture.start()` also uses `DispatchSemaphore` for camera permission — this runs on a private serial DispatchQueue (not inside a Task), so it is safe to keep. No mutable CameraCapture state is captured in the `requestAccess` closure.

**Tuple → struct:**
- `pendingTestConnect: (String, String, String?)?` — tuples are not Sendable in Swift 6. Convert to a small struct `TestConnectParams`.

### 3. SwiftUI Views (CallView, HomeView, InCallSettingsSheet)

- SwiftUI `View` types are implicitly `@MainActor` in Swift 6 — minimal structural changes
- Replace `Task { }` patterns in `.task(id:)` and `onChange()` with direct async calls to VisioManager methods (which are now async where needed)
- Remove nested `DispatchQueue.global().async` in closures — use `Task.detached` when genuinely needing off-main-thread work
- The `searchTask: Task<Void, Never>` pattern (HomeView, InCallSettingsSheet) remains acceptable with clean cancellation

Low-impact views (ChatView, ParticipantListSheet, SettingsView, VisioLogo, Theme): near-zero changes expected.

### 4. Services and Auxiliary Classes

**VideoFrameRouter (P2):**
- **Keep `NSLock`** — this is a performance-critical video path. `deliverFrame()` is called from a C callback at 30fps per track. Hopping to MainActor for every frame would degrade performance.
- Mark the class `@unchecked Sendable` — the `NSLock` provides correct synchronization, matching the same rationale used for UniFFI types.
- Global C callback (`visioOnVideoFrame`) remains `nonisolated`. Only the final `view.enqueueSampleBuffer()` dispatches to main.

**OidcAuthManager (P1):**
- Annotate `@MainActor` (the `@Published` properties and auth flow are UI-bound)
- Replace `DispatchQueue.main.async` with direct code
- `onComplete` callback remains a standard closure
- Verify `ASWebAuthenticationPresentationContextProviding` delegate conformance — if the system calls it on arbitrary queues, the delegate method needs `nonisolated` annotation

**ContextDetector (P2):**
- Annotate `@MainActor`
- CoreMotion and NWPathMonitor handlers arrive on system queues — UI state updates use `Task { @MainActor in }`
- FFI calls are routed through `nonisolated` wrapper methods on VisioManager (since `client` is `nonisolated let`, these wrappers simply forward to `client` without a MainActor hop):
  - `nonisolated func reportNetworkType(...)` — called from `pathUpdateHandler`
  - `nonisolated func reportMotionDetected(...)` — called from CoreMotion callback
  - `nonisolated func reportBluetoothCarKit(...)` — called from Bluetooth detection
- `audioRouteChanged` is an `@objc` notification handler called on arbitrary queues — it reads `connectionState` which is `@MainActor`-isolated. Wrap in `Task { @MainActor in }` to access it safely

**Singleton access pattern:**
- `VisioManager.shared`, `CallKitManager.shared`, `PiPManager.shared`, `VideoFrameRouter.shared` are `static let` on classes. Under Swift 6, non-Sendable classes need their singletons to be `@MainActor` or the class must be `Sendable`.
- For `@MainActor` classes (VisioManager, OidcAuthManager): `static let shared` inherits `@MainActor` — access from non-main contexts requires `await`.
- For non-MainActor classes (VideoFrameRouter): `@unchecked Sendable` makes `static let` safe.
- CallKitManager delegate callbacks must dispatch to MainActor before accessing `VisioManager.shared`. Pattern: wrap body in `Task { @MainActor in ... }` but call `action.fulfill()` **inside** the MainActor task after the work completes, since fulfilling before async work finishes would signal CallKit prematurely.

**Audio/Camera captures (P3):**
- `AVAudioSourceNode` render callback (AudioPlayout) and `AVCaptureVideoDataOutput` delegate (CameraCapture) run on real-time/high-priority threads
- These closures need `@Sendable` annotation and captured state must be Sendable
- Use `@unchecked Sendable` where the class manages its own synchronization (audio buffer queues, capture session queues)

**PiPManager (P3):**
- Annotate `@MainActor` — all API calls are UI-triggered (setup/tearDown from CallView)
- `pushFrame()` is called from VideoFrameRouter's video callback thread — must be declared `nonisolated` and only enqueue the sample buffer (no MainActor state access needed)
- `AVPictureInPictureControllerDelegate` and `AVPictureInPictureSampleBufferPlaybackDelegate` callbacks are called on main thread by the system — compatible with `@MainActor`

**CallKitManager (P3):**
- Add `@Sendable` on closures passed to system APIs
- Proper isolation annotations for delegate callbacks (see singleton pattern above for `action.fulfill()` ordering)
- Full migration, no temporary suppressions

### 5. UniFFI Generated Code and FFI Boundary

**`Generated/visio.swift`:**
- Auto-generated — not modified directly
- Add `VisioFFI+Sendable.swift` with `@retroactive @unchecked Sendable` extensions for FFI types passed between isolation domains
- Justified: Rust manages its own synchronization via Arc/Mutex

**C callbacks (visio-video):**
- C callbacks like `visioOnVideoFrame` are `nonisolated` by nature — no change required
- Swift state access from these callbacks handled via `@unchecked Sendable` on VideoFrameRouter

**Rust side:** No changes expected (visio-core, visio-ffi, generate-bindings.sh). Validate on first build.

## Risks

1. **UniFFI generated code:** May produce Swift 6 errors that cannot be resolved with extensions alone. Mitigation: rollback to Swift 5 + complete warnings mode.
2. **Xcode 26 clang changes:** New default C flags could affect Rust FFI linkage. Mitigation: validate on first build, adjust `.cargo/config.toml` if needed.
3. **System delegate protocols:** Some system frameworks may add Sendable requirements to delegate protocols in the iOS 26 SDK. Mitigation: verify each delegate conformance.
4. **Runtime concurrency bugs:** Data races and deadlocks are often runtime-only. Mitigated by the test plan below.

## Test Plan

1. **Compilation:** Zero errors, zero concurrency warnings under Swift 6 + Xcode 26.3
2. **Unit tests:** All 56 existing tests pass (48 visio-core + 8 visio-desktop)
3. **Manual smoke test on device:**
   - Connect to a room, verify audio/video
   - Toggle camera and microphone
   - Send/receive chat messages
   - PiP mode (enter/exit)
   - CallKit integration (incoming/outgoing)
   - Adaptive context mode changes
   - Disconnect and reconnect
4. **Backward compatibility:** Verify on iOS 16 device/simulator (iPhone 12 tester)

## Success Criteria

1. Project compiles in Swift 6 language mode with Xcode 26.3
2. Deployment target remains iOS 16.0
3. Zero concurrency errors across all Swift files
4. Zero concurrency warnings across all Swift files
5. No temporary suppressions (`nonisolated(unsafe)`, `@preconcurrency`) left in the codebase. `@unchecked Sendable` is explicitly allowed as a justified escape hatch for types that manage their own synchronization (VideoFrameRouter, UniFFI types, audio/camera captures).
6. All existing tests pass
7. Manual smoke test passes on iOS 16+ device
