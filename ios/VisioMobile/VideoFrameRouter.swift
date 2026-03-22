import AVFoundation
import CoreMedia
import CoreVideo

/// Thread-safe wrapper for CMSampleBuffer, which is CM_SWIFT_NONSENDABLE but is
/// internally reference-counted and safe to pass across isolation boundaries.
struct SendableSampleBuffer: @unchecked Sendable {
    let buffer: CMSampleBuffer
}

final class VideoFrameRouter: @unchecked Sendable {
    static let shared = VideoFrameRouter()

    private var views: [String: VideoDisplayView] = [:]
    private var lastBuffers: [String: CMSampleBuffer] = [:]
    private let lock = NSLock()

    private init() {}

    func register(trackSid: String, view: VideoDisplayView) {
        lock.lock()
        views[trackSid] = view
        let buffered = lastBuffers[trackSid]
        lock.unlock()

        if let buffered, let fresh = VideoFrameRouter.restampSampleBuffer(buffered) {
            let box = SendableSampleBuffer(buffer: fresh)
            DispatchQueue.main.async {
                view.flushDisplayLayer()
                view.enqueueSampleBuffer(box.buffer)
            }
        }
    }

    func clearAll() {
        lock.lock()
        views.removeAll()
        lastBuffers.removeAll()
        lock.unlock()
    }

    func unregister(trackSid: String, view: VideoDisplayView) {
        lock.lock()
        if views[trackSid] === view {
            views.removeValue(forKey: trackSid)
        }
        lock.unlock()
    }

    func invalidateTrack(trackSid: String) {
        lock.lock()
        views.removeValue(forKey: trackSid)
        lastBuffers.removeValue(forKey: trackSid)
        lock.unlock()
    }

    func deliverFrame(
        width: UInt32, height: UInt32,
        yPtr: UnsafePointer<UInt8>, yStride: UInt32,
        uPtr: UnsafePointer<UInt8>, uStride: UInt32,
        vPtr: UnsafePointer<UInt8>, vStride: UInt32,
        trackSid: String
    ) {
        guard let pixelBuffer = createNV12PixelBuffer(
            width: Int(width), height: Int(height),
            yPtr: yPtr, yStride: Int(yStride),
            uPtr: uPtr, uStride: Int(uStride),
            vPtr: vPtr, vStride: Int(vStride)
        ) else { return }

        guard let sampleBuffer = createSampleBuffer(from: pixelBuffer) else { return }

        lock.lock()
        lastBuffers[trackSid] = sampleBuffer
        let view = views[trackSid]
        lock.unlock()

        guard let view else { return }

        let box = SendableSampleBuffer(buffer: sampleBuffer)
        DispatchQueue.main.async {
            view.enqueueSampleBuffer(box.buffer)
        }
    }

    // MARK: - Pixel buffer creation

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

        if let yDst = CVPixelBufferGetBaseAddressOfPlane(pb, 0) {
            let yDstStride = CVPixelBufferGetBytesPerRowOfPlane(pb, 0)
            for row in 0..<height {
                let src = yPtr.advanced(by: row * yStride)
                let dst = yDst.advanced(by: row * yDstStride).assumingMemoryBound(to: UInt8.self)
                memcpy(dst, src, width)
            }
        }

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

    static func restampSampleBuffer(_ original: CMSampleBuffer) -> CMSampleBuffer? {
        guard let imageBuffer = CMSampleBufferGetImageBuffer(original) else { return nil }

        var formatDesc: CMVideoFormatDescription?
        let fdStatus = CMVideoFormatDescriptionCreateForImageBuffer(
            allocator: kCFAllocatorDefault,
            imageBuffer: imageBuffer,
            formatDescriptionOut: &formatDesc
        )
        guard fdStatus == noErr, let desc = formatDesc else { return nil }

        var timingInfo = CMSampleTimingInfo(
            duration: CMTime.invalid,
            presentationTimeStamp: CMClockGetTime(CMClockGetHostTimeClock()),
            decodeTimeStamp: CMTime.invalid
        )

        var restamped: CMSampleBuffer?
        let sbStatus = CMSampleBufferCreateReadyWithImageBuffer(
            allocator: kCFAllocatorDefault,
            imageBuffer: imageBuffer,
            formatDescription: desc,
            sampleTiming: &timingInfo,
            sampleBufferOut: &restamped
        )
        guard sbStatus == noErr else { return nil }
        return restamped
    }

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
