import AVFoundation
import Foundation
import SwiftUI
import UserNotifications
import visioFFI

struct TestConnectParams: Sendable {
    let livekitUrl: String
    let token: String
    let mediaFile: String?
}

/// Central state manager for the Visio app, backed by UniFFI-generated VisioClient.
/// Conforms to VisioEventListener to receive room events from Rust.
@MainActor
class VisioManager: ObservableObject {

    // MARK: - Shared singleton (for CallKit access)

    static let shared = VisioManager()

    // MARK: - Published state

    @Published var connectionState: ConnectionState = .disconnected
    @Published var participants: [ParticipantInfo] = []
    @Published var activeSpeakers: [String] = []
    @Published var chatMessages: [ChatMessage] = []
    @Published var isMicEnabled: Bool = false
    @Published var isCameraEnabled: Bool = false
    @Published var isHandRaised: Bool = false
    @Published var handRaisedMap: [String: Int] = [:]  // sid -> position
    @Published var unreadCount: Int = 0
    @Published var errorMessage: String?
    @Published var videoTrackSids: [String] = []
    @Published var isChatOpen: Bool = false
    @Published var currentLang: String = "fr"
    @Published var currentTheme: String = "light"
    @Published var displayName: String = ""
    @Published var pendingDeepLink: String? = nil
    /// For E2E testing: (livekitUrl, token, mediaFile?) from visio-test:// deep link.
    /// Only used in DEBUG builds.
    @Published var pendingTestConnect: TestConnectParams? = nil
    @Published var isFrontCamera: Bool = true
    @Published var waitingParticipants: [WaitingParticipant] = []
    @Published var lobbyNotification: WaitingParticipant? = nil
    @Published var lobbyDenied: Bool = false
    @Published var roomAccesses: [RoomAccess] = []
    var currentRoomId: String?
    var currentAccessLevel: String = ""
    @Published var isAuthenticated: Bool = false
    @Published var authenticatedDisplayName: String = ""
    @Published var authenticatedEmail: String = ""
    @Published var authenticatedMeetInstance: String = ""
    @Published var backgroundMode: String = "off"
    @Published var reactions: [ReactionData] = []
    @Published var adaptiveMode: AdaptiveMode = .office
    /// Set when a screen share track is subscribed; cleared on disconnect.
    @Published var lastScreenShareParticipantSid: String? = nil
    @Published var upcomingMeetings: [Meeting] = []
    @Published var calendarLoading: Bool = false

    let authManager = OidcAuthManager()

    // MARK: - Private

    nonisolated let client: VisioClient
    private var audioPlayout: AudioPlayout?
    private var audioCapture: AudioCapture?
    private var cameraCapture: CameraCapture?
    private var syntheticAudio: SyntheticAudioCapture?
    private var mediaFileCapture: MediaFileCapture?
    private var contextDetector: ContextDetector?
    private var reactionIdCounter: Int64 = 0
    private var cameraWasEnabledBeforeCar = false
    private var connectionTimestamp: Date?
    private let connectionGraceSeconds: TimeInterval = 5.0

    // MARK: - Init

