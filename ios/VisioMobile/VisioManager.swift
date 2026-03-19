import AVFoundation
import Combine
import Foundation
import SwiftUI
import visioFFI

/// Central state manager for the Visio app, backed by UniFFI-generated VisioClient.
/// Conforms to VisioEventListener to receive room events from Rust.
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
    @Published var pendingTestConnect: (String, String, String?)? = nil
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

    let authManager = OidcAuthManager()
    private var authCancellable: AnyCancellable?

    // MARK: - Private

    let client: VisioClient
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
        currentTheme = settings.theme ?? "light"
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

        // Forward authManager changes so SwiftUI picks up pendingInstance.
        authCancellable = authManager.objectWillChange.sink { [weak self] _ in
            self?.objectWillChange.send()
        }
    }

    // MARK: - Public API

    func connect(url: String, username: String?) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                let settings = self.client.getSettings()

                let cameraNeeded = settings.cameraEnabledOnJoin || self.client.isCameraEnabled()
                Self.ensureMediaPermissions(mic: settings.micEnabledOnJoin, camera: cameraNeeded)

                if settings.micEnabledOnJoin {
                    Self.configureAudioSession()
                }

                try self.client.connect(meetUrl: url, username: username)

                if settings.micEnabledOnJoin {
                    try self.client.setMicrophoneEnabled(enabled: true)
                }
                if cameraNeeded {
                    try self.client.setCameraEnabled(enabled: true)
                }

                let parts = self.client.participants()
                let mic = self.client.isMicrophoneEnabled()
                let cam = self.client.isCameraEnabled()
                let msgs = self.client.chatMessages()
                let state = self.client.connectionState()
                let hand = self.client.isHandRaised()

                if mic {
                    let capture = AudioCapture()
                    capture.start()
                    DispatchQueue.main.async { self.audioCapture = capture }
                }

                DispatchQueue.main.async {
                    self.participants = parts
                    self.isMicEnabled = mic
                    self.isCameraEnabled = cam
                    self.chatMessages = msgs
                    self.connectionState = state
                    self.isHandRaised = hand
                    self.errorMessage = nil
                    if cam {
                        let capture = CameraCapture()
                        capture.start()
                        self.cameraCapture = capture
                    }

                    // Start audio playout now that connection is established
                    self.startAudioPlayout()

                    // Start context detection for adaptive modes
                    self.startContextDetection()
                }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = "Connection failed: \(error.localizedDescription)"
                }
            }
        }
    }

    func connectWithToken(livekitUrl: String, token: String) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                let settings = self.client.getSettings()

                let cameraNeeded = settings.cameraEnabledOnJoin || self.client.isCameraEnabled()
                Self.ensureMediaPermissions(mic: settings.micEnabledOnJoin, camera: cameraNeeded)

                try self.client.connectWithToken(livekitUrl: livekitUrl, token: token)

                if settings.micEnabledOnJoin {
                    try self.client.setMicrophoneEnabled(enabled: true)
                }
                if cameraNeeded {
                    try self.client.setCameraEnabled(enabled: true)
                }

                let parts = self.client.participants()
                let mic = self.client.isMicrophoneEnabled()
                let cam = self.client.isCameraEnabled()
                let state = self.client.connectionState()

                if mic {
                    let capture = AudioCapture()
                    capture.start()
                    DispatchQueue.main.async { self.audioCapture = capture }
                }

                DispatchQueue.main.async {
                    self.participants = parts
                    self.isMicEnabled = mic
                    self.isCameraEnabled = cam
                    self.connectionState = state
                    self.errorMessage = nil
                    if cam {
                        let capture = CameraCapture()
                        capture.start()
                        self.cameraCapture = capture
                    }
                    // Start audio playout now that connection is established
                    self.startAudioPlayout()
                }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = "Test connect failed: \(error.localizedDescription)"
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
        for sid in sids {
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                self?.client.stopVideoRenderer(trackSid: sid)
            }
        }
        VideoFrameRouter.shared.clearAll()
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            self.client.disconnect()
            DispatchQueue.main.async {
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
    }

    func toggleMic() {
        let newValue = !isMicEnabled
        setMicEnabled(newValue)
    }

    func setMicEnabled(_ enabled: Bool) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                if enabled {
                    // Configure audio session before enabling mic
                    Self.configureAudioSession()
                }
                try self.client.setMicrophoneEnabled(enabled: enabled)

                if enabled {
                    // Start mic capture on background thread (AVAudioEngine must not start on main)
                    if self.audioCapture == nil {
                        let capture = AudioCapture()
                        capture.start()
                        DispatchQueue.main.async { self.audioCapture = capture }
                    }
                } else {
                    self.audioCapture?.stop()
                    DispatchQueue.main.async { self.audioCapture = nil }
                }

                DispatchQueue.main.async {
                    self.isMicEnabled = enabled
                }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = "Mic toggle failed: \(error.localizedDescription)"
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
    /// Blocks the calling thread until the user responds (must not be called on main).
    static func ensureMediaPermissions(mic: Bool, camera: Bool) {
        let semaphore = DispatchSemaphore(value: 0)

        if mic {
            let micStatus = AVCaptureDevice.authorizationStatus(for: .audio)
            if micStatus == .notDetermined {
                AVCaptureDevice.requestAccess(for: .audio) { granted in
                    NSLog("VisioManager: mic permission %@", granted ? "granted" : "denied")
                    semaphore.signal()
                }
                semaphore.wait()
            }
        }

        if camera {
            let camStatus = AVCaptureDevice.authorizationStatus(for: .video)
            if camStatus == .notDetermined {
                AVCaptureDevice.requestAccess(for: .video) { granted in
                    NSLog("VisioManager: camera permission %@", granted ? "granted" : "denied")
                    semaphore.signal()
                }
                semaphore.wait()
            }
        }
    }

    /// Configure AVAudioSession for voice chat (play + record).
    static func configureAudioSession() {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playAndRecord, mode: .voiceChat, options: [.defaultToSpeaker, .allowBluetooth])
            try session.setPreferredSampleRate(48_000)
            try session.setActive(true)
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
                DispatchQueue.main.async {
                    self.errorMessage = "Camera permission denied. Enable in Settings → Visio Mobile → Camera."
                }
                return
            }
        }
        setCameraEnabled(!isCameraEnabled)
    }

    func setCameraEnabled(_ enabled: Bool) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                try self.client.setCameraEnabled(enabled: enabled)
                let parts = self.client.participants()
                DispatchQueue.main.async {
                    self.isCameraEnabled = enabled
                    self.participants = parts
                    if enabled {
                        let capture = CameraCapture()
                        capture.start()
                        self.cameraCapture = capture
                    } else {
                        self.cameraCapture?.stop()
                        self.cameraCapture = nil
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = "Camera toggle failed: \(error.localizedDescription)"
                }
            }
        }
    }

    func toggleHandRaise() {
        let shouldRaise = !isHandRaised
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                if shouldRaise {
                    try self.client.raiseHand()
                } else {
                    try self.client.lowerHand()
                }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = "Hand raise failed: \(error.localizedDescription)"
                }
            }
        }
    }

    func sendReaction(_ emoji: String) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                try self.client.sendReaction(emoji: emoji)
                // Show reaction locally (server echo is filtered out in Rust)
                DispatchQueue.main.async {
                    let reaction = ReactionData(
                        id: self.reactionIdCounter,
                        participantSid: "local",
                        participantName: self.client.getSettings().displayName ?? "",
                        emoji: emoji,
                        timestamp: Date()
                    )
                    self.reactionIdCounter += 1
                    self.reactions.append(reaction)
                }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = "Reaction failed: \(error.localizedDescription)"
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
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                let msg = try self.client.sendChatMessage(text: trimmed)
                DispatchQueue.main.async {
                    self.chatMessages.append(msg)
                }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = "Send failed: \(error.localizedDescription)"
                }
            }
        }
    }

    // MARK: - Lobby

    func admitParticipant(_ id: String) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                try self.client.admitParticipant(participantId: id)
                DispatchQueue.main.async {
                    self.waitingParticipants.removeAll { $0.id == id }
                }
            } catch {
                NSLog("VisioManager: admit failed: \(error)")
            }
        }
    }

    func denyParticipant(_ id: String) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                try self.client.denyParticipant(participantId: id)
                DispatchQueue.main.async {
                    self.waitingParticipants.removeAll { $0.id == id }
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
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            self?.client.cancelLobby()
        }
    }

    // MARK: - Authentication

    func initAuth() {
        guard let cookie = authManager.getSavedCookie(),
              let meetInstance = client.getMeetInstances().first else { return }

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                try self.client.authenticate(meetUrl: "https://\(meetInstance)", cookie: cookie)
                let state = self.client.getSessionState()
                DispatchQueue.main.async {
                    self.updateSessionFromState(state)
                }
            } catch {
                self.authManager.clearCookie()
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

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                try self.client.authenticate(meetUrl: "https://\(meetInstance)", cookie: cookie)
                let state = self.client.getSessionState()
                DispatchQueue.main.async {
                    self.updateSessionFromState(state)
                }
            } catch {
                self.authManager.clearCookie()
            }
        }
    }

    func logoutSession() {
        let instance = authenticatedMeetInstance.isEmpty
            ? client.getMeetInstances().first ?? ""
            : authenticatedMeetInstance
        guard !instance.isEmpty else { return }
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            try? self.client.logout(meetUrl: "https://\(instance)")
            self.authManager.clearCookie()
            DispatchQueue.main.async {
                self.isAuthenticated = false
                self.authenticatedDisplayName = ""
                self.authenticatedEmail = ""
                self.authenticatedMeetInstance = ""
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
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            do {
                let accesses = try self?.client.listAccesses(roomId: roomId) ?? []
                DispatchQueue.main.async {
                    self?.roomAccesses = accesses
                }
            } catch { }
        }
    }

    func addAccessMember(userId: String) {
        guard let roomId = currentRoomId else { return }
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            do {
                _ = try self?.client.addAccess(userId: userId, roomId: roomId)
                self?.refreshAccesses()
            } catch { }
        }
    }

    func removeAccessMember(accessId: String) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            do {
                try self?.client.removeAccess(accessId: accessId)
                self?.refreshAccesses()
            } catch { }
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

    func setNotificationParticipantJoin(_ enabled: Bool) {
        client.setNotificationParticipantJoin(enabled: enabled)
    }

    func setNotificationHandRaised(_ enabled: Bool) {
        client.setNotificationHandRaised(enabled: enabled)
    }

    func setNotificationMessageReceived(_ enabled: Bool) {
        client.setNotificationMessageReceived(enabled: enabled)
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
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                guard let self else { return }
                do {
                    try self.client.reconnect()
                } catch {
                    DispatchQueue.main.async {
                        self.errorMessage = "Reconnection failed: \(error.localizedDescription)"
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
}

// MARK: - VisioEventListener

extension VisioManager: VisioEventListener {

    func onEvent(event: VisioEvent) {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
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
                    DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                        guard let self else { return }
                        let updated = self.client.participants()
                        DispatchQueue.main.async { self.participants = updated }
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
                    DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                        guard let self else { return }
                        self.client.startVideoRenderer(trackSid: sid)
                        let updated = self.client.participants()
                        DispatchQueue.main.async {
                            self.participants = updated
                            if isScreenShare {
                                self.lastScreenShareParticipantSid = nil
                                self.lastScreenShareParticipantSid = participantSid
                            }
                        }
                    }
                }

            case .trackUnsubscribed(let trackSid):
                self.videoTrackSids.removeAll { $0 == trackSid }
                VideoFrameRouter.shared.invalidateTrack(trackSid: trackSid)
                DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                    self?.client.stopVideoRenderer(trackSid: trackSid)
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
                            DispatchQueue.main.asyncAfter(deadline: .now() + self.connectionGraceSeconds) {
                                if self.adaptiveMode == .car {
                                    self.cameraWasEnabledBeforeCar = self.isCameraEnabled
                                    if self.isCameraEnabled { self.toggleCamera() }
                                }
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
                DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                    guard let self else { return }
                    do {
                        try self.client.reconnect()
                    } catch {
                        DispatchQueue.main.async {
                            self.errorMessage = "Reconnection failed: \(error.localizedDescription)"
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
            }
        }
    }
}

// MARK: - Reaction Data

struct ReactionData: Identifiable {
    let id: Int64
    let participantSid: String
    let participantName: String
    let emoji: String
    let timestamp: Date
}
