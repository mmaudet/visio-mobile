# iOS Xcode 26 + Swift 6 Migration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Compile the iOS app with Xcode 26 in Swift 6 strict concurrency mode while keeping iOS 16.0 deployment target — zero errors, zero warnings.

**Architecture:** Progressive migration: enable strict concurrency warnings in Swift 5 mode first, fix all warnings, then flip to Swift 6. Core pattern: `@MainActor` on UI-bound classes, `nonisolated` methods for FFI calls, `@unchecked Sendable` for classes with manual synchronization.

**Tech Stack:** Swift 6, Xcode 26.3, SwiftUI, UniFFI, AVFoundation, CallKit, CoreMotion

**Spec:** `docs/superpowers/specs/2026-03-20-ios-xcode26-swift6-migration-design.md`

---

### Task 1: Enable strict concurrency warnings (Phase 1)

**Files:**
- Modify: `ios/VisioMobile.xcodeproj/project.pbxproj`

- [ ] **Step 1: Add SWIFT_STRICT_CONCURRENCY build setting**

In `project.pbxproj`, add `SWIFT_STRICT_CONCURRENCY = complete;` to **every** build configuration block (Debug and Release) for the main app target. Also update `LastUpgradeCheck` and `LastSwiftUpdateCheck` from `1500` to `2630`.

Search for `SWIFT_VERSION = 5.0;` in each build configuration — add `SWIFT_STRICT_CONCURRENCY = complete;` on the line before it. There are multiple build configuration sections (Debug/Release for each target). Update all of them.

Also search for `LastSwiftUpdateCheck = 1500` and `LastUpgradeCheck = 1500` and change both to `2630`.

- [ ] **Step 2: Build to see warnings**

Run:
```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep -E "warning:|error:" | head -80
```

Expected: The build succeeds but produces concurrency warnings. Capture the full warning list — this is the migration roadmap.

- [ ] **Step 3: Commit**

```bash
git add ios/VisioMobile.xcodeproj/project.pbxproj
git commit -m "build(ios): enable strict concurrency checking in Swift 5 mode"
```

---

### Task 2: Add TestConnectParams struct and Sendable conformances

**Files:**
- Modify: `ios/VisioMobile/VisioManager.swift:34`
- Modify: `ios/VisioMobile/VisioManager.swift:1039-1045` (ReactionData)

- [ ] **Step 1: Add TestConnectParams struct**

Add above the `VisioManager` class definition (before line 8):

```swift
struct TestConnectParams: Sendable {
    let livekitUrl: String
    let token: String
    let mediaFile: String?
}
```

- [ ] **Step 2: Replace the tuple in VisioManager**

Change line 34 from:
```swift
@Published var pendingTestConnect: (String, String, String?)? = nil
```
to:
```swift
@Published var pendingTestConnect: TestConnectParams? = nil
```

- [ ] **Step 3: Update all usage sites of pendingTestConnect**

In `CallView.swift`, find any destructuring of the tuple (e.g., `let (url, token, media) = ...`) and update to use struct field access (e.g., `params.livekitUrl`, `params.token`, `params.mediaFile`).

In `VisioMobileApp.swift`, find where the tuple is constructed (the deep link handler) and update to use `TestConnectParams(livekitUrl:token:mediaFile:)`.

- [ ] **Step 4: Make ReactionData Sendable**

In `VisioManager.swift`, add `Sendable` conformance to `ReactionData` (line 1039):

```swift
struct ReactionData: Identifiable, Sendable {
```

This is valid because all fields (`Int64`, `String`, `Date`) are already Sendable.

