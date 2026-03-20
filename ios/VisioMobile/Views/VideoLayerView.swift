import SwiftUI
import AVFoundation

struct VideoLayerView: UIViewRepresentable {
    let trackSid: String
    var isScreenShare: Bool = false

    func makeUIView(context: Context) -> VideoDisplayView {
        let view = VideoDisplayView()
        view.trackSid = trackSid
        view.setupDisplayLayer(fill: !isScreenShare)
        VideoFrameRouter.shared.register(trackSid: trackSid, view: view)
        return view
    }

    func updateUIView(_ uiView: VideoDisplayView, context: Context) {
        guard uiView.trackSid != trackSid else { return }
        let oldSid = uiView.trackSid
        uiView.trackSid = trackSid
        uiView.setupDisplayLayer(fill: !isScreenShare)
        VideoFrameRouter.shared.unregister(trackSid: oldSid, view: uiView)
        VideoFrameRouter.shared.register(trackSid: trackSid, view: uiView)
    }

    static func dismantleUIView(_ uiView: VideoDisplayView, coordinator: ()) {
        VideoFrameRouter.shared.unregister(trackSid: uiView.trackSid, view: uiView)
    }
}

class VideoDisplayView: UIView, @unchecked Sendable {
    var trackSid: String = ""
    private var displayLayer: AVSampleBufferDisplayLayer?

    override func layoutSubviews() {
        super.layoutSubviews()
        displayLayer?.frame = bounds
    }

    func setupDisplayLayer(fill: Bool = true) {
        displayLayer?.removeFromSuperlayer()

        let layer = AVSampleBufferDisplayLayer()
        layer.videoGravity = fill ? .resizeAspectFill : .resizeAspect
        layer.frame = bounds
        self.layer.addSublayer(layer)
        displayLayer = layer
    }

    func flushDisplayLayer() {
        guard let layer = displayLayer else { return }
        layer.flush()
        if #available(iOS 17.4, *) {
        } else {
            if layer.status == .failed {
                setupDisplayLayer()
            }
        }
    }

    func enqueueSampleBuffer(_ sampleBuffer: CMSampleBuffer) {
        guard let layer = displayLayer else { return }
        if layer.status == .failed {
            layer.flush()
        }
        layer.enqueue(sampleBuffer)
    }
}