    init() {
        // VisioClient() creates a tokio runtime -- acceptable to block on main thread at launch.
        let documentsDir: URL
        if let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first {
            documentsDir = dir
        } else {
            NSLog("VisioManager: documents directory unavailable, using temp directory")
            documentsDir = FileManager.default.temporaryDirectory
        }
        client = VisioClient(dataDir: documentsDir.path)
        client.addListener(listener: self)

        // Load persisted settings
        let settings = client.getSettings()
        currentLang = settings.language ?? "fr"
        currentTheme = settings.theme
        displayName = settings.displayName ?? ""

        // Register the video frame callback so Rust can deliver I420 frames to Swift.
        visio_video_set_ios_callback({ width, height, yPtr, yStride, uPtr, uStride, vPtr, vStride, trackSidCStr, userData in
            guard let yPtr, let uPtr, let vPtr, let trackSidCStr else { return }
            let trackSid = String(cString: trackSidCStr)
            VideoFrameRouter.shared.deliverFrame(
                width: width, height: height,
                yPtr: yPtr, yStride: yStride,
                uPtr: uPtr, uStride: uStride,
                vPtr: vPtr, vStride: vStride,
                trackSid: trackSid
            )
        }, nil)

        // Load ONNX segmentation model for background blur
        if let modelUrl = Bundle.main.url(forResource: "selfie_segmentation", withExtension: "onnx") {
            do {
                try client.loadBlurModel(modelPath: modelUrl.path)
                NSLog("VisioManager: blur model loaded")
            } catch {
                NSLog("VisioManager: failed to load blur model: \(error)")
            }
        } else {
            NSLog("VisioManager: selfie_segmentation.onnx not found in bundle")
        }

        // Observe Bluetooth audio device disconnection so we can log the
        // iOS automatic fallback to built-in speaker.
        NotificationCenter.default.addObserver(
            forName: AVAudioSession.routeChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let reason = notification.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
                  reason == AVAudioSession.RouteChangeReason.oldDeviceUnavailable.rawValue else {
                return
            }
            _ = self  // capture self to silence unused-capture warning
            print("Audio route changed: Bluetooth device disconnected, iOS auto-fallback to built-in")
        }
    }

    // MARK: - Public API

