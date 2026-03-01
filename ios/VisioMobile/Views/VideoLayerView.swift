import SwiftUI
import AVFoundation

struct VideoLayerView: UIViewRepresentable {
    let trackSid: String

    func makeUIView(context: Context) -> VideoDisplayView {
        let view = VideoDisplayView()
        view.trackSid = trackSid
        view.setupDisplayLayer()
        VideoFrameRouter.shared.register(trackSid: trackSid, view: view)
        return view
    }

    func updateUIView(_ uiView: VideoDisplayView, context: Context) {}

    static func dismantleUIView(_ uiView: VideoDisplayView, coordinator: ()) {
        VideoFrameRouter.shared.unregister(trackSid: uiView.trackSid)
    }
}

class VideoDisplayView: UIView {
    var trackSid: String = ""
    private var displayLayer: AVSampleBufferDisplayLayer?

    override func layoutSubviews() {
        super.layoutSubviews()
        displayLayer?.frame = bounds
    }

    func setupDisplayLayer() {
        let layer = AVSampleBufferDisplayLayer()
        layer.videoGravity = .resizeAspect
        layer.frame = bounds
        self.layer.addSublayer(layer)
        displayLayer = layer
    }

    /// Called from VideoFrameRouter on the main thread to enqueue a frame.
    func enqueueSampleBuffer(_ sampleBuffer: CMSampleBuffer) {
        displayLayer?.enqueue(sampleBuffer)
    }
}
