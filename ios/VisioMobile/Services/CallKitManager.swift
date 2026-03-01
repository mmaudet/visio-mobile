import CallKit
import AVFoundation

/// Manages CallKit integration for system call UI (green bar, Dynamic Island, lock screen controls).
///
/// Flow:
/// 1. `connect()` -> `reportCallStarted()` -> iOS shows call indicator
/// 2. Incoming phone call -> `performSetHeldCallAction` -> auto-mute mic
/// 3. `disconnect()` -> `reportCallEnded()` -> indicator removed
/// 4. Lock screen: native mute/hangup buttons -> actions relayed to VisioManager
class CallKitManager: NSObject, CXProviderDelegate {

    static let shared = CallKitManager()

    private let provider: CXProvider
    private let callController = CXCallController()
    private(set) var currentCallUUID: UUID?

    override init() {
        let config = CXProviderConfiguration()
        config.supportsVideo = true
        config.maximumCallsPerCallGroup = 1
        config.supportedHandleTypes = [.generic]
        config.iconTemplateImageData = nil  // Use default app icon
        provider = CXProvider(configuration: config)
        super.init()
        provider.setDelegate(self, queue: nil)
    }

    // MARK: - Public API

    /// Report that an outgoing call has started (user joined a room).
    func reportCallStarted(roomName: String) {
        let uuid = UUID()
        currentCallUUID = uuid

        let handle = CXHandle(type: .generic, value: roomName)
        let action = CXStartCallAction(call: uuid, handle: handle)
        action.isVideo = true

        let transaction = CXTransaction(action: action)
        callController.request(transaction) { [weak self] error in
            if let error {
                NSLog("CallKitManager: start call failed: \(error.localizedDescription)")
            } else {
                self?.provider.reportOutgoingCall(with: uuid, connectedAt: Date())
                NSLog("CallKitManager: call started for room '\(roomName)'")
            }
        }

        // Configure audio session for voice chat
        configureAudioSession()
    }

    /// Report that the call has ended.
    func reportCallEnded() {
        guard let uuid = currentCallUUID else { return }
        let action = CXEndCallAction(call: uuid)
        let transaction = CXTransaction(action: action)
        callController.request(transaction) { error in
            if let error {
                NSLog("CallKitManager: end call failed: \(error.localizedDescription)")
            } else {
                NSLog("CallKitManager: call ended")
            }
        }
        currentCallUUID = nil
    }

    // MARK: - CXProviderDelegate

    func providerDidReset(_ provider: CXProvider) {
        NSLog("CallKitManager: provider did reset")
        currentCallUUID = nil
    }

    func provider(_ provider: CXProvider, perform action: CXStartCallAction) {
        configureAudioSession()
        action.fulfill()
    }

    func provider(_ provider: CXProvider, perform action: CXEndCallAction) {
        // System ended the call (user tapped end on lock screen / Dynamic Island)
        VisioManager.shared.disconnect()
        currentCallUUID = nil
        action.fulfill()
    }

    func provider(_ provider: CXProvider, perform action: CXSetMutedCallAction) {
        // System toggled mute (lock screen mute button)
        VisioManager.shared.setMicEnabled(!action.isMuted)
        action.fulfill()
    }

    func provider(_ provider: CXProvider, perform action: CXSetHeldCallAction) {
        // Phone call interrupted -- mute mic when held
        if action.isOnHold {
            VisioManager.shared.setMicEnabled(false)
        }
        action.fulfill()
    }

    func provider(_ provider: CXProvider, didActivate audioSession: AVAudioSession) {
        NSLog("CallKitManager: audio session activated")
    }

    func provider(_ provider: CXProvider, didDeactivate audioSession: AVAudioSession) {
        NSLog("CallKitManager: audio session deactivated")
    }

    // MARK: - Private

    private func configureAudioSession() {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playAndRecord, mode: .voiceChat, options: [.allowBluetooth, .allowBluetoothA2DP, .defaultToSpeaker])
            try session.setActive(true)
        } catch {
            NSLog("CallKitManager: audio session config failed: \(error)")
        }
    }
}