    func connect(url: String, username: String?) {
        // Set connecting state immediately so CallView never renders the
        // "Disconnected" banner before the async event arrives from Rust.
        self.connectionState = .connecting
        self.errorMessage = nil

        let client = self.client
        Task.detached {
            do {
                let settings = client.getSettings()

                let cameraNeeded = settings.cameraEnabledOnJoin || client.isCameraEnabled()
                await Self.ensureMediaPermissions(mic: settings.micEnabledOnJoin, camera: cameraNeeded)

                if settings.micEnabledOnJoin {
                    Self.configureAudioSession()
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

                var audioCapture: AudioCapture?
                if mic {
                    let capture = AudioCapture()
                    capture.start()
                    audioCapture = capture
                }

                var cameraCapture: CameraCapture?
                if cam {
                    let capture = CameraCapture()
                    capture.start()
                    cameraCapture = capture
                }

                await MainActor.run { [weak self] in
                    self?.audioCapture = audioCapture
                    self?.participants = parts
                    self?.isMicEnabled = mic
                    self?.isCameraEnabled = cam
                    self?.chatMessages = msgs
                    self?.connectionState = state
                    self?.isHandRaised = hand
                    self?.errorMessage = nil
                    self?.cameraCapture = cameraCapture

                    // Start audio playout now that connection is established
                    self?.startAudioPlayout()

                    // Start context detection for adaptive modes
                    self?.startContextDetection()
                }
            } catch {
                await MainActor.run { [weak self] in
                    self?.errorMessage = "Connection failed: \(error.localizedDescription)"
                }
            }
        }
    }

    func connectWithToken(livekitUrl: String, token: String) {
        self.connectionState = .connecting
        self.errorMessage = nil

        let client = self.client
        Task.detached {
            do {
                let settings = client.getSettings()

                let cameraNeeded = settings.cameraEnabledOnJoin || client.isCameraEnabled()
                await Self.ensureMediaPermissions(mic: settings.micEnabledOnJoin, camera: cameraNeeded)

                try client.connectWithToken(livekitUrl: livekitUrl, token: token)

                if settings.micEnabledOnJoin {
                    try client.setMicrophoneEnabled(enabled: true)
                }
                if cameraNeeded {
                    try client.setCameraEnabled(enabled: true)
                }

                let parts = client.participants()
                let mic = client.isMicrophoneEnabled()
                let cam = client.isCameraEnabled()
                let state = client.connectionState()

                var audioCapture: AudioCapture?
                if mic {
                    let capture = AudioCapture()
                    capture.start()
                    audioCapture = capture
                }

                var cameraCapture: CameraCapture?
                if cam {
                    let capture = CameraCapture()
                    capture.start()
                    cameraCapture = capture
                }

                await MainActor.run { [weak self] in
                    self?.audioCapture = audioCapture
                    self?.participants = parts
                    self?.isMicEnabled = mic
                    self?.isCameraEnabled = cam
                    self?.connectionState = state
                    self?.errorMessage = nil
                    self?.cameraCapture = cameraCapture
                    // Start audio playout now that connection is established
                    self?.startAudioPlayout()
                }
            } catch {
                await MainActor.run { [weak self] in
                    self?.errorMessage = "Test connect failed: \(error.localizedDescription)"
                }
            }
        }
    }

    func startContextDetection() {
        guard client.isAdaptiveModeEnabled() else {
            NSLog("VisioManager: adaptive mode disabled, skipping context detection")
            return
        }
        let detector = ContextDetector()
        detector.start()
        contextDetector = detector
    }

    func stopContextDetection() {
        contextDetector?.stop()
        contextDetector = nil
        adaptiveMode = .office
        client.setAdaptiveModeOverride(mode: .office)
    }

    func disconnect() {
        stopAudioPlayout()
        audioCapture?.stop()
        audioCapture = nil
        cameraCapture?.stop()
        cameraCapture = nil
        contextDetector?.stop()
        contextDetector = nil
        let sids = videoTrackSids
        VideoFrameRouter.shared.clearAll()
        let client = self.client
        Task.detached {
            for sid in sids {
                client.stopVideoRenderer(trackSid: sid)
            }
            client.disconnect()
            await MainActor.run { [weak self] in
                self?.connectionState = .disconnected
                self?.participants = []
                self?.activeSpeakers = []
                self?.chatMessages = []
                self?.isMicEnabled = false
                self?.isCameraEnabled = false
                self?.isHandRaised = false
                self?.handRaisedMap = [:]
                self?.unreadCount = 0
                self?.errorMessage = nil
                self?.videoTrackSids = []
                self?.isChatOpen = false
                self?.waitingParticipants = []
                self?.lobbyNotification = nil
                self?.lobbyDenied = false
                self?.reactions = []
                self?.lastScreenShareParticipantSid = nil
            }
        }
    }

    func toggleMic() {
        let newValue = !isMicEnabled
        setMicEnabled(newValue)
    }

    func setMicEnabled(_ enabled: Bool) {
        let client = self.client
        Task.detached {
            do {
                if enabled {
                    // Configure audio session before enabling mic
                    Self.configureAudioSession()
                }
                try client.setMicrophoneEnabled(enabled: enabled)

                var newCapture: AudioCapture?
                if enabled {
                    // Start mic capture on background thread (AVAudioEngine must not start on main)
                    let capture = AudioCapture()
                    capture.start()
                    newCapture = capture
                }

                await MainActor.run { [weak self] in
                    if enabled {
                        self?.audioCapture?.stop()
                        self?.audioCapture = newCapture
                    } else {
                        self?.audioCapture?.stop()
                        self?.audioCapture = nil
                    }
                    self?.isMicEnabled = enabled
                }
            } catch {
                await MainActor.run { [weak self] in
                    self?.errorMessage = "Mic toggle failed: \(error.localizedDescription)"
                }
            }
        }
    }

    /// Start synthetic audio capture (440Hz sine wave for E2E testing on simulators).
    func startSyntheticAudio() {
        guard syntheticAudio == nil else { return }
        let capture = SyntheticAudioCapture()
        capture.start()
        syntheticAudio = capture
    }

    /// Stop synthetic audio capture.
    func stopSyntheticAudio() {
        syntheticAudio?.stop()
        syntheticAudio = nil
    }

    /// Start media file capture (audio + video from MP4) for E2E testing.
    func startMediaFileCapture(_ filePath: String) {
        guard mediaFileCapture == nil else { return }
        let capture = MediaFileCapture(filePath: filePath)
        capture.startAudio()
        capture.startVideo()
        mediaFileCapture = capture
    }

    /// Stop media file capture.
    func stopMediaFileCapture() {
        mediaFileCapture?.stopAudio()
        mediaFileCapture?.stopVideo()
        mediaFileCapture = nil
    }

    /// Request camera and/or microphone permissions before using them.
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

    /// Configure AVAudioSession for voice chat (play + record).
    /// Preserves the current Bluetooth route if one is active.
    nonisolated static func configureAudioSession() {
        let session = AVAudioSession.sharedInstance()
        let previousBtInput = session.currentRoute.inputs.first {
            [.bluetoothHFP, .bluetoothA2DP, .bluetoothLE].contains($0.portType)
        }
        do {
            try session.setCategory(.playAndRecord, mode: .voiceChat, options: [.defaultToSpeaker, .allowBluetooth, .allowBluetoothA2DP])
            try session.setPreferredSampleRate(48_000)
            try session.setActive(true)
            if let btInput = previousBtInput,
               let matchingInput = session.availableInputs?.first(where: { $0.portType == btInput.portType }) {
                try session.setPreferredInput(matchingInput)
                NSLog("VisioManager: restored Bluetooth input: %@", matchingInput.portName)
            }
            NSLog("VisioManager: audio session configured (.playAndRecord)")
        } catch {
            NSLog("VisioManager: audio session config failed: %@", error.localizedDescription)
        }
    }

    func toggleCamera() {
        // Ensure camera permission before enabling
        if !isCameraEnabled {
            let status = AVCaptureDevice.authorizationStatus(for: .video)
            if status == .denied || status == .restricted {
                self.errorMessage = "Camera permission denied. Enable in Settings → Visio Mobile → Camera."
                return
            }
        }
        setCameraEnabled(!isCameraEnabled)
    }

    func setCameraEnabled(_ enabled: Bool) {
        let client = self.client
        Task.detached {
            do {
                try client.setCameraEnabled(enabled: enabled)
                let parts = client.participants()

                var cameraCapture: CameraCapture?
                if enabled {
                    let capture = CameraCapture()
                    capture.start()
                    cameraCapture = capture
                }

                await MainActor.run { [weak self] in
                    self?.isCameraEnabled = enabled
                    self?.participants = parts
                    self?.cameraCapture?.stop()
                    self?.cameraCapture = enabled ? cameraCapture : nil
                }
            } catch {
                await MainActor.run { [weak self] in
                    self?.errorMessage = "Camera toggle failed: \(error.localizedDescription)"
                }
            }
        }
    }

    func toggleHandRaise() {
        let shouldRaise = !isHandRaised
        let client = self.client
        Task.detached {
            do {
                if shouldRaise {
                    try client.raiseHand()
                } else {
                    try client.lowerHand()
                }
            } catch {
                await MainActor.run { [weak self] in
                    self?.errorMessage = "Hand raise failed: \(error.localizedDescription)"
                }
            }
        }
    }

    func sendReaction(_ emoji: String) {
        let client = self.client
        Task.detached {
            do {
                try client.sendReaction(emoji: emoji)
                let displayName = client.getSettings().displayName ?? ""
                // Show reaction locally (server echo is filtered out in Rust)
                await MainActor.run { [weak self] in
                    guard let self else { return }
                    let reaction = ReactionData(
                        id: self.reactionIdCounter,
                        participantSid: "local",
                        participantName: displayName,
                        emoji: emoji,
                        timestamp: Date()
                    )
                    self.reactionIdCounter += 1
                    self.reactions.append(reaction)
                }
            } catch {
                await MainActor.run { [weak self] in
                    self?.errorMessage = "Reaction failed: \(error.localizedDescription)"
                }
            }
        }
    }

    func setChatOpen(_ open: Bool) {
        isChatOpen = open
        client.setChatOpen(open: open)
        if open {
            unreadCount = 0
        }
    }

    func sendMessage(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let client = self.client
        Task.detached {
            do {
                let msg = try client.sendChatMessage(text: trimmed)
                await MainActor.run { [weak self] in
                    self?.chatMessages.append(msg)
                }
            } catch {
                await MainActor.run { [weak self] in
                    self?.errorMessage = "Send failed: \(error.localizedDescription)"
                }
            }
        }
    }

    // MARK: - Lobby

    func admitParticipant(_ id: String) {
        let client = self.client
        Task.detached {
            do {
                try client.admitParticipant(participantId: id)
                await MainActor.run { [weak self] in
                    self?.waitingParticipants.removeAll { $0.id == id }
                }
            } catch {
                NSLog("VisioManager: admit failed: \(error)")
            }
        }
    }

    func denyParticipant(_ id: String) {
        let client = self.client
        Task.detached {
            do {
                try client.denyParticipant(participantId: id)
                await MainActor.run { [weak self] in
                    self?.waitingParticipants.removeAll { $0.id == id }
                }
            } catch {
                NSLog("VisioManager: deny failed: \(error)")
            }
        }
    }

    func clearLobbyNotification() {
        lobbyNotification = nil
    }

    func cancelLobby() {
        let client = self.client
        Task.detached {
            client.cancelLobby()
        }
    }

    // MARK: - Authentication

    func initAuth() {
        guard let cookie = authManager.getSavedCookie(),
              let meetInstance = client.getMeetInstances().first else { return }

        let client = self.client
        let authManager = self.authManager
        Task.detached {
            do {
                try client.authenticate(meetUrl: "https://\(meetInstance)", cookie: cookie)
                let state = client.getSessionState()
                await MainActor.run { [weak self] in
                    self?.updateSessionFromState(state)
                }
            } catch {
                await MainActor.run {
                    authManager.clearCookie()
                }
            }
        }
    }

    private func updateSessionFromState(_ state: SessionState) {
        switch state {
        case .authenticated(let displayName, let email, let meetInstance):
            isAuthenticated = true
            authenticatedDisplayName = displayName
            authenticatedEmail = email
            authenticatedMeetInstance = meetInstance
            if self.displayName.isEmpty {
                self.displayName = displayName
            }
        case .anonymous:
            isAuthenticated = false
            authenticatedDisplayName = ""
            authenticatedEmail = ""
            authenticatedMeetInstance = ""
        }
    }

    func onAuthCookieReceived(_ cookie: String, meetInstance: String) {
        authManager.saveCookie(cookie)
        // Auto-add the instance to saved Meet instances
        var instances = client.getMeetInstances()
        if !instances.contains(meetInstance) {
            instances.append(meetInstance)
            client.setMeetInstances(instances: instances)
        }

        let client = self.client
        let authManager = self.authManager
        Task.detached {
            do {
                try client.authenticate(meetUrl: "https://\(meetInstance)", cookie: cookie)
                let state = client.getSessionState()
                await MainActor.run { [weak self] in
                    self?.updateSessionFromState(state)
                }
            } catch {
                await MainActor.run {
                    authManager.clearCookie()
                }
            }
        }
    }

    func logoutSession() {
        let instance = authenticatedMeetInstance.isEmpty
            ? client.getMeetInstances().first ?? ""
            : authenticatedMeetInstance
        guard !instance.isEmpty else { return }
        let client = self.client
        let authManager = self.authManager
        Task.detached {
            try? client.logout(meetUrl: "https://\(instance)")
            await MainActor.run { [weak self] in
                authManager.clearCookie()
                self?.isAuthenticated = false
                self?.authenticatedDisplayName = ""
                self?.authenticatedEmail = ""
                self?.authenticatedMeetInstance = ""
            }
        }
    }

    // MARK: - Access Management

    func setCurrentRoom(roomId: String?, accessLevel: String) {
        currentRoomId = roomId
        currentAccessLevel = accessLevel
    }

    func refreshAccesses() {
        guard let roomId = currentRoomId else { return }
        let client = self.client
        Task.detached {
            do {
                let accesses = try client.listAccesses(roomId: roomId)
                await MainActor.run { [weak self] in
                    self?.roomAccesses = accesses
                }
            } catch {
                NSLog("VisioManager: refreshAccesses failed: %@", error.localizedDescription)
            }
        }
    }

    func addAccessMember(userId: String) {
        guard let roomId = currentRoomId else { return }
        let client = self.client
        Task.detached {
            do {
                _ = try client.addAccess(userId: userId, roomId: roomId)
                await MainActor.run { [weak self] in
                    self?.refreshAccesses()
                }
            } catch {
                NSLog("VisioManager: addAccessMember failed: %@", error.localizedDescription)
            }
        }
    }

    func removeAccessMember(accessId: String) {
        let client = self.client
        Task.detached {
            do {
                try client.removeAccess(accessId: accessId)
                await MainActor.run { [weak self] in
                    self?.refreshAccesses()
                }
            } catch {
                NSLog("VisioManager: removeAccessMember failed: %@", error.localizedDescription)
            }
        }
    }

    // MARK: - Settings

    func getSettings() -> Settings {
        return client.getSettings()
    }

    func setDisplayName(_ name: String?) {
        client.setDisplayName(name: name)
    }

    func setLanguage(_ lang: String?) {
        if let lang { currentLang = lang }
        client.setLanguage(lang: lang)
    }

    func setMicEnabledOnJoin(_ enabled: Bool) {
        client.setMicEnabledOnJoin(enabled: enabled)
    }

    func setCameraEnabledOnJoin(_ enabled: Bool) {
        client.setCameraEnabledOnJoin(enabled: enabled)
    }

    func setTheme(_ theme: String) {
        currentTheme = theme
        client.setTheme(theme: theme)
    }

    func updateDisplayName(_ name: String) {
        displayName = name
    }

    func switchCamera(toFront: Bool) {
        cameraCapture?.switchCamera(toFront: toFront)
        isFrontCamera = toFront
    }

    func refreshCalendarNow() {
        let client = self.client
        calendarLoading = true
        Task.detached {
            client.refreshCalendarNow()
        }
    }

    func requestNotificationPermissionIfNeeded() {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { granted, error in
            if let error {
                NSLog("VisioManager: notification permission error: %@", error.localizedDescription)
            } else {
                NSLog("VisioManager: notification permission %@", granted ? "granted" : "denied")
            }
        }
    }

    private func scheduleMeetingNotification(title: String, body: String, identifier: String) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default

        let trigger = UNTimeIntervalNotificationTrigger(timeInterval: 1, repeats: false)
        let request = UNNotificationRequest(identifier: identifier, content: content, trigger: trigger)
        UNUserNotificationCenter.current().add(request) { error in
            if let error {
                NSLog("VisioManager: notification scheduling failed: %@", error.localizedDescription)
            }
        }
    }

