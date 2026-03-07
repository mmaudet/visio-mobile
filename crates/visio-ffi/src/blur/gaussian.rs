/// Fast Gaussian blur approximation using 3-pass box blur on individual image planes.
///
/// A 3-pass box blur closely approximates a Gaussian blur and runs in O(n) per pass,
/// independent of the blur radius. This is used to blur the Y, U, and V planes of
/// an I420 frame independently.

/// Apply a 3-pass box blur approximation of Gaussian blur on a single plane.
///
/// * `data`   — pixel values (may include stride padding)
/// * `width`  — plane width in pixels
/// * `height` — plane height in pixels
/// * `stride` — row stride in bytes (>= width)
/// * `radius` — blur radius in pixels
///
/// Returns a new buffer with the blurred plane, packed (width * height, no stride padding).
pub fn blur_plane(
    data: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    radius: usize,
) -> Vec<u8> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let r = radius.min(width.saturating_sub(1)).min(height.saturating_sub(1));

    let mut src = extract_plane(data, width, height, stride);
    let mut dst = vec![0u8; width * height];
    // 3-pass box blur
    for _ in 0..3 {
        box_blur_h(&src, &mut dst, width, height, r);
        box_blur_v(&dst, &mut src, width, height, r);
    }
    src
}

/// Copy plane pixels out of a strided buffer into a packed (width*height) buffer.
fn extract_plane(data: &[u8], width: usize, height: usize, stride: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height];
    for row in 0..height {
        out[row * width..(row + 1) * width]
            .copy_from_slice(&data[row * stride..row * stride + width]);
    }
    out
}

/// Horizontal box blur with clamped-edge sampling.
fn box_blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let diameter = (2 * r + 1) as u32;
    for y in 0..h {
        let row = y * w;
        // Build initial window sum for x=0: samples from index -r..=+r,
        // clamping out-of-bounds indices to the nearest edge pixel.
        let mut sum: u32 = 0;
        for i in 0..=2 * r {
            let sx = (i as isize - r as isize).max(0).min(w as isize - 1) as usize;
            sum += src[row + sx] as u32;
        }
        dst[row] = (sum / diameter) as u8;

        // Slide the window across the row.
        for x in 1..w {
            // Add the new pixel entering on the right.
            let right = (x + r).min(w - 1);
            sum += src[row + right] as u32;
            // Remove the pixel leaving on the left.
            let left = (x as isize - r as isize - 1).max(0) as usize;
            sum -= src[row + left] as u32;
            dst[row + x] = (sum / diameter) as u8;
        }
    }
}

/// Vertical box blur with clamped-edge sampling.
fn box_blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let diameter = (2 * r + 1) as u32;
    for x in 0..w {
        // Build initial window sum for y=0.
        let mut sum: u32 = 0;
        for i in 0..=2 * r {
            let sy = (i as isize - r as isize).max(0).min(h as isize - 1) as usize;
            sum += src[sy * w + x] as u32;
        }
        dst[x] = (sum / diameter) as u8;

        // Slide the window down the column.
        for y in 1..h {
            let bottom = (y + r).min(h - 1);
            sum += src[bottom * w + x] as u32;
            let top = (y as isize - r as isize - 1).max(0) as usize;
            sum -= src[top * w + x] as u32;
            dst[y * w + x] = (sum / diameter) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_uniform_plane_unchanged() {
        let data = vec![128u8; 10 * 10];
        let result = blur_plane(&data, 10, 10, 10, 3);
        // Uniform input must remain uniform.
        for &v in &result {
            assert_eq!(v, 128, "uniform plane should stay exactly 128");
        }
    }

    #[test]
    fn blur_reduces_contrast() {
        let mut data = vec![0u8; 10 * 10];
        // Single white pixel in the middle.
        data[5 * 10 + 5] = 255;
        let result = blur_plane(&data, 10, 10, 10, 2);
        // Center should be dimmer than 255; its neighbor should be brighter than 0.
        assert!(result[5 * 10 + 5] < 255);
        assert!(result[5 * 10 + 4] > 0);
    }

    #[test]
    fn blur_with_stride() {
        // Plane is 4 wide but stride is 8 (extra padding bytes).
        let width = 4;
        let height = 4;
        let stride = 8;
        let mut data = vec![0u8; stride * height];
        for row in 0..height {
            for col in 0..width {
                data[row * stride + col] = 100;
            }
        }
        let result = blur_plane(&data, width, height, stride, 1);
        assert_eq!(result.len(), width * height);
        // All pixels were 100, so output must stay 100.
        for &v in &result {
            assert_eq!(v, 100);
        }
    }

    #[test]
    fn blur_radius_zero_is_identity() {
        let data: Vec<u8> = (0..25).map(|i| (i * 10) as u8).collect();
        let result = blur_plane(&data, 5, 5, 5, 0);
        assert_eq!(result, data);
    }

    #[test]
    fn blur_empty_plane() {
        let result = blur_plane(&[], 0, 0, 0, 5);
        assert!(result.is_empty());
    }
}
