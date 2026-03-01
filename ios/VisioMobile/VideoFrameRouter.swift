import AVFoundation
import CoreMedia
import CoreVideo

/// Routes I420 video frames from the Rust callback to the correct VideoDisplayView.
///
/// The Rust `visio_video` crate calls `visio_video_set_ios_callback` once at startup.
/// Each frame arrives with a `track_sid`; this singleton looks up the registered
/// VideoDisplayView and enqueues a CMSampleBuffer for display.
final class VideoFrameRouter {
    static let shared = VideoFrameRouter()

    /// Registered views, keyed by track SID.
    private var views: [String: VideoDisplayView] = [:]
    private let lock = NSLock()

    private init() {}

    func register(trackSid: String, view: VideoDisplayView) {
        lock.lock()
        views[trackSid] = view
        lock.unlock()
    }

    func unregister(trackSid: String) {
        lock.lock()
        views.removeValue(forKey: trackSid)
        lock.unlock()
    }

    /// Called from the C callback on a background thread.
    func deliverFrame(
        width: UInt32, height: UInt32,
        yPtr: UnsafePointer<UInt8>, yStride: UInt32,
        uPtr: UnsafePointer<UInt8>, uStride: UInt32,
        vPtr: UnsafePointer<UInt8>, vStride: UInt32,
        trackSid: String
    ) {
        lock.lock()
        let view = views[trackSid]
        lock.unlock()

        guard let view else { return }

        // Create a bi-planar (NV12) CVPixelBuffer from I420 planes.
        // AVSampleBufferDisplayLayer prefers kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange.
        guard let pixelBuffer = createNV12PixelBuffer(
            width: Int(width), height: Int(height),
            yPtr: yPtr, yStride: Int(yStride),
            uPtr: uPtr, uStride: Int(uStride),
            vPtr: vPtr, vStride: Int(vStride)
        ) else { return }

        guard let sampleBuffer = createSampleBuffer(from: pixelBuffer) else { return }

        DispatchQueue.main.async {
            view.enqueueSampleBuffer(sampleBuffer)
        }
    }

    // MARK: - Pixel buffer creation

    /// Convert I420 (Y + U + V planar) to NV12 (Y + interleaved UV).
    /// NV12 is hardware-friendly on iOS and avoids a shader/conversion step.
    private func createNV12PixelBuffer(
        width: Int, height: Int,
        yPtr: UnsafePointer<UInt8>, yStride: Int,
        uPtr: UnsafePointer<UInt8>, uStride: Int,
        vPtr: UnsafePointer<UInt8>, vStride: Int
    ) -> CVPixelBuffer? {
        var pixelBuffer: CVPixelBuffer?
        let status = CVPixelBufferCreate(
            kCFAllocatorDefault,
            width, height,
            kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
            [
                kCVPixelBufferIOSurfacePropertiesKey: [:] as CFDictionary
            ] as CFDictionary,
            &pixelBuffer
        )
        guard status == kCVReturnSuccess, let pb = pixelBuffer else { return nil }

        CVPixelBufferLockBaseAddress(pb, [])
        defer { CVPixelBufferUnlockBaseAddress(pb, []) }

        // Copy Y plane
        if let yDst = CVPixelBufferGetBaseAddressOfPlane(pb, 0) {
            let yDstStride = CVPixelBufferGetBytesPerRowOfPlane(pb, 0)
            for row in 0..<height {
                let src = yPtr.advanced(by: row * yStride)
                let dst = yDst.advanced(by: row * yDstStride).assumingMemoryBound(to: UInt8.self)
                memcpy(dst, src, width)
            }
        }

        // Interleave U + V into NV12 UV plane
        let chromaH = height / 2
        let chromaW = width / 2
        if let uvDst = CVPixelBufferGetBaseAddressOfPlane(pb, 1) {
            let uvDstStride = CVPixelBufferGetBytesPerRowOfPlane(pb, 1)
            for row in 0..<chromaH {
                let uSrc = uPtr.advanced(by: row * uStride)
                let vSrc = vPtr.advanced(by: row * vStride)
                let dst = uvDst.advanced(by: row * uvDstStride).assumingMemoryBound(to: UInt8.self)
                for col in 0..<chromaW {
                    dst[col * 2] = uSrc[col]
                    dst[col * 2 + 1] = vSrc[col]
                }
            }
        }

        return pb
    }

    // MARK: - Sample buffer creation

    private func createSampleBuffer(from pixelBuffer: CVPixelBuffer) -> CMSampleBuffer? {
        var formatDesc: CMVideoFormatDescription?
        let status = CMVideoFormatDescriptionCreateForImageBuffer(
            allocator: kCFAllocatorDefault,
            imageBuffer: pixelBuffer,
            formatDescriptionOut: &formatDesc
        )
        guard status == noErr, let desc = formatDesc else { return nil }

        var timingInfo = CMSampleTimingInfo(
            duration: CMTime.invalid,
            presentationTimeStamp: CMClockGetTime(CMClockGetHostTimeClock()),
            decodeTimeStamp: CMTime.invalid
        )

        var sampleBuffer: CMSampleBuffer?
        let sbStatus = CMSampleBufferCreateReadyWithImageBuffer(
            allocator: kCFAllocatorDefault,
            imageBuffer: pixelBuffer,
            formatDescription: desc,
            sampleTiming: &timingInfo,
            sampleBufferOut: &sampleBuffer
        )
        guard sbStatus == noErr else { return nil }
        return sampleBuffer
    }
}

// MARK: - Global C callback for Rust → Swift video frames

/// This function is registered with `visio_video_set_ios_callback` at app startup.
/// It is called from Rust worker threads whenever a video frame is decoded.
func visioOnVideoFrame(
    width: UInt32, height: UInt32,
    yPtr: UnsafePointer<UInt8>?, yStride: UInt32,
    uPtr: UnsafePointer<UInt8>?, uStride: UInt32,
    vPtr: UnsafePointer<UInt8>?, vStride: UInt32,
    trackSidCStr: UnsafePointer<CChar>?,
    userData: UnsafeMutableRawPointer?
) {
    guard let yPtr, let uPtr, let vPtr, let trackSidCStr else { return }
    let trackSid = String(cString: trackSidCStr)

    VideoFrameRouter.shared.deliverFrame(
        width: width, height: height,
        yPtr: yPtr, yStride: yStride,
        uPtr: uPtr, uStride: uStride,
        vPtr: vPtr, vStride: vStride,
        trackSid: trackSid
    )
}