    func setNotificationParticipantJoin(_ enabled: Bool) {
        client.setNotificationParticipantJoin(enabled: enabled)
    }

    func setNotificationHandRaised(_ enabled: Bool) {
        client.setNotificationHandRaised(enabled: enabled)
    }

    func setNotificationMessageReceived(_ enabled: Bool) {
        client.setNotificationMessageReceived(enabled: enabled)
    }

    // MARK: - Nonisolated FFI wrappers (for use from non-MainActor contexts)

    nonisolated func reportNetworkType(_ type: NetworkType) {
        client.reportNetworkType(networkType: type)
    }

    nonisolated func reportMotionDetected(_ detected: Bool) {
        client.reportMotionDetected(detected: detected)
    }

    nonisolated func reportBluetoothCarKit(connected: Bool) {
        client.reportBluetoothCarKit(connected: connected)
    }

    // MARK: - Lifecycle

    func onAppBackgrounded() {
        guard case .connected = connectionState else { return }
        cameraCapture?.stop()
        cameraCapture = nil
        stopAudioPlayout()
    }

    func onAppForegrounded() {
        switch connectionState {
        case .connected:
            startAudioPlayout()
            if isCameraEnabled {
                let capture = CameraCapture()
                capture.start()
                cameraCapture = capture
            }
        case .disconnected:
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
        default:
            break
        }
    }

