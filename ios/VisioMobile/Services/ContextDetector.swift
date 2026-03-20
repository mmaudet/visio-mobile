import Foundation
import Network
import CoreMotion
import AVFoundation
import visioFFI

@MainActor
class ContextDetector: NSObject {
    private let pathMonitor = NWPathMonitor()
    private let motionManager = CMMotionActivityManager()
    private let monitorQueue = DispatchQueue(label: "io.visio.context")

    private let manager: VisioManager

    private var isMoving = false
    private var routeChangeObserver: NSObjectProtocol?
    private var interruptionObserver: NSObjectProtocol?

    init(manager: VisioManager = VisioManager.shared) {
        self.manager = manager
        super.init()
    }

    func start() {
        startNetworkMonitoring()
        startMotionDetection()
        startBluetoothMonitoring()
    }

    func stop() {
        pathMonitor.cancel()
        motionManager.stopActivityUpdates()
        if let token = routeChangeObserver {
            NotificationCenter.default.removeObserver(token)
            routeChangeObserver = nil
        }
        if let token = interruptionObserver {
            NotificationCenter.default.removeObserver(token)
            interruptionObserver = nil
        }
    }

    private func startNetworkMonitoring() {
        pathMonitor.pathUpdateHandler = { [weak self] path in
            guard let self else { return }
            let networkType: NetworkType
            if path.usesInterfaceType(.wifi) {
                networkType = .wifi
            } else if path.usesInterfaceType(.cellular) {
                networkType = .cellular
            } else {
                networkType = .unknown
            }
            self.manager.reportNetworkType(networkType)
        }
        pathMonitor.start(queue: monitorQueue)
    }

    private func startMotionDetection() {
        guard CMMotionActivityManager.isActivityAvailable() else { return }
        motionManager.startActivityUpdates(to: .main) { [weak self] activity in
            guard let activity = activity else { return }
            let moving = activity.walking || activity.running || activity.cycling
            guard moving != self?.isMoving else { return }
            self?.isMoving = moving
            self?.manager.reportMotionDetected(moving)
        }
    }

    private func startBluetoothMonitoring() {
        routeChangeObserver = NotificationCenter.default.addObserver(
            forName: AVAudioSession.routeChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.handleAudioRouteChange()
            }
        }

        interruptionObserver = NotificationCenter.default.addObserver(
            forName: AVAudioSession.interruptionNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.reportBluetoothCarKit()
            }
        }

        reportBluetoothCarKit()
    }

    private func handleAudioRouteChange() {
        reportBluetoothCarKit()

        guard case .connected = manager.connectionState else { return }

        let route = AVAudioSession.sharedInstance().currentRoute
        let hasBluetooth = route.outputs.contains { port in
            port.portType == .bluetoothA2DP ||
            port.portType == .bluetoothHFP ||
            port.portType == .bluetoothLE ||
            port.portType == .carAudio
        }

        if hasBluetooth {
            // BT device connected — route audio to it
            manager.routeAudioToBluetooth()
        } else {
            // BT disconnected — check if another BT device is still available
            let session = AVAudioSession.sharedInstance()
            let hasRemainingBt = session.availableInputs?.contains { port in
                port.portType == .bluetoothHFP ||
                port.portType == .bluetoothA2DP ||
                port.portType == .bluetoothLE
            } ?? false

            if hasRemainingBt {
                // Another BT device available — route to it
                manager.routeAudioToBluetooth()
            } else {
                // No BT left — restore phone speaker/mic
                manager.restoreDefaultAudioRoute()
            }
        }
    }

    private func reportBluetoothCarKit() {
        let route = AVAudioSession.sharedInstance().currentRoute
        let hasCarKit = route.outputs.contains { port in
            // HFP and carAudio are car-specific profiles
            // A2DP alone is typically headphones/earbuds, not a car
            port.portType == .bluetoothHFP ||
            port.portType == .carAudio
        }
        manager.reportBluetoothCarKit(connected: hasCarKit)
    }
}
