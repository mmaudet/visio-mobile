/// I420 ↔ RGB conversion utilities for the background blur pipeline.
///
/// The segmentation model expects packed RGB input, while camera frames
/// arrive as I420 (YUV 4:2:0 planar). These functions handle the
/// bidirectional conversion using BT.601 full-range coefficients.

/// Convert I420 planes to packed RGB (BT.601 full-range).
/// Output: `Vec<u8>` of length `width * height * 3`.
pub fn i420_to_rgb(
    y: &[u8],
    u: &[u8],
    v: &[u8],
    width: usize,
    height: usize,
    stride_y: usize,
    stride_u: usize,
    stride_v: usize,
) -> Vec<u8> {
    let mut rgb = vec![0u8; width * height * 3];
    for row in 0..height {
        for col in 0..width {
            let y_val = y[row * stride_y + col] as f32;
            let u_val = u[(row / 2) * stride_u + col / 2] as f32 - 128.0;
            let v_val = v[(row / 2) * stride_v + col / 2] as f32 - 128.0;
            let r = (y_val + 1.402 * v_val).clamp(0.0, 255.0) as u8;
            let g = (y_val - 0.344 * u_val - 0.714 * v_val).clamp(0.0, 255.0) as u8;
            let b = (y_val + 1.772 * u_val).clamp(0.0, 255.0) as u8;
            let idx = (row * width + col) * 3;
            rgb[idx] = r;
            rgb[idx + 1] = g;
            rgb[idx + 2] = b;
        }
    }
    rgb
}

/// Resize RGB image to target dimensions using bilinear interpolation.
pub fn resize_rgb(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    let mut dst = vec![0u8; dst_w * dst_h * 3];
    let x_ratio = src_w as f32 / dst_w as f32;
    let y_ratio = src_h as f32 / dst_h as f32;
    for y in 0..dst_h {
        for x in 0..dst_w {
            let src_x = x as f32 * x_ratio;
            let src_y = y as f32 * y_ratio;
            let x0 = src_x as usize;
            let y0 = src_y as usize;
            let x1 = (x0 + 1).min(src_w - 1);
            let y1 = (y0 + 1).min(src_h - 1);
            let fx = src_x - x0 as f32;
            let fy = src_y - y0 as f32;
            for c in 0..3 {
                let v00 = src[(y0 * src_w + x0) * 3 + c] as f32;
                let v10 = src[(y0 * src_w + x1) * 3 + c] as f32;
                let v01 = src[(y1 * src_w + x0) * 3 + c] as f32;
                let v11 = src[(y1 * src_w + x1) * 3 + c] as f32;
                let val = v00 * (1.0 - fx) * (1.0 - fy)
                    + v10 * fx * (1.0 - fy)
                    + v01 * (1.0 - fx) * fy
                    + v11 * fx * fy;
                dst[(y * dst_w + x) * 3 + c] = val.clamp(0.0, 255.0) as u8;
            }
        }
    }
    dst
}

/// Rotate packed RGB image by 0, 90, 180, or 270 degrees clockwise.
/// For 90/270, the output dimensions are swapped (width↔height).
pub fn rotate_rgb(src: &[u8], width: usize, height: usize, degrees: u32) -> Vec<u8> {
    match degrees {
        0 | 360 => src.to_vec(),
        90 => {
            // 90° CW: dst[x, height-1-y] = src[y, x]
            let dst_w = height;
            let dst_h = width;
            let mut dst = vec![0u8; dst_w * dst_h * 3];
            for y in 0..height {
                for x in 0..width {
                    let src_idx = (y * width + x) * 3;
                    let dx = height - 1 - y;
                    let dy = x;
                    let dst_idx = (dy * dst_w + dx) * 3;
                    dst[dst_idx..dst_idx + 3].copy_from_slice(&src[src_idx..src_idx + 3]);
                }
            }
            dst
        }
        180 => {
            let mut dst = vec![0u8; width * height * 3];
            for y in 0..height {
                for x in 0..width {
                    let src_idx = (y * width + x) * 3;
                    let dst_idx = ((height - 1 - y) * width + (width - 1 - x)) * 3;
                    dst[dst_idx..dst_idx + 3].copy_from_slice(&src[src_idx..src_idx + 3]);
                }
            }
            dst
        }
        270 => {
            // 270° CW (= 90° CCW): dst[y, width-1-x] = src[y_src, x_src]
            let dst_w = height;
            let dst_h = width;
            let mut dst = vec![0u8; dst_w * dst_h * 3];
            for y in 0..height {
                for x in 0..width {
                    let src_idx = (y * width + x) * 3;
                    let dx = y;
                    let dy = width - 1 - x;
                    let dst_idx = (dy * dst_w + dx) * 3;
                    dst[dst_idx..dst_idx + 3].copy_from_slice(&src[src_idx..src_idx + 3]);
                }
            }
            dst
        }
        _ => src.to_vec(),
    }
}

