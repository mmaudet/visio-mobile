import AVFoundation
import UIKit

/// Captures camera frames via AVCaptureSession and pushes I420 data to Rust.
///
/// Uses kCVPixelFormatType_420YpCbCr8BiPlanarFullRange (NV12) from the camera,
/// converts to I420 (Y + U + V planar), and calls visio_push_ios_camera_frame().
final class CameraCapture: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    private let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "io.visio.camera", qos: .userInitiated)
    private var frameCount: UInt64 = 0
    private var currentPosition: AVCaptureDevice.Position = .front
    private var currentInput: AVCaptureDeviceInput?
    private var videoOutput: AVCaptureVideoDataOutput?
    private var uPlane = [UInt8]()
    private var vPlane = [UInt8]()

    func start() {
        // Configure and start on the camera queue (Apple warns against
        // calling startRunning() on the main queue).
        queue.async { [self] in
            let authStatus = AVCaptureDevice.authorizationStatus(for: .video)
            NSLog("CameraCapture: auth status = %d (0=notDetermined,1=restricted,2=denied,3=authorized)", authStatus.rawValue)

            if authStatus == .notDetermined {
                // Request permission synchronously on this queue (blocks until user responds).
                let semaphore = DispatchSemaphore(value: 0)
                var granted = false
                AVCaptureDevice.requestAccess(for: .video) { result in
                    granted = result
                    semaphore.signal()
                }
                semaphore.wait()
                if !granted {
                    NSLog("CameraCapture: user denied camera permission")
                    return
                }
                NSLog("CameraCapture: user granted camera permission")
            } else if authStatus == .denied || authStatus == .restricted {
                NSLog("CameraCapture: camera permission denied/restricted, cannot start")
                return
            }

            let discoverySession = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.builtInWideAngleCamera, .builtInDualCamera, .builtInTrueDepthCamera],
                mediaType: .video,
                position: .unspecified
            )
            for dev in discoverySession.devices {
                NSLog("CameraCapture: found device '%@' position=%d uniqueID=%@",
                      dev.localizedName, dev.position.rawValue, dev.uniqueID)
            }

            session.beginConfiguration()
            session.sessionPreset = .medium

            // Try front camera first, then any camera.
            var device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .front)
            if device == nil {
                NSLog("CameraCapture: no front camera, trying any position")
                device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .unspecified)
            }
            guard let device else {
                NSLog("CameraCapture: no camera device available")
                session.commitConfiguration()
                return
            }
            let input: AVCaptureDeviceInput
            do {
                input = try AVCaptureDeviceInput(device: device)
            } catch {
                NSLog("CameraCapture: failed to create input: %@", error.localizedDescription)
                session.commitConfiguration()
                return
            }
            NSLog("CameraCapture: using device '%@'", device.localizedName)

            if session.canAddInput(input) {
                session.addInput(input)
                currentInput = input
                currentPosition = device.position
            }

            let output = AVCaptureVideoDataOutput()
            output.videoSettings = [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
            ]
            output.alwaysDiscardsLateVideoFrames = true
            output.setSampleBufferDelegate(self, queue: queue)

            if session.canAddOutput(output) {
                session.addOutput(output)
                self.videoOutput = output
                if let connection = output.connection(with: .video) {
                    if connection.isVideoOrientationSupported {
                        connection.videoOrientation = Self.currentVideoOrientation()
                    }
                    if connection.isVideoMirroringSupported && device.position == .front {
                        connection.isVideoMirrored = true
                    }
                }
            }

            session.commitConfiguration()
            session.startRunning()
            NSLog("CameraCapture: session started, isRunning=%d", session.isRunning ? 1 : 0)

            // Observe device orientation changes to update capture orientation.
            DispatchQueue.main.async {
                UIDevice.current.beginGeneratingDeviceOrientationNotifications()
                NotificationCenter.default.addObserver(
                    self,
                    selector: #selector(self.orientationDidChange),
                    name: UIDevice.orientationDidChangeNotification,
                    object: nil
                )
            }
        }
    }

    func switchCamera(toFront: Bool) {
        queue.async { [self] in
            let newPosition: AVCaptureDevice.Position = toFront ? .front : .back
            guard newPosition != currentPosition else { return }

            guard let newDevice = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: newPosition) else {
                NSLog("CameraCapture: no camera for position %d", newPosition.rawValue)
                return
            }
            let newInput: AVCaptureDeviceInput
            do {
                newInput = try AVCaptureDeviceInput(device: newDevice)
            } catch {
                NSLog("CameraCapture: failed to create input for position %d: %@", newPosition.rawValue, error.localizedDescription)
                return
            }

            session.beginConfiguration()
            if let currentInput {
                session.removeInput(currentInput)
            }
            if session.canAddInput(newInput) {
                session.addInput(newInput)
                currentInput = newInput
                currentPosition = newPosition
            }
            session.commitConfiguration()
            NSLog("CameraCapture: switched to %@ camera", toFront ? "front" : "back")
        }
    }

    func isFront() -> Bool {
        return currentPosition == .front
    }

    func stop() {
        DispatchQueue.main.async {
            NotificationCenter.default.removeObserver(self, name: UIDevice.orientationDidChangeNotification, object: nil)
            UIDevice.current.endGeneratingDeviceOrientationNotifications()
        }
        queue.async { [self] in
            session.stopRunning()
            NSLog("CameraCapture: stopped (pushed %llu frames)", frameCount)
        }
    }

    // MARK: - Orientation

    /// Map current device orientation to AVCaptureVideoOrientation.
    static func currentVideoOrientation() -> AVCaptureVideoOrientation {
        switch UIDevice.current.orientation {
        case .portrait:            return .portrait
        case .portraitUpsideDown:  return .portraitUpsideDown
        case .landscapeLeft:      return .landscapeRight  // UIKit and AVFoundation use opposite conventions
        case .landscapeRight:     return .landscapeLeft
        default:                  return .portrait
        }
    }

    @objc private func orientationDidChange() {
        queue.async { [self] in
            guard let connection = videoOutput?.connection(with: .video),
                  connection.isVideoOrientationSupported else { return }
            let newOrientation = Self.currentVideoOrientation()
            if connection.videoOrientation != newOrientation {
                connection.videoOrientation = newOrientation
                NSLog("CameraCapture: orientation updated to %d", newOrientation.rawValue)
            }
        }
    }

    // MARK: - AVCaptureVideoDataOutputSampleBufferDelegate

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }

        frameCount += 1
        if frameCount % 30 == 1 {
            let width = CVPixelBufferGetWidth(pixelBuffer)
            let height = CVPixelBufferGetHeight(pixelBuffer)
            let yStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 0)
            let uvStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 1)
            NSLog("CameraCapture: frame #%llu, %dx%d, yStride=%d, uvStride=%d",
                  frameCount, width, height, yStride, uvStride)
        }

        pushNV12FrameToRust(pixelBuffer, uPlane: &uPlane, vPlane: &vPlane)
    }
}
