use super::{convert, gaussian, model, segment};
use std::sync::Mutex;

/// Background mode: Off, Blur, or Image replacement (by image ID 1-8).
#[derive(Clone, Debug, PartialEq)]
pub enum BackgroundMode {
    Off,
    Blur,
    Image(u8), // 1-8, corresponds to assets/backgrounds/{id}.jpg
}

static MODE: Mutex<BackgroundMode> = Mutex::new(BackgroundMode::Off);

/// Cached replacement image in I420 format, resized to current frame dimensions.
static REPLACEMENT_CACHE: Mutex<Option<ReplacementImage>> = Mutex::new(None);

/// Raw JPEG bytes for the current replacement image (used to re-generate I420
/// when rotation or frame dimensions change).
static REPLACEMENT_JPEG: Mutex<Option<(u8, Vec<u8>)>> = Mutex::new(None);

struct ReplacementImage {
    id: u8,
    width: usize,
    height: usize,
    rotation: u32,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

const Y_BLUR_RADIUS: usize = 15;
const UV_BLUR_RADIUS: usize = 7;

/// Linearly interpolate between background and foreground pixel values.
/// `mask` is in [0.0, 1.0]: 0.0 = full background, 1.0 = full foreground.
fn lerp_u8(bg: u8, fg: u8, mask: f32) -> u8 {
    (bg as f32 * (1.0 - mask) + fg as f32 * mask + 0.5) as u8
}

pub struct BlurProcessor;

impl BlurProcessor {
    /// Set the current background mode.
    /// Clears the replacement image cache when switching away from Image mode
    /// or switching to a different image ID.
    pub fn set_mode(mode: BackgroundMode) {
        let should_clear = match &mode {
            BackgroundMode::Image(new_id) => {
                let cache = REPLACEMENT_CACHE.lock().unwrap();
                cache.as_ref().map(|c| c.id != *new_id).unwrap_or(false)
            }
            _ => true,
        };
        if should_clear {
            *REPLACEMENT_CACHE.lock().unwrap() = None;
        }
        *MODE.lock().unwrap() = mode;
    }

    /// Get the current background mode.
    pub fn get_mode() -> BackgroundMode {
        MODE.lock().unwrap().clone()
    }

    /// Store JPEG bytes for a replacement image. The actual I420 conversion
    /// is deferred to `process_i420` where the real frame dimensions and
    /// rotation are known.
    pub fn load_replacement_image(
        id: u8,
        jpeg_bytes: &[u8],
        _target_w: usize,
        _target_h: usize,
    ) -> Result<(), String> {
        // Validate that the JPEG is decodable
        convert::jpeg_dimensions(jpeg_bytes)?;
        *REPLACEMENT_JPEG.lock().map_err(|e| e.to_string())? =
            Some((id, jpeg_bytes.to_vec()));
        // Invalidate cached I420 so it gets regenerated with correct dimensions/rotation
        *REPLACEMENT_CACHE.lock().map_err(|e| e.to_string())? = None;
        Ok(())
    }

    /// Generate (or return cached) I420 replacement image for the given frame
    /// dimensions and rotation.
    fn get_replacement(
        id: u8,
        frame_w: usize,
        frame_h: usize,
        rotation: u32,
    ) -> Option<()> {
        // Check if cache is already valid
        {
            let cache = REPLACEMENT_CACHE.lock().ok()?;
            if let Some(ref r) = *cache {
                if r.id == id && r.width == frame_w && r.height == frame_h && r.rotation == rotation
                {
                    return Some(());
                }
            }
        }

        // Need to regenerate — get JPEG bytes
        let jpeg_guard = REPLACEMENT_JPEG.lock().ok()?;
        let (stored_id, jpeg_bytes) = jpeg_guard.as_ref()?;
        if *stored_id != id {
            return None;
        }

        let rgb = convert::decode_jpeg_to_rgb(jpeg_bytes).ok()?;
        let (src_w, src_h) = convert::jpeg_dimensions(jpeg_bytes).ok()?;

        // Pre-rotate: apply inverse rotation so the image appears correct
        // after the display rotation is applied.
        let pre_rot = (360 - rotation) % 360;
        let (target_w, target_h) = if pre_rot == 90 || pre_rot == 270 {
            // After rotating 90/270, width and height swap
            (frame_h, frame_w)
        } else {
            (frame_w, frame_h)
        };

        let resized = convert::resize_rgb(&rgb, src_w, src_h, target_w, target_h);
        let rotated = convert::rotate_rgb(&resized, target_w, target_h, pre_rot);
        let (y, u, v) = convert::rgb_to_i420(&rotated, frame_w, frame_h);

        let mut cache = REPLACEMENT_CACHE.lock().ok()?;
        *cache = Some(ReplacementImage {
            id,
            width: frame_w,
            height: frame_h,
            rotation,
            y,
            u,
            v,
        });
        Some(())
    }

