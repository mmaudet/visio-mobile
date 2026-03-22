import CoreVideo
import visioFFI

/// Convert an NV12 CVPixelBuffer to I420 planes and call `handler` with the Y/U/V pointers and strides.
private func withI420Planes(
    _ pixelBuffer: CVPixelBuffer,
    uPlane: inout [UInt8],
    vPlane: inout [UInt8],
    handler: (UnsafePointer<UInt8>, UInt32, UnsafePointer<UInt8>, UInt32, UnsafePointer<UInt8>, UInt32, UInt32, UInt32) -> Void
) {
    CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
    defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

    let width = CVPixelBufferGetWidth(pixelBuffer)
    let height = CVPixelBufferGetHeight(pixelBuffer)
    let chromaW = width / 2
    let chromaH = height / 2
    let chromaSize = chromaW * chromaH

    guard let yBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 0),
          let uvBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 1) else { return }

    let yStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 0)
    let uvStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 1)

    let yPtr = yBase.assumingMemoryBound(to: UInt8.self)
    let uvPtr = uvBase.assumingMemoryBound(to: UInt8.self)

    // Resize buffers only when resolution changes.
    if uPlane.count != chromaSize {
        uPlane = [UInt8](repeating: 0, count: chromaSize)
        vPlane = [UInt8](repeating: 0, count: chromaSize)
    }

    // De-interleave NV12 UV plane into separate U and V planes (I420).
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
            guard let uBase = uBuf.baseAddress,
                  let vBase = vBuf.baseAddress else { return }
            handler(yPtr, UInt32(yStride), uBase, UInt32(chromaW), vBase, UInt32(chromaW), UInt32(width), UInt32(height))
        }
    }
}

/// Push an NV12 CVPixelBuffer to Rust as I420 via the C FFI (live call path).
func pushNV12FrameToRust(
    _ pixelBuffer: CVPixelBuffer,
    uPlane: inout [UInt8],
    vPlane: inout [UInt8]
) {
    withI420Planes(pixelBuffer, uPlane: &uPlane, vPlane: &vPlane) { yPtr, yStride, uPtr, uStride, vPtr, vStride, width, height in
        visio_push_ios_camera_frame(yPtr, yStride, uPtr, uStride, vPtr, vStride, width, height)
    }
}