- [ ] **Step 5: Build and verify**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep -c "warning:"
```

Expected: Build succeeds. Warning count should be the same or slightly lower than Task 1.

- [ ] **Step 6: Commit**

```bash
git add ios/VisioMobile/VisioManager.swift ios/VisioMobile/Views/CallView.swift ios/VisioMobile/VisioMobileApp.swift
git commit -m "refactor(ios): replace tuple with TestConnectParams struct, add Sendable"
```

---

### Task 3: Migrate VisioManager to @MainActor — Part 1 (annotation + simple methods)

**Files:**
- Modify: `ios/VisioMobile/VisioManager.swift`

- [ ] **Step 1: Add @MainActor annotation to the class**

Change line 8 from:
```swift
class VisioManager: ObservableObject {
```
to:
```swift
@MainActor
class VisioManager: ObservableObject {
```

- [ ] **Step 2: Mark `client` as nonisolated**

Change line 56 from:
```swift
let client: VisioClient
```
to:
```swift
nonisolated let client: VisioClient
```

This is safe because `VisioClient` is already `@unchecked Sendable` in the generated code.

- [ ] **Step 3: Convert ensureMediaPermissions to async**

Replace the existing `ensureMediaPermissions` method (lines 365-389) with:

```swift
static func ensureMediaPermissions(mic: Bool, camera: Bool) async {
    if mic {
        let micStatus = AVCaptureDevice.authorizationStatus(for: .audio)
        if micStatus == .notDetermined {
            let granted = await AVCaptureDevice.requestAccess(for: .audio)
            NSLog("VisioManager: mic permission %@", granted ? "granted" : "denied")
        }
    }

    if camera {
        let camStatus = AVCaptureDevice.authorizationStatus(for: .video)
        if camStatus == .notDetermined {
            let granted = await AVCaptureDevice.requestAccess(for: .video)
            NSLog("VisioManager: camera permission %@", granted ? "granted" : "denied")
        }
    }
}
```

Note: `AVCaptureDevice.requestAccess(for:)` has an async variant available since iOS 16.

- [ ] **Step 4: Convert configureAudioSession to nonisolated**

The audio session configuration doesn't need MainActor — it's a system API that's thread-safe. Mark it:

```swift
nonisolated static func configureAudioSession() {
```

- [ ] **Step 5: Convert simple synchronous methods that don't touch DispatchQueue**

The following methods are already safe on MainActor and need no changes (they just access `@Published` state or call `client` directly):
- `setChatOpen`, `getSettings`, `setDisplayName`, `setLanguage`, `setMicEnabledOnJoin`, `setCameraEnabledOnJoin`, `setTheme`, `updateDisplayName`, `switchCamera`, `setNotificationParticipantJoin`, `setNotificationHandRaised`, `setNotificationMessageReceived`, `startAudioPlayout`, `stopAudioPlayout`, `routeAudioToBluetooth`, `restoreDefaultAudioRoute`, `clearLobbyNotification`

These need no code changes — they inherit `@MainActor` from the class.

Methods that call `client` synchronously (like `setChatOpen`, `getSettings`, `setDisplayName`, etc.) will work because `client` is `nonisolated let` and `VisioClient` is `Sendable`.

- [ ] **Step 6: Add nonisolated FFI wrapper methods for ContextDetector**

Add at the end of the `// MARK: - Settings` section:

```swift
// MARK: - Nonisolated FFI wrappers (for use from non-MainActor contexts)

nonisolated func reportNetworkType(_ type: NetworkType) {
    client.reportNetworkType(networkType: type)
}

nonisolated func reportMotionDetected(_ detected: Bool) {
    client.reportMotionDetected(detected: detected)
}

nonisolated func reportBluetoothCarKit() {
    client.reportBluetoothCarKit()
}
```

- [ ] **Step 7: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep -E "warning:|error:" | head -80
```

Expected: Many new errors/warnings because the DispatchQueue methods conflict with @MainActor. That's expected — we fix them in the next task.

- [ ] **Step 8: Commit**

```bash
git add ios/VisioMobile/VisioManager.swift
git commit -m "refactor(ios): annotate VisioManager @MainActor, async permissions, nonisolated FFI"
```

---

### Task 4: Migrate VisioManager — Part 2 (convert DispatchQueue methods to async)

**Files:**
- Modify: `ios/VisioMobile/VisioManager.swift`

All methods that use `DispatchQueue.global().async { [weak self] in ... DispatchQueue.main.async { ... } }` must be converted to use `Task.detached` + `MainActor.run`. The pattern is:

**Before:**
```swift
func someMethod() {
    DispatchQueue.global(qos: .userInitiated).async { [weak self] in
        guard let self else { return }
        let result = self.client.doSomething()
        DispatchQueue.main.async {
            self.somePublished = result
        }
    }
}
```

**After:**
```swift
func someMethod() {
    let client = self.client
    Task.detached {
        let result = client.doSomething()
        await MainActor.run { [weak self] in
            self?.somePublished = result
        }
    }
}
```

Key rules:
- Capture `client` (which is `nonisolated let` and Sendable) before entering `Task.detached`
- Use `[weak self]` only in the `MainActor.run` block (not in `Task.detached` — we don't want to extend lifetime during FFI calls)
- For methods that are called by UI (buttons, etc.), they remain synchronous and fire-and-forget via `Task.detached`

- [ ] **Step 1: Convert `connect()` (lines 116-182)**

```swift
func connect(url: String, username: String?) {
    self.connectionState = .connecting
    self.errorMessage = nil

    let client = self.client
    Task.detached {
        do {
            let settings = client.getSettings()
            let cameraNeeded = settings.cameraEnabledOnJoin || client.isCameraEnabled()
            await VisioManager.ensureMediaPermissions(mic: settings.micEnabledOnJoin, camera: cameraNeeded)

            if settings.micEnabledOnJoin {
                VisioManager.configureAudioSession()
            }

            try client.connect(meetUrl: url, username: username)

            if settings.micEnabledOnJoin {
                try client.setMicrophoneEnabled(enabled: true)
            }
            if cameraNeeded {
                try client.setCameraEnabled(enabled: true)
            }

            let parts = client.participants()
            let mic = client.isMicrophoneEnabled()
            let cam = client.isCameraEnabled()
            let msgs = client.chatMessages()
            let state = client.connectionState()
            let hand = client.isHandRaised()

            var capture: AudioCapture? = nil
            if mic {
                capture = AudioCapture()
                capture?.start()
            }

            await MainActor.run { [weak self] in
                guard let self else { return }
                self.participants = parts
                self.isMicEnabled = mic
                self.isCameraEnabled = cam
                self.chatMessages = msgs
                self.connectionState = state
                self.isHandRaised = hand
                self.errorMessage = nil
                self.audioCapture = capture
                if cam {
                    let camCapture = CameraCapture()
                    camCapture.start()
                    self.cameraCapture = camCapture
                }
                self.startAudioPlayout()
                self.startContextDetection()
            }
        } catch {
            await MainActor.run { [weak self] in
                self?.errorMessage = "Connection failed: \(error.localizedDescription)"
            }
        }
    }
}
```

- [ ] **Step 2: Convert `connectWithToken()` (lines 184-236)**

Same pattern as `connect()`. Replace `DispatchQueue.global().async` with `Task.detached`, capture `client` before entering. Replace `DispatchQueue.main.async` with `await MainActor.run`.

- [ ] **Step 3: Convert `disconnect()` (lines 255-293)**

```swift
func disconnect() {
    stopAudioPlayout()
    audioCapture?.stop()
    audioCapture = nil
    cameraCapture?.stop()
    cameraCapture = nil
    contextDetector?.stop()
    contextDetector = nil
    let sids = videoTrackSids
    let client = self.client
    Task.detached {
        for sid in sids {
            client.stopVideoRenderer(trackSid: sid)
        }
        client.disconnect()
        await MainActor.run { [weak self] in
            guard let self else { return }
            self.connectionState = .disconnected
            self.participants = []
            self.activeSpeakers = []
            self.chatMessages = []
            self.isMicEnabled = false
            self.isCameraEnabled = false
            self.isHandRaised = false
            self.handRaisedMap = [:]
            self.unreadCount = 0
            self.errorMessage = nil
            self.videoTrackSids = []
            self.isChatOpen = false
            self.waitingParticipants = []
            self.lobbyNotification = nil
            self.lobbyDenied = false
            self.reactions = []
            self.lastScreenShareParticipantSid = nil
        }
    }
    VideoFrameRouter.shared.clearAll()
}
```

- [ ] **Step 4: Convert `setMicEnabled()` (lines 300-331)**

```swift
func setMicEnabled(_ enabled: Bool) {
    let client = self.client
    Task.detached {
        do {
            if enabled {
                VisioManager.configureAudioSession()
            }
            try client.setMicrophoneEnabled(enabled: enabled)

            var capture: AudioCapture? = nil
            if enabled {
                capture = AudioCapture()
                capture?.start()
            }

            await MainActor.run { [weak self] in
                guard let self else { return }
                if enabled {
                    if self.audioCapture == nil {
                        self.audioCapture = capture
                    }
                } else {
                    self.audioCapture?.stop()
                    self.audioCapture = nil
                }
                self.isMicEnabled = enabled
            }
        } catch {
            await MainActor.run { [weak self] in
                self?.errorMessage = "Mic toggle failed: \(error.localizedDescription)"
            }
        }
    }
}
```

- [ ] **Step 5: Convert `setCameraEnabled()` (lines 418-442)**

Same pattern. Replace `DispatchQueue.global().async` → `Task.detached { [client] in ... }`, replace `DispatchQueue.main.async` → `await MainActor.run`.

- [ ] **Step 6: Convert remaining DispatchQueue methods**

Apply the same pattern to all remaining methods that use `DispatchQueue.global().async`:
- `toggleHandRaise()` (lines 444-460)
- `sendReaction()` (lines 462-485)
- `sendMessage()` (lines 495-511)
- `admitParticipant()` (lines 515-527)
- `denyParticipant()` (lines 529-541)
- `cancelLobby()` (lines 547-551)
- `initAuth()` (lines 555-571)
- `onAuthCookieReceived()` (lines 591-612)
- `logoutSession()` (lines 614-630)
- `refreshAccesses()` (lines 639-649)
- `addAccessMember()` (lines 651-658) — note: calls `refreshAccesses()` which is `@MainActor`. Inside `Task.detached`, use `await MainActor.run { self?.refreshAccesses() }` after the FFI call.
- `removeAccessMember()` (lines 661-668) — same note: `refreshAccesses()` call needs `await MainActor.run`
- `onAppForegrounded()` (lines 728-751) — only the `.disconnected` case

For each: capture `client` before `Task.detached`, use `await MainActor.run` for UI updates.

- [ ] **Step 7: Fix toggleCamera**

`toggleCamera()` (lines 404-416) uses `DispatchQueue.main.async` for an error message. Since we're now `@MainActor`, remove the `DispatchQueue.main.async` wrapper — just set `self.errorMessage` directly.

- [ ] **Step 8: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep -E "warning:|error:" | head -80
```

Expected: VisioManager should have significantly fewer warnings. May still have warnings from `onEvent`.

- [ ] **Step 9: Commit**

```bash
git add ios/VisioMobile/VisioManager.swift
git commit -m "refactor(ios): convert VisioManager DispatchQueue methods to Task.detached"
```

---

### Task 5: Migrate VisioManager — Part 3 (onEvent listener)

**Files:**
- Modify: `ios/VisioMobile/VisioManager.swift:799-1035`

- [ ] **Step 1: Make onEvent nonisolated**

The `VisioEventListener` protocol requires `Sendable` conformance. Since `VisioManager` is `@MainActor`, the `onEvent` method must be `nonisolated` (it's called from Rust threads):

```swift
extension VisioManager: VisioEventListener {
    nonisolated func onEvent(event: VisioEvent) {
        Task { @MainActor [weak self] in
            self?.handleEvent(event)
        }
    }
}
```

- [ ] **Step 2: Extract handleEvent as a private MainActor method**

Move the entire `switch event` body into a new private method:

```swift
// MARK: - Event Handling

private func handleEvent(_ event: VisioEvent) {
    switch event {
    // ... (entire existing switch body, unchanged)
    }
}
```

- [ ] **Step 3: Fix DispatchQueue calls inside handleEvent**

Inside `handleEvent`, there are nested `DispatchQueue.global().async` calls:
- `.trackUnmuted` case (lines 862-866): `DispatchQueue.global().async` to refresh participants
- `.trackSubscribed` case (lines 894-905): `DispatchQueue.global().async` to start renderer
- `.trackUnsubscribed` case (lines 911-913): `DispatchQueue.global().async` to stop renderer
- `.connectionLost` case (lines 1002-1011): `DispatchQueue.global().async` for reconnect
- `.adaptiveModeChanged` (line 977): `DispatchQueue.main.asyncAfter` for delayed camera toggle

Convert each to `Task.detached` with `client` capture:

For `.trackUnmuted` camera/screenShare:
```swift
case .camera, .screenShare:
    let client = self.client
    Task.detached {
        let updated = client.participants()
        await MainActor.run { [weak self] in
            self?.participants = updated
        }
    }
```

For `.trackSubscribed`:
```swift
let client = self.client
Task.detached {
    client.startVideoRenderer(trackSid: sid)
    let updated = client.participants()
    await MainActor.run { [weak self] in
        self?.participants = updated
        if isScreenShare {
            self?.lastScreenShareParticipantSid = nil
            self?.lastScreenShareParticipantSid = participantSid
        }
    }
}
```

For `.connectionLost`:
```swift
let client = self.client
Task.detached {
    do {
        try client.reconnect()
    } catch {
        await MainActor.run { [weak self] in
            self?.errorMessage = "Reconnection failed: \(error.localizedDescription)"
        }
    }
}
```

For `.adaptiveModeChanged` — replace `DispatchQueue.main.asyncAfter` with:
```swift
Task { @MainActor [weak self] in
    try? await Task.sleep(for: .seconds(self?.connectionGraceSeconds ?? 5))
    guard let self, self.adaptiveMode == .car else { return }
    self.cameraWasEnabledBeforeCar = self.isCameraEnabled
    if self.isCameraEnabled { self.toggleCamera() }
}
```

- [ ] **Step 4: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep -E "warning:|error:" | head -80
```

Expected: VisioManager should be clean or nearly clean.

- [ ] **Step 5: Commit**

```bash
git add ios/VisioMobile/VisioManager.swift
git commit -m "refactor(ios): migrate VisioManager onEvent to nonisolated + handleEvent"
```

---

### Task 6: Migrate OidcAuthManager to @MainActor

**Files:**
- Modify: `ios/VisioMobile/Auth/OidcAuthManager.swift`

- [ ] **Step 1: Add @MainActor annotation**

```swift
@MainActor
class OidcAuthManager: NSObject, ObservableObject, ASWebAuthenticationPresentationContextProviding {
```

- [ ] **Step 2: Remove all DispatchQueue.main.async wrappers**

Since the class is now `@MainActor`, all `DispatchQueue.main.async { ... }` blocks can be replaced with direct code. Find each occurrence (lines ~55, 60, 67, 76, 108) and inline the contents.

For `DispatchQueue.main.asyncAfter(deadline: .now() + 0.5)` (line 108), replace with:
```swift
Task { @MainActor in
    try? await Task.sleep(for: .milliseconds(500))
    // ... retry logic
}
```

- [ ] **Step 3: Make presentationAnchor nonisolated if needed**

Check if `ASWebAuthenticationPresentationContextProviding`'s `presentationAnchor(for:)` is called on an arbitrary queue. If so:
```swift
nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
    MainActor.assumeIsolated {
        // existing implementation accessing UIApplication.shared
    }
}
```

If the protocol method is already MainActor-annotated in the iOS 26 SDK, this is unnecessary.

- [ ] **Step 4: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep "OidcAuthManager" | head -20
```

- [ ] **Step 5: Commit**

```bash
git add ios/VisioMobile/Auth/OidcAuthManager.swift
git commit -m "refactor(ios): migrate OidcAuthManager to @MainActor"
```

---

### Task 7: Migrate VideoFrameRouter to @unchecked Sendable

**Files:**
- Modify: `ios/VisioMobile/VideoFrameRouter.swift`

- [ ] **Step 1: Add @unchecked Sendable conformance**

```swift
final class VideoFrameRouter: @unchecked Sendable {
```

This is justified: the `NSLock` provides correct manual synchronization. Do NOT change to `@MainActor` — this is a performance-critical video path.

- [ ] **Step 2: Verify DispatchQueue.main.async usage is compatible**

The `DispatchQueue.main.async` calls (lines 21, 81, 131) dispatch UI layer work to the main thread. These remain correct — they are dispatching to MainActor from a non-isolated context, which is the intended pattern.

No code changes needed beyond adding `@unchecked Sendable`.

- [ ] **Step 3: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep "VideoFrameRouter" | head -10
```

- [ ] **Step 4: Commit**

```bash
git add ios/VisioMobile/VideoFrameRouter.swift
git commit -m "refactor(ios): mark VideoFrameRouter @unchecked Sendable"
```

---

### Task 8: Migrate ContextDetector

**Files:**
- Modify: `ios/VisioMobile/Services/ContextDetector.swift`

- [ ] **Step 1: Add @MainActor annotation**

```swift
@MainActor
class ContextDetector: NSObject {
```

- [ ] **Step 2: Convert system callbacks to use Task { @MainActor in }**

The `pathUpdateHandler`, CoreMotion callback, and audio route notification are called from system queues. They need to hop back to MainActor for UI state and use `nonisolated` wrappers for FFI.

For `pathUpdateHandler` (around line 31):
```swift
pathMonitor.pathUpdateHandler = { [weak self] path in
    guard let self else { return }
    let networkType: NetworkType = // ... existing logic
    Task { @MainActor [weak self] in
        self?.manager.reportNetworkType(networkType)  // nonisolated wrapper
    }
}
```

Wait — ContextDetector accesses `VisioManager.shared` directly. Since VisioManager is `@MainActor`, accessing `.shared` from a non-MainActor context requires `await`. But the nonisolated wrapper methods on VisioManager access `client` directly without MainActor hop.

Pattern for system callbacks:
```swift
// For FFI calls (no MainActor needed):
VisioManager.shared.reportNetworkType(networkType)  // nonisolated func

// For UI state reads (MainActor needed):
Task { @MainActor in
    let state = VisioManager.shared.connectionState
    // ...
}
```

Note: accessing `VisioManager.shared` itself is fine from any context because `shared` is `static let` on a `@MainActor` class — the static property is accessible, but instance properties are isolated.

Actually, `VisioManager.shared` access requires MainActor in Swift 6. The nonisolated wrapper methods are on the instance, so we still need the instance reference. Solution: capture the manager reference at init time.

- [ ] **Step 3: Capture manager reference at init**

Add a stored property:
```swift
private let manager: VisioManager

init(manager: VisioManager = VisioManager.shared) {
    self.manager = manager
    super.init()
}
```

Then use `manager.reportNetworkType(...)` (nonisolated, no MainActor needed for FFI calls).

For `audioRouteChanged` which reads `connectionState`:
```swift
// Replace @objc notification observer pattern with queue-based observer
// in start() method, use:
NotificationCenter.default.addObserver(
    forName: AVAudioSession.routeChangeNotification,
    object: nil,
    queue: .main
) { [weak self] notification in
    guard let self else { return }
    guard case .connected = self.manager.connectionState else { return }
    // ... existing Bluetooth detection logic
    // For FFI calls, call nonisolated methods: self.manager.reportBluetoothCarKit()
}
```

- [ ] **Step 4: Remove DispatchQueue.main.async wrappers**

Since system callbacks now use `Task { @MainActor in }`, remove nested `DispatchQueue.main.async` calls and inline the code.

- [ ] **Step 5: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep "ContextDetector" | head -10
```

- [ ] **Step 6: Commit**

```bash
git add ios/VisioMobile/Services/ContextDetector.swift
git commit -m "refactor(ios): migrate ContextDetector to @MainActor with nonisolated FFI"
```

---

### Task 9: Migrate CallKitManager

**Files:**
- Modify: `ios/VisioMobile/Services/CallKitManager.swift`

- [ ] **Step 1: Add @MainActor annotation**

```swift
@MainActor
class CallKitManager: NSObject, CXProviderDelegate {
```

- [ ] **Step 2: Make CXProviderDelegate methods nonisolated**

`CXProviderDelegate` methods are called by the system on an internal queue. They must be `nonisolated`:

```swift
nonisolated func providerDidReset(_ provider: CXProvider) {
    // No-op or minimal cleanup
}

nonisolated func provider(_ provider: CXProvider, perform action: CXAnswerCallAction) {
    Task { @MainActor in
        // ... existing logic
        action.fulfill()
    }
}

nonisolated func provider(_ provider: CXProvider, perform action: CXEndCallAction) {
    Task { @MainActor in
        VisioManager.shared.disconnect()
        action.fulfill()
    }
}

nonisolated func provider(_ provider: CXProvider, perform action: CXSetMutedCallAction) {
    Task { @MainActor in
        VisioManager.shared.setMicEnabled(!action.isMuted)
        action.fulfill()
    }
}

nonisolated func provider(_ provider: CXProvider, didActivate audioSession: AVAudioSession) {
    // Audio session activation — no MainActor state needed
}

nonisolated func provider(_ provider: CXProvider, didDeactivate audioSession: AVAudioSession) {
    // Audio session deactivation — no MainActor state needed
}
```

Important: `action.fulfill()` is called **inside** the `Task { @MainActor in }` block, after the work completes.

- [ ] **Step 3: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep "CallKitManager" | head -10
```

- [ ] **Step 4: Commit**

```bash
git add ios/VisioMobile/Services/CallKitManager.swift
git commit -m "refactor(ios): migrate CallKitManager to @MainActor with nonisolated delegates"
```

---

### Task 10: Migrate PiPManager

**Files:**
- Modify: `ios/VisioMobile/Services/PiPManager.swift`

- [ ] **Step 1: Add @MainActor annotation**

```swift
@MainActor
class PiPManager: NSObject, AVPictureInPictureControllerDelegate {
```

- [ ] **Step 2: Make pushFrame nonisolated**

`pushFrame()` is called from VideoFrameRouter's video callback thread. It only enqueues a sample buffer — no MainActor state access:

```swift
nonisolated func pushFrame(_ pixelBuffer: CVPixelBuffer, timestamp: CMTime) {
    // existing implementation — just enqueues to AVSampleBufferDisplayLayer
}
```

- [ ] **Step 3: Check delegate methods**

`AVPictureInPictureControllerDelegate` and `AVPictureInPictureSampleBufferPlaybackDelegate` callbacks are called on the main thread by the system — compatible with `@MainActor`, no `nonisolated` needed.

- [ ] **Step 4: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep "PiPManager" | head -10
```

- [ ] **Step 5: Commit**

```bash
git add ios/VisioMobile/Services/PiPManager.swift
git commit -m "refactor(ios): migrate PiPManager to @MainActor with nonisolated pushFrame"
```

---

### Task 11: Migrate Audio/Camera/Media capture classes

**Files:**
- Modify: `ios/VisioMobile/AudioCapture.swift`
- Modify: `ios/VisioMobile/AudioPlayout.swift`
- Modify: `ios/VisioMobile/CameraCapture.swift`
- Modify: `ios/VisioMobile/SyntheticAudioCapture.swift`
- Modify: `ios/VisioMobile/MediaFileCapture.swift`

- [ ] **Step 1: Mark all capture classes @unchecked Sendable**

These classes manage their own thread safety via internal queues (`DispatchQueue`, `AVAudioEngine` internal queues). They should NOT be `@MainActor` — audio/video work happens on background threads.

```swift
final class AudioCapture: @unchecked Sendable {
final class AudioPlayout: @unchecked Sendable {
final class CameraCapture: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate, @unchecked Sendable {
final class SyntheticAudioCapture: @unchecked Sendable {
final class MediaFileCapture: @unchecked Sendable {
```

- [ ] **Step 2: Add @Sendable to closures passed to system APIs**

In AudioCapture, the `installTap` closure captures only value types and converter — verify it compiles without warnings.

In AudioPlayout, the `AVAudioSourceNode` render closure (line 75) captures `self` — since the class is now `@unchecked Sendable`, this should be fine.

In CameraCapture, the notification observer closure (line 109 `DispatchQueue.main.async`) should remain as-is.

- [ ] **Step 3: CameraCapture semaphore — verify it's safe**

The `DispatchSemaphore` in `CameraCapture.start()` (lines 27-33) runs on a private `queue` (not inside a Task), so it's safe to keep. Verify no compiler warning.

- [ ] **Step 4: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep -E "AudioCapture|AudioPlayout|CameraCapture|SyntheticAudio|MediaFile" | head -20
```

- [ ] **Step 5: Commit**

```bash
git add ios/VisioMobile/AudioCapture.swift ios/VisioMobile/AudioPlayout.swift ios/VisioMobile/CameraCapture.swift ios/VisioMobile/SyntheticAudioCapture.swift ios/VisioMobile/MediaFileCapture.swift
git commit -m "refactor(ios): mark capture classes @unchecked Sendable"
```

---

### Task 12: Migrate SwiftUI Views

**Files:**
- Modify: `ios/VisioMobile/Views/CallView.swift`
- Modify: `ios/VisioMobile/Views/HomeView.swift`
- Modify: `ios/VisioMobile/Views/InCallSettingsSheet.swift`
- Modify: `ios/VisioMobile/VisioMobileApp.swift`

- [ ] **Step 1: Fix CallView**

SwiftUI Views are implicitly `@MainActor` in Swift 6. The main issues will be:
- `DispatchQueue.global().async` inside `.task` or `onChange` — convert to `Task.detached`
- Access to `VisioManager` methods that are now `@MainActor` — already fine from SwiftUI views

Check for and fix any `DispatchQueue` usage remaining in CallView. The `.task` modifiers and `onChange` handlers should work as-is since they run on MainActor.

- [ ] **Step 2: Fix HomeView**

The `DispatchQueue` calls in HomeView (lines 418, 629, 632, 638, 692, 705, 711) need conversion:
- `DispatchQueue.global().async` for user search → `Task.detached` with `await MainActor.run` for UI updates
- `DispatchQueue.global().async` for room creation → same pattern
- `DispatchQueue.main.async` in cookie callback → remove wrapper (already on MainActor)

The `WKNavigationDelegate` Coordinator may need `@MainActor` annotation or `nonisolated` methods depending on how WKWebView calls delegates.

- [ ] **Step 3: Fix InCallSettingsSheet**

Same pattern: convert `DispatchQueue.global().async` (lines 186, 505) to `Task.detached`, remove `DispatchQueue.main.async` wrappers.

- [ ] **Step 4: Fix VisioMobileApp**

Update the deep link handler to construct `TestConnectParams` instead of a tuple.

Check `@ObservedObject private var manager = VisioManager.shared` — this should work since `VisioMobileApp` is implicitly `@MainActor` as a SwiftUI `App`.

- [ ] **Step 5: Build and check**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep -E "warning:|error:" | head -40
```

Expected: Should be close to zero warnings.

- [ ] **Step 6: Commit**

```bash
git add ios/VisioMobile/Views/CallView.swift ios/VisioMobile/Views/HomeView.swift ios/VisioMobile/Views/InCallSettingsSheet.swift ios/VisioMobile/VisioMobileApp.swift
git commit -m "refactor(ios): migrate SwiftUI views for strict concurrency"
```

---

### Note: VisioFFI+Sendable.swift is NOT needed

The spec mentions creating `VisioFFI+Sendable.swift`. This is **unnecessary** because the UniFFI-generated `visio.swift` already includes:
- `extension ChatMessage: Sendable {}` (and similar for all value types) via `#if compiler(>=6)`
- `open class VisioClient: VisioClientProtocol, @unchecked Sendable`
- `public protocol VisioEventListener: AnyObject, Sendable`

No additional Sendable extensions are needed.

---

### Task 13: Fix remaining low-impact files

**Files:**
- Modify: `ios/VisioMobile/Views/ChatView.swift` (if warnings)
- Modify: `ios/VisioMobile/Views/ParticipantListSheet.swift` (if warnings)
- Modify: `ios/VisioMobile/Views/SettingsView.swift` (if warnings)
- Modify: `ios/VisioMobile/Views/VideoLayerView.swift` (if warnings)
- Modify: `ios/VisioMobile/NV12Util.swift` (if warnings)
- Modify: `ios/VisioMobile/Theme.swift` (if warnings)
- Modify: `ios/VisioMobile/ToolbarFix.swift` (if warnings)
- Modify: `ios/VisioMobile/Views/VisioLogo.swift` (if warnings)
- Modify: `ios/VisioMobile/i18n/Strings.swift` (if warnings)

- [ ] **Step 1: Build and capture remaining warnings**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep "warning:" | head -40
```

- [ ] **Step 2: Fix each warning**

These files have minimal concurrency surface. Common fixes:
- `@Sendable` on closures if needed
- `Sendable` conformance on simple structs
- Import fixes for `@preconcurrency import` if a framework isn't yet Swift 6-ready (but we aim to avoid `@preconcurrency`)

- [ ] **Step 3: Build and verify zero warnings**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep -c "warning:"
```

Expected: `0`

- [ ] **Step 4: Commit**

```bash
git add ios/
git commit -m "refactor(ios): fix remaining strict concurrency warnings"
```

---

### Task 14: Flip to Swift 6 (Phase 3)

**Files:**
- Modify: `ios/VisioMobile.xcodeproj/project.pbxproj`

- [ ] **Step 1: Change SWIFT_VERSION to 6.0**

In `project.pbxproj`, replace all occurrences of `SWIFT_VERSION = 5.0;` with `SWIFT_VERSION = 6.0;` in every build configuration block.

- [ ] **Step 2: Build in Swift 6 mode**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' -quiet build 2>&1 | grep -E "warning:|error:" | head -40
```

Expected: Zero errors, zero warnings. If any new errors appear (Swift 6 is stricter than `-strict-concurrency=complete` in some edge cases), fix them.

- [ ] **Step 3: Verify deployment target unchanged**

```bash
grep "IPHONEOS_DEPLOYMENT_TARGET" /Users/mmaudet/work/visio-mobile-v2/ios/VisioMobile.xcodeproj/project.pbxproj
```

Expected: All occurrences show `16.0`.

- [ ] **Step 4: Commit**

```bash
git add ios/VisioMobile.xcodeproj/project.pbxproj
git commit -m "build(ios): switch to Swift 6 language mode"
```

---

### Task 15: Final validation

- [ ] **Step 1: Clean build**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/ios && xcodebuild -project VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' clean build 2>&1 | tail -5
```

Expected: `** BUILD SUCCEEDED **`

- [ ] **Step 2: Run Rust tests**

```bash
cd /Users/mmaudet/work/visio-mobile-v2 && cargo test -p visio-core 2>&1 | tail -5
```

Expected: All 48 tests pass.

- [ ] **Step 3: Verify no @preconcurrency or nonisolated(unsafe) in codebase**

```bash
grep -r "@preconcurrency\|nonisolated(unsafe)" /Users/mmaudet/work/visio-mobile-v2/ios/VisioMobile/ --include="*.swift" | grep -v Generated/
```

Expected: No matches.

- [ ] **Step 4: Verify @unchecked Sendable usage is limited to justified cases**

```bash
grep -r "@unchecked Sendable" /Users/mmaudet/work/visio-mobile-v2/ios/VisioMobile/ --include="*.swift" | grep -v Generated/
```

Expected: Only VideoFrameRouter, AudioCapture, AudioPlayout, CameraCapture, SyntheticAudioCapture, MediaFileCapture.

- [ ] **Step 5: Manual smoke test (per spec test plan)**

Test on a real device or simulator:
- Connect to a room, verify audio/video
- Toggle camera and microphone
- Send/receive chat messages
- PiP mode (enter/exit)
- CallKit integration (incoming/outgoing)
- Adaptive context mode changes
- Disconnect and reconnect
- Backward compatibility: verify on iOS 16 simulator or iPhone 12 tester device

- [ ] **Step 6: Create GitHub issue for GlassEffect follow-up**

Create an issue to track the visual GlassEffect/Liquid Glass adoption (explicitly out of scope for this PR):

```bash
gh issue create --title "feat(ios): adopt Liquid Glass / GlassEffect for iOS 26" --body "$(cat <<'EOF'
## Context

Issue #41 was split into two parts:
1. **Xcode 26 + Swift 6 migration** (done in this PR) — compile with Xcode 26 in Swift 6 strict concurrency mode, iOS 16.0 deployment target maintained
2. **Liquid Glass visual adoption** (this issue) — use `@available(iOS 26, *)` to apply GlassEffect on iOS 26 devices

## Scope

- Apply GlassEffect to navigation bars, toolbars, and in-call overlays
- Use `@available(iOS 26, *)` guards with fallback to current style for iOS 16-18
- Consider adaptive design for both glass and non-glass devices

## Prerequisites

- Swift 6 migration PR merged (issue #41)
EOF
)"
```

- [ ] **Step 7: Commit any final fixes if needed**

```bash
git add ios/ && git commit -m "build(ios): final Swift 6 strict concurrency cleanup"
```