    /// Process an I420 frame in-place: apply background blur or replacement.
    ///
    /// Returns `true` if the frame was modified, `false` if mode is Off or
    /// the model is not loaded.
    pub fn process_i420(
        y: &mut [u8],
        u: &mut [u8],
        v: &mut [u8],
        width: usize,
        height: usize,
        stride_y: usize,
        stride_u: usize,
        stride_v: usize,
        rotation: u32,
    ) -> bool {
        // 1. Check mode
        let mode = MODE.lock().unwrap().clone();
        if mode == BackgroundMode::Off {
            return false;
        }

        // 2-4. Run segmentation: convert I420->RGB, resize to 256x256, run model, get mask
        let rgb = convert::i420_to_rgb(y, u, v, width, height, stride_y, stride_u, stride_v);
        let rgb_256 = convert::resize_rgb(&rgb, width, height, 256, 256);

        let mask_result = model::with_session(|session| {
            segment::segment(session, &rgb_256)
        });

        let mask_256 = match mask_result {
            Some(Ok(m)) => m,
            _ => return false,
        };

        // 5. Resize mask to frame dimensions
        let mask = segment::resize_mask(&mask_256, width, height);

        let uv_w = width / 2;
        let uv_h = height / 2;

        match mode {
            BackgroundMode::Blur => {
                // 6. Blur each I420 plane to get background
                let bg_y = gaussian::blur_plane(y, width, height, stride_y, Y_BLUR_RADIUS);
                let bg_u = gaussian::blur_plane(u, uv_w, uv_h, stride_u, UV_BLUR_RADIUS);
                let bg_v = gaussian::blur_plane(v, uv_w, uv_h, stride_v, UV_BLUR_RADIUS);

                // 8. Composite Y plane
                for row in 0..height {
                    for col in 0..width {
                        let m = mask[row * width + col];
                        let src_idx = row * stride_y + col;
                        let bg_idx = row * width + col;
                        y[src_idx] = lerp_u8(bg_y[bg_idx], y[src_idx], m);
                    }
                }

                // Composite U plane
                for row in 0..uv_h {
                    for col in 0..uv_w {
                        // Average mask over 2x2 luma block for chroma
                        let m = (mask[row * 2 * width + col * 2]
                            + mask[row * 2 * width + col * 2 + 1]
                            + mask[(row * 2 + 1) * width + col * 2]
                            + mask[(row * 2 + 1) * width + col * 2 + 1])
                            * 0.25;
                        let src_idx = row * stride_u + col;
                        let bg_idx = row * uv_w + col;
                        u[src_idx] = lerp_u8(bg_u[bg_idx], u[src_idx], m);
                    }
                }

                // Composite V plane
                for row in 0..uv_h {
                    for col in 0..uv_w {
                        let m = (mask[row * 2 * width + col * 2]
                            + mask[row * 2 * width + col * 2 + 1]
                            + mask[(row * 2 + 1) * width + col * 2]
                            + mask[(row * 2 + 1) * width + col * 2 + 1])
                            * 0.25;
                        let src_idx = row * stride_v + col;
                        let bg_idx = row * uv_w + col;
                        v[src_idx] = lerp_u8(bg_v[bg_idx], v[src_idx], m);
                    }
                }
            }
            BackgroundMode::Image(id) => {
                // 7. Get cached replacement I420 planes (regenerated if rotation changed)
                Self::get_replacement(id, width, height, rotation);
                let cache = REPLACEMENT_CACHE.lock().unwrap();
                let replacement = match cache.as_ref() {
                    Some(r) if r.width == width && r.height == height && r.rotation == rotation => r,
                    _ => return false,
                };

                // 8. Composite Y plane
                for row in 0..height {
                    for col in 0..width {
                        let m = mask[row * width + col];
                        let src_idx = row * stride_y + col;
                        let bg_idx = row * width + col;
                        y[src_idx] = lerp_u8(replacement.y[bg_idx], y[src_idx], m);
                    }
                }

                // Composite U plane
                for row in 0..uv_h {
                    for col in 0..uv_w {
                        let m = (mask[row * 2 * width + col * 2]
                            + mask[row * 2 * width + col * 2 + 1]
                            + mask[(row * 2 + 1) * width + col * 2]
                            + mask[(row * 2 + 1) * width + col * 2 + 1])
                            * 0.25;
                        let src_idx = row * stride_u + col;
                        let bg_idx = row * uv_w + col;
                        u[src_idx] = lerp_u8(replacement.u[bg_idx], u[src_idx], m);
                    }
                }

                // Composite V plane
                for row in 0..uv_h {
                    for col in 0..uv_w {
                        let m = (mask[row * 2 * width + col * 2]
                            + mask[row * 2 * width + col * 2 + 1]
                            + mask[(row * 2 + 1) * width + col * 2]
                            + mask[(row * 2 + 1) * width + col * 2 + 1])
                            * 0.25;
                        let src_idx = row * stride_v + col;
                        let bg_idx = row * uv_w + col;
                        v[src_idx] = lerp_u8(replacement.v[bg_idx], v[src_idx], m);
                    }
                }
            }
            BackgroundMode::Off => unreachable!(),
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_extremes() {
        assert_eq!(lerp_u8(0, 255, 1.0), 255); // full foreground
        assert_eq!(lerp_u8(0, 255, 0.0), 0); // full background
    }

    #[test]
    fn lerp_midpoint() {
        let result = lerp_u8(0, 200, 0.5);
        assert!((result as i16 - 100).abs() <= 1);
    }

    #[test]
    fn mode_default_is_off() {
        *MODE.lock().unwrap() = BackgroundMode::Off;
        assert_eq!(BlurProcessor::get_mode(), BackgroundMode::Off);
    }

    #[test]
    fn set_mode_roundtrip() {
        BlurProcessor::set_mode(BackgroundMode::Blur);
        assert_eq!(BlurProcessor::get_mode(), BackgroundMode::Blur);
        BlurProcessor::set_mode(BackgroundMode::Image(3));
        assert_eq!(BlurProcessor::get_mode(), BackgroundMode::Image(3));
        BlurProcessor::set_mode(BackgroundMode::Off);
        assert_eq!(BlurProcessor::get_mode(), BackgroundMode::Off);
    }
}
