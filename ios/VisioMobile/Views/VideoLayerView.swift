import SwiftUI
import AVFoundation

struct VideoLayerView: UIViewRepresentable {
    let trackSid: String

    func makeUIView(context: Context) -> VideoDisplayView {
        let view = VideoDisplayView()
        view.trackSid = trackSid
        view.setupDisplayLayer()
        return view
    }

    func updateUIView(_ uiView: VideoDisplayView, context: Context) {}
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

    /// Called from the Rust video callback to enqueue a frame.
    /// In production, this receives CVPixelBuffer from the Rust callback
    /// and wraps it in a CMSampleBuffer for display.
    func enqueueSampleBuffer(_ sampleBuffer: CMSampleBuffer) {
        displayLayer?.enqueue(sampleBuffer)
    }
}
