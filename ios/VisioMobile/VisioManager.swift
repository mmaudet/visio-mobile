import Foundation
import SwiftUI

/// Central state manager for the Visio app, backed by UniFFI-generated VisioClient.
/// Conforms to VisioEventListener to receive room events from Rust.
class VisioManager: ObservableObject {

    // MARK: - Published state

    @Published var connectionState: ConnectionState = .disconnected
    @Published var participants: [ParticipantInfo] = []
    @Published var activeSpeakers: [String] = []
    @Published var chatMessages: [ChatMessage] = []
    @Published var isMicEnabled: Bool = false
    @Published var isCameraEnabled: Bool = false
    @Published var errorMessage: String?

    // MARK: - Private

    private let client: VisioClient

    // MARK: - Init

    init() {
        // VisioClient() creates a tokio runtime — acceptable to block on main thread at launch.
        client = VisioClient()
        client.addListener(listener: self)
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
                DispatchQueue.main.async {
                    self.participants = parts
                    self.isMicEnabled = mic
                    self.isCameraEnabled = cam
                    self.chatMessages = msgs
                    self.connectionState = state
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
                self.errorMessage = nil
            }
        }
    }

    func toggleMic() {
        let newValue = !isMicEnabled
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                try self.client.setMicrophoneEnabled(enabled: newValue)
                DispatchQueue.main.async {
                    self.isMicEnabled = newValue
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
                }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = "Camera toggle failed: \(error.localizedDescription)"
                }
            }
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
                // Replace if already present, otherwise append.
                if let idx = self.participants.firstIndex(where: { $0.sid == info.sid }) {
                    self.participants[idx] = info
                } else {
                    self.participants.append(info)
                }

            case .participantLeft(let sid):
                self.participants.removeAll { $0.sid == sid }

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

            case .trackSubscribed, .trackUnsubscribed:
                // Video track handling will be added in a later phase.
                break
            }
        }
    }
}