/// Decode JPEG bytes to packed RGB.
///
/// Note: This expects the JPEG to contain RGB pixel data (not grayscale).
/// Most camera and photo JPEGs are RGB. Grayscale JPEGs will return
/// single-channel data which would be misinterpreted as RGB.
pub fn decode_jpeg_to_rgb(jpeg_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = jpeg_decoder::Decoder::new(jpeg_bytes);
    let pixels = decoder.decode().map_err(|e| e.to_string())?;
    Ok(pixels)
}

/// Get JPEG dimensions without fully decoding pixel data.
pub fn jpeg_dimensions(jpeg_bytes: &[u8]) -> Result<(usize, usize), String> {
    let mut decoder = jpeg_decoder::Decoder::new(jpeg_bytes);
    decoder.read_info().map_err(|e| e.to_string())?;
    let info = decoder.info().ok_or("no JPEG info")?;
    Ok((info.width as usize, info.height as usize))
}

/// Convert packed RGB to I420 planes (BT.601 full-range).
///
/// Returns `(Y, U, V)` planes. The U and V planes are subsampled 2x2.
/// Width and height should be even for correct chroma subsampling.
pub fn rgb_to_i420(rgb: &[u8], width: usize, height: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut y = vec![0u8; width * height];
    let uv_w = width / 2;
    let uv_h = height / 2;
    let mut u_plane = vec![0u8; uv_w * uv_h];
    let mut v_plane = vec![0u8; uv_w * uv_h];
    for row in 0..height {
        for col in 0..width {
            let idx = (row * width + col) * 3;
            let r = rgb[idx] as f32;
            let g = rgb[idx + 1] as f32;
            let b = rgb[idx + 2] as f32;
            y[row * width + col] = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
            if row % 2 == 0 && col % 2 == 0 {
                let uv_idx = (row / 2) * uv_w + col / 2;
                u_plane[uv_idx] =
                    (-0.169 * r - 0.331 * g + 0.500 * b + 128.0).clamp(0.0, 255.0) as u8;
                v_plane[uv_idx] =
                    (0.500 * r - 0.419 * g - 0.081 * b + 128.0).clamp(0.0, 255.0) as u8;
            }
        }
    }
    (y, u_plane, v_plane)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i420_to_rgb_black_frame() {
        // Y=0, U=128, V=128 -> RGB(0, 0, 0)
        let y = vec![0u8; 4];
        let u = vec![128u8; 1];
        let v = vec![128u8; 1];
        let rgb = i420_to_rgb(&y, &u, &v, 2, 2, 2, 1, 1);
        assert!(rgb.iter().all(|&b| b == 0));
    }

    #[test]
    fn i420_to_rgb_white_frame() {
        // Y=255, U=128, V=128 -> RGB(255, 255, 255)
        let y = vec![255u8; 4];
        let u = vec![128u8; 1];
        let v = vec![128u8; 1];
        let rgb = i420_to_rgb(&y, &u, &v, 2, 2, 2, 1, 1);
        assert!(rgb.iter().all(|&b| b == 255));
    }

    #[test]
    fn resize_rgb_identity() {
        let src = vec![100u8; 2 * 2 * 3];
        let dst = resize_rgb(&src, 2, 2, 2, 2);
        assert_eq!(src, dst);
    }

    #[test]
    fn resize_rgb_downsample() {
        let src = vec![200u8; 4 * 4 * 3];
        let dst = resize_rgb(&src, 4, 4, 2, 2);
        assert_eq!(dst.len(), 2 * 2 * 3);
        assert!(dst.iter().all(|&b| b == 200));
    }

    #[test]
    fn rgb_roundtrip_black() {
        // Black: RGB(0,0,0) -> I420 -> RGB should stay near black
        let rgb = vec![0u8; 4 * 4 * 3];
        let (y, u, v) = rgb_to_i420(&rgb, 4, 4);
        let back = i420_to_rgb(&y, &u, &v, 4, 4, 4, 2, 2);
        // Allow small rounding error
        assert!(back.iter().all(|&b| b <= 2));
    }

    #[test]
    fn rgb_roundtrip_white() {
        // White: RGB(255,255,255) -> I420 -> RGB should stay near white
        let rgb = vec![255u8; 4 * 4 * 3];
        let (y, u, v) = rgb_to_i420(&rgb, 4, 4);
        let back = i420_to_rgb(&y, &u, &v, 4, 4, 4, 2, 2);
        assert!(back.iter().all(|&b| b >= 253));
    }
}
