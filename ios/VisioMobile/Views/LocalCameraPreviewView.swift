import SwiftUI
import AVFoundation

struct LocalCameraPreviewView: UIViewRepresentable {
    let isFront: Bool

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeUIView(context: Context) -> PreviewUIView {
        let view = PreviewUIView()
        context.coordinator.view = view
        context.coordinator.startSession(front: isFront)
        return view
    }

    func updateUIView(_ uiView: PreviewUIView, context: Context) {
        context.coordinator.switchCamera(front: isFront)
    }

    static func dismantleUIView(_ uiView: PreviewUIView, coordinator: Coordinator) {
        coordinator.stopSession()
    }

    class Coordinator {
        weak var view: PreviewUIView?
        private let session = AVCaptureSession()
        private var currentInput: AVCaptureDeviceInput?
        private var currentPosition: AVCaptureDevice.Position = .unspecified

        func startSession(front: Bool) {
            session.sessionPreset = .medium
            let position: AVCaptureDevice.Position = front ? .front : .back
            guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: position),
                  let input = try? AVCaptureDeviceInput(device: device) else { return }

            session.beginConfiguration()
            session.addInput(input)
            session.commitConfiguration()

            currentInput = input
            currentPosition = position

            DispatchQueue.main.async { [weak self] in
                self?.view?.previewLayer.session = self?.session
            }

            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                self?.session.startRunning()
            }
        }

        func switchCamera(front: Bool) {
            let newPosition: AVCaptureDevice.Position = front ? .front : .back
            guard newPosition != currentPosition else { return }
            guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: newPosition),
                  let input = try? AVCaptureDeviceInput(device: device) else { return }

            session.beginConfiguration()
            if let old = currentInput { session.removeInput(old) }
            session.addInput(input)
            session.commitConfiguration()

            currentInput = input
            currentPosition = newPosition
        }

        func stopSession() {
            session.stopRunning()
        }
    }
}

class PreviewUIView: UIView {
    let previewLayer = AVCaptureVideoPreviewLayer()

    override init(frame: CGRect) {
        super.init(frame: frame)
        previewLayer.videoGravity = .resizeAspectFill
        layer.addSublayer(previewLayer)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        previewLayer.frame = bounds
    }
}
