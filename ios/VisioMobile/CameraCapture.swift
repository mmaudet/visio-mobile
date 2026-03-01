import AVFoundation
import visioFFI

/// Captures camera frames via AVCaptureSession and pushes I420 data to Rust.
///
/// Uses kCVPixelFormatType_420YpCbCr8BiPlanarFullRange (NV12) from the camera,
/// converts to I420 (Y + U + V planar), and calls visio_push_ios_camera_frame().
final class CameraCapture: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    private let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "io.visio.camera", qos: .userInitiated)
    private var frameCount: UInt64 = 0

    func start() {
        // Configure and start on the camera queue (Apple warns against
        // calling startRunning() on the main queue).
        queue.async { [self] in
            let authStatus = AVCaptureDevice.authorizationStatus(for: .video)
            NSLog("CameraCapture: auth status = %d (0=notDetermined,1=restricted,2=denied,3=authorized)", authStatus.rawValue)

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
            session.sessionPreset = .vga640x480

            // Try front camera first, then any camera.
            var device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .front)
            if device == nil {
                NSLog("CameraCapture: no front camera, trying any position")
                device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .unspecified)
            }
            guard let device, let input = try? AVCaptureDeviceInput(device: device) else {
                NSLog("CameraCapture: no camera available (device=%@)", device == nil ? "nil" : "found but input failed")
                session.commitConfiguration()
                return
            }
            NSLog("CameraCapture: using device '%@'", device.localizedName)

            if session.canAddInput(input) {
                session.addInput(input)
            }

            let output = AVCaptureVideoDataOutput()
            output.videoSettings = [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
            ]
            output.alwaysDiscardsLateVideoFrames = true
            output.setSampleBufferDelegate(self, queue: queue)

            if session.canAddOutput(output) {
                session.addOutput(output)
            }

            session.commitConfiguration()
            session.startRunning()
            NSLog("CameraCapture: session started, isRunning=%d", session.isRunning ? 1 : 0)
        }
    }

    func stop() {
        queue.async { [self] in
            session.stopRunning()
            NSLog("CameraCapture: stopped (pushed %llu frames)", frameCount)
        }
    }

    // MARK: - AVCaptureVideoDataOutputSampleBufferDelegate

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }

        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        let chromaW = width / 2
        let chromaH = height / 2

        guard let yBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 0),
              let uvBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 1) else { return }

        let yStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 0)
        let uvStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 1)

        let yPtr = yBase.assumingMemoryBound(to: UInt8.self)
        let uvPtr = uvBase.assumingMemoryBound(to: UInt8.self)

        frameCount += 1
        if frameCount % 30 == 1 {
            NSLog("CameraCapture: frame #%llu, %dx%d, yStride=%d, uvStride=%d",
                  frameCount, width, height, yStride, uvStride)
        }

        var uPlane = [UInt8](repeating: 0, count: chromaW * chromaH)
        var vPlane = [UInt8](repeating: 0, count: chromaW * chromaH)

        for row in 0..<chromaH {
            let uvRow = uvPtr.advanced(by: row * uvStride)
            let dstOffset = row * chromaW
            for col in 0..<chromaW {
                uPlane[dstOffset + col] = uvRow[col * 2]
                vPlane[dstOffset + col] = uvRow[col * 2 + 1]
            }
        }

        uPlane.withUnsafeBufferPointer { uBuf in
            vPlane.withUnsafeBufferPointer { vBuf in
                visio_push_ios_camera_frame(
                    yPtr, UInt32(yStride),
                    uBuf.baseAddress!, UInt32(chromaW),
                    vBuf.baseAddress!, UInt32(chromaW),
                    UInt32(width), UInt32(height)
                )
            }
        }
    }
}
