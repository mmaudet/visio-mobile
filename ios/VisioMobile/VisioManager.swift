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

    // MARK: - Private

    let client: VisioClient
    private var audioPlayout: AudioPlayout?
    private var cameraCapture: CameraCapture?

    // MARK: - Init

    init() {
        // VisioClient() creates a tokio runtime -- acceptable to block on main thread at launch.
        let documentsDir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        client = VisioClient(dataDir: documentsDir.path)
        client.addListener(listener: self)

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
    }

    // MARK: - Public API

    func connect(url: String, username: String?) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                try self.client.connect(meetUrl: url, username: username)
                // Sync initial state after successful connection.
                let parts = self.client.participants()
                let mic = self.client.isMicrophoneEnabled()
                let cam = self.client.isCameraEnabled()
                let msgs = self.client.chatMessages()
                let state = self.client.connectionState()
                let hand = self.client.isHandRaised()
                DispatchQueue.main.async {
                    self.participants = parts
                    self.isMicEnabled = mic
                    self.isCameraEnabled = cam
                    self.chatMessages = msgs
                    self.connectionState = state
                    self.isHandRaised = hand
                    self.errorMessage = nil
                }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = "Connection failed: \(error.localizedDescription)"
                }
            }
        }
    }

    func disconnect() {
        stopAudioPlayout()
        cameraCapture?.stop()
        cameraCapture = nil
        // Stop all video renderers
        let sids = videoTrackSids
        for sid in sids {
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                self?.client.stopVideoRenderer(trackSid: sid)
            }
        }
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
                try self.client.setMicrophoneEnabled(enabled: enabled)
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

    func toggleCamera() {
        let newValue = !isCameraEnabled
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                try self.client.setCameraEnabled(enabled: newValue)
                DispatchQueue.main.async {
                    self.isCameraEnabled = newValue
                    if newValue {
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

    // MARK: - Settings

    func getSettings() -> Settings {
        return client.getSettings()
    }

    func setDisplayName(_ name: String?) {
        client.setDisplayName(name: name)
    }

    func setLanguage(_ lang: String?) {
        client.setLanguage(lang: lang)
    }

    func setMicEnabledOnJoin(_ enabled: Bool) {
        client.setMicEnabledOnJoin(enabled: enabled)
    }

    func setCameraEnabledOnJoin(_ enabled: Bool) {
        client.setCameraEnabledOnJoin(enabled: enabled)
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
}

// MARK: - VisioEventListener

extension VisioManager: VisioEventListener {

    func onEvent(event: VisioEvent) {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            switch event {
            case .connectionStateChanged(let state):
                self.connectionState = state

            case .participantJoined(let info):
                if let idx = self.participants.firstIndex(where: { $0.sid == info.sid }) {
                    self.participants[idx] = info
                } else {
                    self.participants.append(info)
                }

            case .participantLeft(let sid):
                self.participants.removeAll { $0.sid == sid }
                self.handRaisedMap.removeValue(forKey: sid)

            case .trackMuted(let sid, _):
                if let idx = self.participants.firstIndex(where: { $0.sid == sid }) {
                    var p = self.participants[idx]
                    p.isMuted = true
                    self.participants[idx] = p
                }

            case .trackUnmuted(let sid, _):
                if let idx = self.participants.firstIndex(where: { $0.sid == sid }) {
                    var p = self.participants[idx]
                    p.isMuted = false
                    self.participants[idx] = p
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
                    DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                        self?.client.startVideoRenderer(trackSid: sid)
                    }
                }

            case .trackUnsubscribed(let trackSid):
                self.videoTrackSids.removeAll { $0 == trackSid }
                VideoFrameRouter.shared.unregister(trackSid: trackSid)
                DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                    self?.client.stopVideoRenderer(trackSid: trackSid)
                }

            case .handRaisedChanged(let participantSid, let raised, let position):
                if raised {
                    self.handRaisedMap[participantSid] = Int(position)
                } else {
                    self.handRaisedMap.removeValue(forKey: participantSid)
                }
                // Update local hand raise state
                let localSid = self.participants.first?.sid  // first is usually local
                if participantSid == localSid || self.client.isHandRaised() != self.isHandRaised {
                    self.isHandRaised = self.client.isHandRaised()
                }

            case .unreadCountChanged(let count):
                self.unreadCount = Int(count)
            }
        }
    }
}
