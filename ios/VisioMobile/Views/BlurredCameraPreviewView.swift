import SwiftUI

/// Camera preview that routes frames through the Rust blur pipeline.
/// Reuses VideoDisplayView + VideoFrameRouter to display processed I420 frames
/// delivered by visio_push_ios_camera_frame with track SID "local-camera".
struct BlurredCameraPreviewView: UIViewRepresentable {
    let isFront: Bool
    @EnvironmentObject private var manager: VisioManager

    func makeUIView(context: Context) -> VideoDisplayView {
        let view = VideoDisplayView()
        view.setupDisplayLayer(fill: true)
        VideoFrameRouter.shared.register(trackSid: "local-camera", view: view)
        manager.startPreviewCapture(isFront: isFront)
        return view
    }

    func updateUIView(_ uiView: VideoDisplayView, context: Context) {
        manager.switchCamera(toFront: isFront)
    }

    static func dismantleUIView(_ uiView: VideoDisplayView, coordinator: ()) {
        VideoFrameRouter.shared.unregister(trackSid: "local-camera", view: uiView)
    }
}
