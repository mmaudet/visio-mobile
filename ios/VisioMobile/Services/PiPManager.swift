import AVKit
import CoreMedia
import CoreVideo

/// Manages Picture-in-Picture for background video during calls.
///
/// Uses AVPictureInPictureController with AVSampleBufferDisplayLayer.
/// Active speaker video frames are pushed from the video callback.
///
/// Flow:
/// 1. App backgrounds (scenePhase == .background) AND call active -> start PiP
/// 2. PiP shows active speaker video
/// 3. Tap PiP -> return to app
/// 4. Close PiP -> audio-only (call continues, PiP closes)
/// 5. App foregrounds -> PiP closes, back to CallView
class PiPManager: NSObject, AVPictureInPictureControllerDelegate {

    static let shared = PiPManager()

    private var pipController: AVPictureInPictureController?
    private let displayLayer = AVSampleBufferDisplayLayer()
    private var isSetUp = false

    override init() {
        super.init()
        displayLayer.videoGravity = .resizeAspect
    }

    // MARK: - Public API

    /// Set up PiP controller. Call once when entering a call.
    func setup() {
        guard !isSetUp else { return }
        guard AVPictureInPictureController.isPictureInPictureSupported() else {
            NSLog("PiPManager: PiP not supported on this device")
            return
        }

        let source = AVPictureInPictureController.ContentSource(
            sampleBufferDisplayLayer: displayLayer,
            playbackDelegate: self
        )
        pipController = AVPictureInPictureController(contentSource: source)
        pipController?.delegate = self
        isSetUp = true
        NSLog("PiPManager: setup complete")
    }

    /// Push a video frame to the PiP display layer.
    func pushFrame(_ pixelBuffer: CVPixelBuffer, timestamp: CMTime) {
        var formatDesc: CMFormatDescription?
        CMVideoFormatDescriptionCreateForImageBuffer(
            allocator: nil,
            imageBuffer: pixelBuffer,
            formatDescriptionOut: &formatDesc
        )
        guard let format = formatDesc else { return }

        var timingInfo = CMSampleTimingInfo(
            duration: .invalid,
            presentationTimeStamp: timestamp,
            decodeTimeStamp: .invalid
        )

        var sampleBuffer: CMSampleBuffer?
        CMSampleBufferCreateForImageBuffer(
            allocator: nil,
            imageBuffer: pixelBuffer,
            dataReady: true,
            makeDataReadyCallback: nil,
            refcon: nil,
            formatDescription: format,
            sampleTiming: &timingInfo,
            sampleBufferOut: &sampleBuffer
        )

        if let sb = sampleBuffer {
            displayLayer.enqueue(sb)
        }
    }

    /// Start PiP if the controller is ready.
    func startIfNeeded() {
        guard let pipController, !pipController.isPictureInPictureActive else { return }
        pipController.startPictureInPicture()
        NSLog("PiPManager: starting PiP")
    }

    /// Stop PiP.
    func stop() {
        guard let pipController, pipController.isPictureInPictureActive else { return }
        pipController.stopPictureInPicture()
        NSLog("PiPManager: stopping PiP")
    }

    /// Tear down PiP when leaving a call.
    func tearDown() {
        stop()
        pipController = nil
        isSetUp = false
    }

    // MARK: - AVPictureInPictureControllerDelegate

    func pictureInPictureControllerWillStartPictureInPicture(_ controller: AVPictureInPictureController) {
        NSLog("PiPManager: will start PiP")
    }

    func pictureInPictureControllerDidStartPictureInPicture(_ controller: AVPictureInPictureController) {
        NSLog("PiPManager: did start PiP")
    }

    func pictureInPictureControllerDidStopPictureInPicture(_ controller: AVPictureInPictureController) {
        NSLog("PiPManager: did stop PiP -- call continues audio-only")
    }

    func pictureInPictureController(_ controller: AVPictureInPictureController, failedToStartPictureInPictureWithError error: Error) {
        NSLog("PiPManager: failed to start PiP: \(error.localizedDescription)")
    }

    func pictureInPictureController(_ controller: AVPictureInPictureController, restoreUserInterfaceForPictureInPictureStopWithCompletionHandler completionHandler: @escaping (Bool) -> Void) {
        // User tapped PiP to return to the app
        NSLog("PiPManager: restore UI from PiP")
        completionHandler(true)
    }
}

// MARK: - AVPictureInPictureSampleBufferPlaybackDelegate

extension PiPManager: AVPictureInPictureSampleBufferPlaybackDelegate {

    func pictureInPictureController(_ controller: AVPictureInPictureController, setPlaying playing: Bool) {
        // No-op: live video, not playback
    }

    func pictureInPictureControllerTimeRangeForPlayback(_ controller: AVPictureInPictureController) -> CMTimeRange {
        CMTimeRange(start: .negativeInfinity, duration: .positiveInfinity)
    }

    func pictureInPictureControllerIsPlaybackPaused(_ controller: AVPictureInPictureController) -> Bool {
        false
    }

    func pictureInPictureController(_ controller: AVPictureInPictureController, didTransitionToRenderSize newRenderSize: CMVideoDimensions) {
        // Could adjust video quality here for smaller PiP window
    }

    func pictureInPictureController(_ controller: AVPictureInPictureController, skipByInterval skipInterval: CMTime, completion completionHandler: @escaping () -> Void) {
        completionHandler()
    }
}
