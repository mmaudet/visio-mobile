use ort::session::Session;
use ort::value::Tensor;

/// Run selfie segmentation on an RGB image.
/// Input: 256x256 RGB image (packed u8).
/// Output: 256x256 mask (f32, 0.0=background, 1.0=person).
pub fn segment(session: &mut Session, rgb_256: &[u8]) -> Result<Vec<f32>, String> {
    assert_eq!(rgb_256.len(), 256 * 256 * 3);

    // Normalize to [0, 1] and reshape to NCHW: [1, 3, 256, 256]
    let mut input = vec![0.0f32; 1 * 3 * 256 * 256];
    for i in 0..(256 * 256) {
        input[i] = rgb_256[i * 3] as f32 / 255.0; // R
        input[256 * 256 + i] = rgb_256[i * 3 + 1] as f32 / 255.0; // G
        input[2 * 256 * 256 + i] = rgb_256[i * 3 + 2] as f32 / 255.0; // B
    }

    let input_tensor = Tensor::from_array(([1usize, 3, 256, 256], input.into_boxed_slice()))
        .map_err(|e| format!("ort tensor: {e}"))?;

    let outputs = session
        .run(ort::inputs![input_tensor])
        .map_err(|e| format!("ort run: {e}"))?;

    let (_shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("ort extract: {e}"))?;

    // Output is [1, 1, 256, 256] or [1, 256, 256] — flatten to 256*256
    let mask: Vec<f32> = data.to_vec();
    Ok(mask)
}

/// Resize a 256x256 f32 mask to target dimensions using bilinear interpolation.
pub fn resize_mask(mask: &[f32], dst_w: usize, dst_h: usize) -> Vec<f32> {
    let src_w = 256;
    let src_h = 256;
    let mut dst = vec![0.0f32; dst_w * dst_h];
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
            dst[y * dst_w + x] = mask[y0 * src_w + x0] * (1.0 - fx) * (1.0 - fy)
                + mask[y0 * src_w + x1] * fx * (1.0 - fy)
                + mask[y1 * src_w + x0] * (1.0 - fx) * fy
                + mask[y1 * src_w + x1] * fx * fy;
        }
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_mask_all_ones() {
        let mask = vec![1.0f32; 256 * 256];
        let resized = resize_mask(&mask, 640, 480);
        assert_eq!(resized.len(), 640 * 480);
        for &v in &resized {
            assert!((v - 1.0).abs() < 1e-5, "expected ~1.0, got {v}");
        }
    }

    #[test]
    fn resize_mask_all_zeros() {
        let mask = vec![0.0f32; 256 * 256];
        let resized = resize_mask(&mask, 1920, 1080);
        assert_eq!(resized.len(), 1920 * 1080);
        for &v in &resized {
            assert!(v.abs() < 1e-5, "expected ~0.0, got {v}");
        }
    }

    #[test]
    fn resize_mask_identity() {
        // Resizing to same size should preserve values
        let mut mask = vec![0.0f32; 256 * 256];
        for i in 0..mask.len() {
            mask[i] = (i as f32) / (256.0 * 256.0);
        }
        let resized = resize_mask(&mask, 256, 256);
        assert_eq!(resized.len(), 256 * 256);
        for (i, (&orig, &res)) in mask.iter().zip(resized.iter()).enumerate() {
            assert!(
                (orig - res).abs() < 1e-4,
                "mismatch at {i}: orig={orig}, resized={res}"
            );
        }
    }
}
