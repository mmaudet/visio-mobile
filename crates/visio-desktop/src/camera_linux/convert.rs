//! Shared pixel format conversion functions for Linux camera backends.
//!
//! Used by both the PipeWire and V4L2 backends to convert captured frames
//! into I420 format for the LiveKit video pipeline.

/// Decode MJPEG frame to RGB using the image crate.
pub fn decode_mjpeg(data: &[u8]) -> Result<Vec<u8>, String> {
    use image::io::Reader as ImageReader;
    use std::io::Cursor;

    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess format: {e}"))?;
    let img = reader
        .decode()
        .map_err(|e| format!("Failed to decode JPEG: {e}"))?;
    Ok(img.to_rgb8().into_raw())
}

/// Convert RGB24 to I420 (BT.601 full range).
pub fn rgb_to_i420(
    rgb: &[u8],
    width: usize,
    height: usize,
    y_dst: &mut [u8],
    y_stride: usize,
    u_dst: &mut [u8],
    u_stride: usize,
    v_dst: &mut [u8],
    v_stride: usize,
) {
    for row in 0..height {
        for col in 0..width {
            let rgb_idx = (row * width + col) * 3;
            let r = rgb[rgb_idx] as f32;
            let g = rgb[rgb_idx + 1] as f32;
            let b = rgb[rgb_idx + 2] as f32;

            let y = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
            y_dst[row * y_stride + col] = y;

            if row % 2 == 0 && col % 2 == 0 {
                let u = ((-0.169 * r - 0.331 * g + 0.5 * b) + 128.0).clamp(0.0, 255.0) as u8;
                let v = ((0.5 * r - 0.419 * g - 0.081 * b) + 128.0).clamp(0.0, 255.0) as u8;
                let cr = row / 2;
                let cc = col / 2;
                u_dst[cr * u_stride + cc] = u;
                v_dst[cr * v_stride + cc] = v;
            }
        }
    }
}

/// Convert YUYV (YUV 4:2:2) directly to I420 — single pass, no intermediate RGB.
pub fn yuyv_to_i420(
    data: &[u8],
    width: usize,
    height: usize,
    y_dst: &mut [u8],
    y_stride: usize,
    u_dst: &mut [u8],
    u_stride: usize,
    v_dst: &mut [u8],
    v_stride: usize,
) {
    for row in 0..height {
        for col in (0..width).step_by(2) {
            let base = (row * width + col) * 2;
            let y0 = data[base];
            let u = data[base + 1];
            let y1 = data[base + 2];
            let v = data[base + 3];

            y_dst[row * y_stride + col] = y0;
            y_dst[row * y_stride + col + 1] = y1;

            if row % 2 == 0 {
                let cr = row / 2;
                let cc = col / 2;
                u_dst[cr * u_stride + cc] = u;
                v_dst[cr * v_stride + cc] = v;
            }
        }
    }
}

/// Convert NV12 (Y plane + interleaved UV plane) to I420 (separate Y, U, V planes).
pub fn nv12_to_i420(
    nv12: &[u8],
    width: usize,
    height: usize,
    y_dst: &mut [u8],
    y_stride: usize,
    u_dst: &mut [u8],
    u_stride: usize,
    v_dst: &mut [u8],
    v_stride: usize,
) {
    let nv12_y_stride = width;
    let nv12_uv_offset = width * height;
    let nv12_uv_stride = width;

    // Copy Y plane
    for row in 0..height {
        let src_start = row * nv12_y_stride;
        let dst_start = row * y_stride;
        y_dst[dst_start..dst_start + width].copy_from_slice(&nv12[src_start..src_start + width]);
    }

    // Split interleaved UV into separate U and V planes
    let chroma_height = height / 2;
    let chroma_width = width / 2;
    for row in 0..chroma_height {
        let uv_row_start = nv12_uv_offset + row * nv12_uv_stride;
        for col in 0..chroma_width {
            u_dst[row * u_stride + col] = nv12[uv_row_start + col * 2];
            v_dst[row * v_stride + col] = nv12[uv_row_start + col * 2 + 1];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_i420_pure_white() {
        let width = 4;
        let height = 4;
        let rgb = vec![255u8; width * height * 3];
        let mut y = vec![0u8; width * height];
        let mut u = vec![0u8; (width / 2) * (height / 2)];
        let mut v = vec![0u8; (width / 2) * (height / 2)];
        rgb_to_i420(
            &rgb,
            width,
            height,
            &mut y,
            width,
            &mut u,
            width / 2,
            &mut v,
            width / 2,
        );
        assert!(y.iter().all(|&val| val == 255));
        assert!(u.iter().all(|&val| (val as i16 - 128).abs() <= 1));
        assert!(v.iter().all(|&val| (val as i16 - 128).abs() <= 1));
    }

    #[test]
    fn test_rgb_to_i420_pure_black() {
        let width = 4;
        let height = 4;
        let rgb = vec![0u8; width * height * 3];
        let mut y = vec![255u8; width * height];
        let mut u = vec![255u8; (width / 2) * (height / 2)];
        let mut v = vec![255u8; (width / 2) * (height / 2)];
        rgb_to_i420(
            &rgb,
            width,
            height,
            &mut y,
            width,
            &mut u,
            width / 2,
            &mut v,
            width / 2,
        );
        assert!(y.iter().all(|&val| val == 0));
        assert!(u.iter().all(|&val| val == 128));
        assert!(v.iter().all(|&val| val == 128));
    }

    #[test]
    fn test_yuyv_to_i420_roundtrip_luma() {
        let width = 4;
        let height = 2;
        let mut yuyv = vec![0u8; width * height * 2];
        for i in 0..(width * height / 2) {
            yuyv[i * 4] = 100; // Y0
            yuyv[i * 4 + 1] = 128; // U
            yuyv[i * 4 + 2] = 100; // Y1
            yuyv[i * 4 + 3] = 128; // V
        }
        let mut y = vec![0u8; width * height];
        let mut u = vec![0u8; (width / 2) * (height / 2)];
        let mut v = vec![0u8; (width / 2) * (height / 2)];
        yuyv_to_i420(
            &yuyv,
            width,
            height,
            &mut y,
            width,
            &mut u,
            width / 2,
            &mut v,
            width / 2,
        );
        assert!(y.iter().all(|&val| val == 100));
        assert!(u.iter().all(|&val| val == 128));
        assert!(v.iter().all(|&val| val == 128));
    }

    #[test]
    fn test_nv12_to_i420_basic() {
        let width = 4;
        let height = 4;
        let y_size = width * height;
        let uv_size = width * (height / 2);
        let mut nv12 = vec![0u8; y_size + uv_size];
        for i in 0..y_size {
            nv12[i] = 200;
        }
        for i in 0..(width / 2 * height / 2) {
            nv12[y_size + i * 2] = 50;
            nv12[y_size + i * 2 + 1] = 180;
        }
        let mut y_dst = vec![0u8; width * height];
        let mut u_dst = vec![0u8; (width / 2) * (height / 2)];
        let mut v_dst = vec![0u8; (width / 2) * (height / 2)];
        nv12_to_i420(
            &nv12,
            width,
            height,
            &mut y_dst,
            width,
            &mut u_dst,
            width / 2,
            &mut v_dst,
            width / 2,
        );
        assert!(y_dst.iter().all(|&val| val == 200));
        assert!(u_dst.iter().all(|&val| val == 50));
        assert!(v_dst.iter().all(|&val| val == 180));
    }
}