    // MARK: - Audio Playout

    func startAudioPlayout() {
        guard audioPlayout == nil else { return }
        let playout = AudioPlayout()
        playout.start()
        audioPlayout = playout
    }

    func stopAudioPlayout() {
        audioPlayout?.stop()
        audioPlayout = nil
    }

    // MARK: - Audio Routing

    func routeAudioToBluetooth() {
        let session = AVAudioSession.sharedInstance()
        if let btInput = session.availableInputs?.first(where: { port in
            port.portType == .bluetoothHFP || port.portType == .bluetoothA2DP
        }) {
            do {
                try session.setPreferredInput(btInput)
                print("[VisioManager] Routed audio input to Bluetooth: \(btInput.portName)")
            } catch {
                print("[VisioManager] Failed to route audio to Bluetooth: \(error)")
            }
        }
        do {
            try session.overrideOutputAudioPort(.none)
        } catch {
            print("[VisioManager] Failed to set output override: \(error)")
        }
    }

    func restoreDefaultAudioRoute() {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setPreferredInput(nil)
            print("[VisioManager] Restored default audio input")
        } catch {
            print("[VisioManager] Failed to restore audio input: \(error)")
        }
    }

    // MARK: - Event Handling

    private func handleEvent(_ event: VisioEvent) {
        switch event {
        case .connectionStateChanged(let state):
            // Ignore stale .connecting events that arrive after we're already .connected
            // (race between sync state read in connect() and async event dispatch)
            if case .connecting = state, case .connected = self.connectionState {
                break
            }
            self.connectionState = state
            if case .connected = state {
                self.connectionTimestamp = Date()
            }

        case .participantJoined(let info):
            if let idx = self.participants.firstIndex(where: { $0.sid == info.sid }) {
                self.participants[idx] = info
            } else {
                self.participants.append(info)
            }

        case .participantLeft(let sid):
            self.participants.removeAll { $0.sid == sid }
            self.handRaisedMap.removeValue(forKey: sid)

        case .trackMuted(let sid, let source):
            if let idx = self.participants.firstIndex(where: { $0.sid == sid }) {
                var p = self.participants[idx]
                switch source {
                case .microphone:
                    p.isMuted = true
                case .camera:
                    if let trackSid = p.videoTrackSid {
                        VideoFrameRouter.shared.invalidateTrack(trackSid: trackSid)
                    }
                    p.hasVideo = false
                    p.videoTrackSid = nil
                case .screenShare:
                    if let trackSid = p.screenShareTrackSid {
                        VideoFrameRouter.shared.invalidateTrack(trackSid: trackSid)
                    }
                    p.hasScreenShare = false
                    p.screenShareTrackSid = nil
                case .unknown:
                    break
                }
                self.participants[idx] = p
            }

        case .trackUnmuted(let sid, let source):
            switch source {
            case .microphone:
                if let idx = self.participants.firstIndex(where: { $0.sid == sid }) {
                    var p = self.participants[idx]
                    p.isMuted = false
                    self.participants[idx] = p
                }
            case .camera, .screenShare:
                let client = self.client
                Task.detached {
                    let updated = client.participants()
                    await MainActor.run { [weak self] in
                        self?.participants = updated
                    }
                }
            case .unknown:
                break
            }

        case .activeSpeakersChanged(let sids):
            self.activeSpeakers = sids

        case .connectionQualityChanged(let sid, let quality):
            if let idx = self.participants.firstIndex(where: { $0.sid == sid }) {
                var p = self.participants[idx]
                p.connectionQuality = quality
                self.participants[idx] = p
            }

        case .chatMessageReceived(let message):
            if !self.chatMessages.contains(where: { $0.id == message.id }) {
                self.chatMessages.append(message)
            }

        case .trackSubscribed(let info):
            if info.kind == .video {
                let sid = info.sid
                if !self.videoTrackSids.contains(sid) {
                    self.videoTrackSids.append(sid)
                }
                let participantSid = info.participantSid
                let isScreenShare = info.source == .screenShare
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
            }

        case .trackUnsubscribed(let trackSid):
            self.videoTrackSids.removeAll { $0 == trackSid }
            VideoFrameRouter.shared.invalidateTrack(trackSid: trackSid)
            let client = self.client
            Task.detached {
                client.stopVideoRenderer(trackSid: trackSid)
            }
            if let idx = self.participants.firstIndex(where: {
                $0.videoTrackSid == trackSid || $0.screenShareTrackSid == trackSid
            }) {
                var p = self.participants[idx]
                if p.videoTrackSid == trackSid {
                    p.hasVideo = false
                    p.videoTrackSid = nil
                }
                if p.screenShareTrackSid == trackSid {
                    p.hasScreenShare = false
                    p.screenShareTrackSid = nil
                }
                self.participants[idx] = p
            }

        case .handRaisedChanged(let participantSid, let raised, let position):
            if raised {
                self.handRaisedMap[participantSid] = Int(position)
            } else {
                self.handRaisedMap.removeValue(forKey: participantSid)
            }
            // Update local hand raise state — always sync from client truth
            if self.client.isHandRaised() != self.isHandRaised {
                self.isHandRaised = self.client.isHandRaised()
            }

        case .reactionReceived(let participantSid, let participantName, let emoji):
            let reaction = ReactionData(
                id: self.reactionIdCounter,
                participantSid: participantSid,
                participantName: participantName,
                emoji: emoji,
                timestamp: Date()
            )
            self.reactionIdCounter += 1
            self.reactions.append(reaction)

        case .unreadCountChanged(let count):
            self.unreadCount = Int(count)

        case .lobbyParticipantJoined(let id, let username):
            if !self.waitingParticipants.contains(where: { $0.id == id }) {
                let participant = WaitingParticipant(id: id, username: username)
                self.waitingParticipants.append(participant)
                self.lobbyNotification = participant
            }

        case .lobbyParticipantLeft(let id):
            self.waitingParticipants.removeAll { $0.id == id }

        case .lobbyDenied:
            self.lobbyDenied = true

        case .adaptiveModeChanged(let mode):
            let previousMode = self.adaptiveMode
            self.adaptiveMode = mode
            if mode == .car {
                self.cameraWasEnabledBeforeCar = self.isCameraEnabled
                if self.isCameraEnabled {
                    // Grace period: don't disable camera if we just connected
                    let grace = self.connectionTimestamp.map { Date().timeIntervalSince($0) } ?? 999
                    if grace < self.connectionGraceSeconds {
                        // Delay camera disable to let camera-on-join complete
                        Task { @MainActor [weak self] in
                            try? await Task.sleep(for: .seconds(self?.connectionGraceSeconds ?? 5))
                            guard let self, self.adaptiveMode == .car else { return }
                            self.cameraWasEnabledBeforeCar = self.isCameraEnabled
                            if self.isCameraEnabled { self.toggleCamera() }
                        }
                    } else {
                        self.toggleCamera()
                    }
                }
                self.routeAudioToBluetooth()
            } else if previousMode == .car {
                self.restoreDefaultAudioRoute()
                if self.cameraWasEnabledBeforeCar {
                    self.cameraWasEnabledBeforeCar = false
                    if !self.isCameraEnabled {
                        self.toggleCamera()
                    }
                }
            }

        case .bandwidthModeChanged:
            break

        case .connectionLost:
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

        case .lobbyTimeout:
            self.lobbyDenied = true

        case .disconnectedDuplicateIdentity:
            self.errorMessage = "Déconnecté : un autre appareil a rejoint avec le même identifiant"

        case .disconnectedByAdmin:
            self.errorMessage = "Vous avez été déconnecté par un administrateur"

        case .aloneInRoom(let remainingSecs):
            self.errorMessage = "Vous êtes seul — déconnexion dans \(remainingSecs)s"

        case .aloneInRoomCancelled:
            self.errorMessage = nil

        case .muteRequested:
            if self.isMicEnabled {
                self.toggleMic()
            }

        case .meetingsUpdated(let meetings):
            self.upcomingMeetings = meetings
            self.calendarLoading = false

        case .meetingImminent(let meeting):
            scheduleMeetingNotification(
                title: "Réunion bientôt",
                body: "\(meeting.summary) commence dans 15 minutes",
                identifier: "meeting-imminent-\(meeting.id)"
            )

        case .meetingStartingSoon(let meeting):
            scheduleMeetingNotification(
                title: "Réunion imminente",
                body: "\(meeting.summary) commence dans moins de 5 minutes",
                identifier: "meeting-soon-\(meeting.id)"
            )

        case .meetingStarted(let meeting):
            scheduleMeetingNotification(
                title: "Réunion en cours",
                body: "\(meeting.summary) a commencé",
                identifier: "meeting-started-\(meeting.id)"
            )

        case .calendarError(let message):
            NSLog("VisioManager: calendar error: %@", message)
            self.calendarLoading = false
        }
    }
}

// MARK: - VisioEventListener

extension VisioManager: VisioEventListener {

    nonisolated func onEvent(event: VisioEvent) {
        Task { @MainActor [weak self] in
            self?.handleEvent(event)
        }
    }
}

// MARK: - Reaction Data

struct ReactionData: Identifiable, Sendable {
    let id: Int64
    let participantSid: String
    let participantName: String
    let emoji: String
    let timestamp: Date
}
