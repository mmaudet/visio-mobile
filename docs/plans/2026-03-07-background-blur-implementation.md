# Background Blur Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add real-time on-device background blur/replacement to the camera pipeline on all 3 platforms.

**Architecture:** A single Rust module in visio-ffi handles person segmentation (via ONNX Runtime + MediaPipe selfie segmentation model) and Gaussian blur compositing on I420 frames. The module intercepts frames after I420 conversion and before `capture_frame()` on all platforms. Settings are stored in visio-core. UI toggle is added to each platform's in-call settings.

**Tech Stack:** Rust, ort (ONNX Runtime crate), MediaPipe selfie segmentation model (ONNX format, ~200KB), I420 pixel manipulation

---

## Task 1: Add ONNX Runtime dependency and model management

**Files:**
- Modify: `crates/visio-ffi/Cargo.toml`
- Create: `crates/visio-ffi/src/blur/mod.rs`
- Create: `crates/visio-ffi/src/blur/model.rs`
- Create: `models/selfie_segmentation.onnx` (downloaded separately)

**Step 1: Add `ort` dependency to visio-ffi**

In `crates/visio-ffi/Cargo.toml`, add:
```toml
[dependencies]
ort = { version = "2", default-features = false, features = ["ndarray"] }
ndarray = "0.16"
```

**Step 2: Create blur module skeleton**

Create `crates/visio-ffi/src/blur/mod.rs`:
```rust
pub mod model;
mod process;

pub use process::BlurProcessor;
```

Create `crates/visio-ffi/src/blur/model.rs`:
```rust
use ort::session::Session;
use std::path::Path;
use std::sync::OnceLock;

static SESSION: OnceLock<Session> = OnceLock::new();

/// Load the selfie segmentation ONNX model from the given path.
/// Called once at app startup or first blur enable.
pub fn load_model(model_path: &Path) -> Result<(), String> {
    let session = Session::builder()
        .map_err(|e| format!("ort session builder: {e}"))?
        .with_intra_threads(2)
        .map_err(|e| format!("ort threads: {e}"))?
        .commit_from_file(model_path)
        .map_err(|e| format!("ort load model: {e}"))?;
    SESSION.set(session).map_err(|_| "model already loaded".into())
}

pub fn get_session() -> Option<&'static Session> {
    SESSION.get()
}
```

**Step 3: Download MediaPipe selfie segmentation model**

The ONNX model will be bundled with each platform's assets. Download from MediaPipe model zoo and convert to ONNX format. Place at `models/selfie_segmentation.onnx` for reference.

Note: The model takes 256x256 RGB input and outputs a 256x256 single-channel mask (0.0 = background, 1.0 = person).

**Step 4: Verify it compiles**

Run: `cargo build -p visio-ffi 2>&1 | tail -10`
Expected: Compiles (model not loaded yet, just structure)

**Step 5: Commit**

```
feat(ffi): add ONNX Runtime dependency and blur module skeleton
```

---

## Task 2: Implement I420 ↔ RGB conversion utilities

**Files:**
- Create: `crates/visio-ffi/src/blur/convert.rs`

**Step 1: Write I420-to-RGB and RGB-to-I420 conversion functions**

The segmentation model needs RGB input. Camera frames are I420. We need bidirectional conversion.

```rust
/// Convert I420 planes to packed RGB (BT.601 full-range).
/// Output: Vec<u8> of length width * height * 3.
pub fn i420_to_rgb(
    y: &[u8], u: &[u8], v: &[u8],
    width: usize, height: usize,
    stride_y: usize, stride_u: usize, stride_v: usize,
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
    src: &[u8], src_w: usize, src_h: usize,
    dst_w: usize, dst_h: usize,
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
```

**Step 2: Add unit tests for conversions**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i420_to_rgb_black_frame() {
        // Y=0, U=128, V=128 → RGB(0, 0, 0)
        let y = vec![0u8; 4];
        let u = vec![128u8; 1];
        let v = vec![128u8; 1];
        let rgb = i420_to_rgb(&y, &u, &v, 2, 2, 2, 1, 1);
        assert!(rgb.iter().all(|&b| b == 0));
    }

    #[test]
    fn i420_to_rgb_white_frame() {
        // Y=255, U=128, V=128 → RGB(255, 255, 255)
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
}
```

**Step 3: Run tests**

Run: `cargo test -p visio-ffi --lib blur 2>&1 | tail -15`
Expected: All tests pass

**Step 4: Commit**

```
feat(ffi): add I420-RGB conversion utilities with tests
```

---

## Task 3: Implement segmentation inference

**Files:**
- Create: `crates/visio-ffi/src/blur/segment.rs`

**Step 1: Implement the segmentation function**

```rust
use ndarray::{Array, CowArray, IxDyn};
use ort::session::Session;

/// Run selfie segmentation on an RGB image.
/// Input: 256x256 RGB image (packed u8).
/// Output: 256x256 mask (f32, 0.0=background, 1.0=person).
pub fn segment(session: &Session, rgb_256: &[u8]) -> Result<Vec<f32>, String> {
    assert_eq!(rgb_256.len(), 256 * 256 * 3);

    // Normalize to [0, 1] and reshape to NCHW: [1, 3, 256, 256]
    let mut input = vec![0.0f32; 1 * 3 * 256 * 256];
    for i in 0..(256 * 256) {
        input[i] = rgb_256[i * 3] as f32 / 255.0;              // R
        input[256 * 256 + i] = rgb_256[i * 3 + 1] as f32 / 255.0; // G
        input[2 * 256 * 256 + i] = rgb_256[i * 3 + 2] as f32 / 255.0; // B
    }

    let input_array = CowArray::from(
        Array::from_shape_vec(IxDyn(&[1, 3, 256, 256]), input)
            .map_err(|e| format!("input shape: {e}"))?
    );

    let outputs = session
        .run(ort::inputs![input_array].map_err(|e| format!("ort inputs: {e}"))?)
        .map_err(|e| format!("ort run: {e}"))?;

    let output_tensor = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("ort extract: {e}"))?;

    // Output is [1, 1, 256, 256] or [1, 256, 256] — flatten to 256*256
    let mask: Vec<f32> = output_tensor.iter().copied().collect();
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
```

**Step 2: Commit**

```
feat(ffi): add ONNX-based selfie segmentation inference
```

---

## Task 4: Implement Gaussian blur on I420

**Files:**
- Create: `crates/visio-ffi/src/blur/gaussian.rs`

**Step 1: Implement box blur approximation on Y/U/V planes**

A 3-pass box blur approximates Gaussian blur and is much faster (O(n) per pass, independent of radius).

```rust
/// Apply 3-pass box blur approximation of Gaussian blur on a single plane.
/// `data`: pixel values, `width`/`height`: plane dimensions, `stride`: row stride.
/// `radius`: blur radius in pixels.
/// Returns a new buffer with the blurred plane.
pub fn blur_plane(
    data: &[u8], width: usize, height: usize, stride: usize, radius: usize,
) -> Vec<u8> {
    let mut src = extract_plane(data, width, height, stride);
    let mut dst = vec![0u8; width * height];
    // 3-pass box blur
    for _ in 0..3 {
        box_blur_h(&src, &mut dst, width, height, radius);
        box_blur_v(&dst, &mut src, width, height, radius);
    }
    src
}

fn extract_plane(data: &[u8], width: usize, height: usize, stride: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height];
    for row in 0..height {
        out[row * width..(row + 1) * width]
            .copy_from_slice(&data[row * stride..row * stride + width]);
    }
    out
}

fn box_blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let diameter = 2 * r + 1;
    for y in 0..h {
        let mut sum = 0u32;
        // Initialize window
        for x in 0..=r.min(w - 1) {
            sum += src[y * w + x] as u32;
        }
        // Left edge padding
        sum += r.saturating_sub(0) as u32 * src[y * w] as u32;

        for x in 0..w {
            dst[y * w + x] = (sum / diameter as u32).min(255) as u8;
            let right = (x + r + 1).min(w - 1);
            let left = (x as isize - r as isize).max(0) as usize;
            sum += src[y * w + right] as u32;
            sum -= src[y * w + left] as u32;
        }
    }
}

fn box_blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let diameter = 2 * r + 1;
    for x in 0..w {
        let mut sum = 0u32;
        for y in 0..=r.min(h - 1) {
            sum += src[y * w + x] as u32;
        }
        sum += r.saturating_sub(0) as u32 * src[x] as u32;

        for y in 0..h {
            dst[y * w + x] = (sum / diameter as u32).min(255) as u8;
            let bottom = (y + r + 1).min(h - 1);
            let top = (y as isize - r as isize).max(0) as usize;
            sum += src[bottom * w + x] as u32;
            sum -= src[top * w + x] as u32;
        }
    }
}
```

**Step 2: Add tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_uniform_plane_unchanged() {
        let data = vec![128u8; 10 * 10];
        let result = blur_plane(&data, 10, 10, 10, 3);
        // Uniform input → uniform output
        for &v in &result {
            assert!((v as i16 - 128).abs() <= 1);
        }
    }

    #[test]
    fn blur_reduces_contrast() {
        let mut data = vec![0u8; 10 * 10];
        // White center pixel
        data[5 * 10 + 5] = 255;
        let result = blur_plane(&data, 10, 10, 10, 2);
        // Center should be dimmer, neighbors should be brighter
        assert!(result[5 * 10 + 5] < 255);
        assert!(result[5 * 10 + 4] > 0);
    }
}
```

**Step 3: Run tests**

Run: `cargo test -p visio-ffi --lib blur 2>&1 | tail -15`

**Step 4: Commit**

```
feat(ffi): add fast box blur approximation for I420 planes
```

---

## Task 5: Implement the BlurProcessor (compositing)

**Files:**
- Create: `crates/visio-ffi/src/blur/process.rs`

**Step 1: Implement the main processor that ties everything together**

```rust
use super::{convert, gaussian, model, segment};
use std::sync::atomic::{AtomicBool, Ordering};

static BLUR_ENABLED: AtomicBool = AtomicBool::new(false);

/// Blur radius for background (applied to each I420 plane scaled appropriately).
const Y_BLUR_RADIUS: usize = 15;
const UV_BLUR_RADIUS: usize = 7; // Half resolution

pub struct BlurProcessor;

impl BlurProcessor {
    pub fn set_enabled(enabled: bool) {
        BLUR_ENABLED.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled() -> bool {
        BLUR_ENABLED.load(Ordering::Relaxed)
    }

    /// Process an I420 frame: segment person, blur background, composite.
    /// Modifies the planes in-place.
    /// Returns false if blur could not be applied (model not loaded, etc.).
    pub fn process_i420(
        y: &mut [u8], u: &mut [u8], v: &mut [u8],
        width: usize, height: usize,
        stride_y: usize, stride_u: usize, stride_v: usize,
    ) -> bool {
        if !Self::is_enabled() {
            return false;
        }

        let session = match model::get_session() {
            Some(s) => s,
            None => return false,
        };

        // 1. Convert I420 to RGB
        let rgb = convert::i420_to_rgb(y, u, v, width, height, stride_y, stride_u, stride_v);

        // 2. Resize to 256x256 for model
        let rgb_256 = convert::resize_rgb(&rgb, width, height, 256, 256);

        // 3. Run segmentation
        let mask_256 = match segment::segment(session, &rgb_256) {
            Ok(m) => m,
            Err(_) => return false,
        };

        // 4. Resize mask back to frame dimensions
        let mask = segment::resize_mask(&mask_256, width, height);

        // 5. Blur each plane
        let y_blurred = gaussian::blur_plane(y, width, height, stride_y, Y_BLUR_RADIUS);
        let uv_w = width / 2;
        let uv_h = height / 2;
        let u_blurred = gaussian::blur_plane(u, uv_w, uv_h, stride_u, UV_BLUR_RADIUS);
        let v_blurred = gaussian::blur_plane(v, uv_w, uv_h, stride_v, UV_BLUR_RADIUS);

        // 6. Composite: foreground (original) where mask > 0.5, background (blurred) elsewhere
        for row in 0..height {
            for col in 0..width {
                let m = mask[row * width + col];
                let idx = row * stride_y + col;
                y[idx] = lerp_u8(y_blurred[row * width + col], y[idx], m);
            }
        }
        for row in 0..uv_h {
            for col in 0..uv_w {
                // Average mask over the 2x2 block this chroma pixel covers
                let m = (mask[row * 2 * width + col * 2]
                    + mask[row * 2 * width + col * 2 + 1]
                    + mask[(row * 2 + 1) * width + col * 2]
                    + mask[(row * 2 + 1) * width + col * 2 + 1])
                    / 4.0;
                let idx_u = row * stride_u + col;
                let idx_v = row * stride_v + col;
                u[idx_u] = lerp_u8(u_blurred[row * uv_w + col], u[idx_u], m);
                v[idx_v] = lerp_u8(v_blurred[row * uv_w + col], v[idx_v], m);
            }
        }

        true
    }
}

#[inline]
fn lerp_u8(bg: u8, fg: u8, mask: f32) -> u8 {
    let m = mask.clamp(0.0, 1.0);
    (bg as f32 * (1.0 - m) + fg as f32 * m) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_extremes() {
        assert_eq!(lerp_u8(0, 255, 1.0), 255); // full foreground
        assert_eq!(lerp_u8(0, 255, 0.0), 0);   // full background
    }

    #[test]
    fn lerp_midpoint() {
        let result = lerp_u8(0, 200, 0.5);
        assert!((result as i16 - 100).abs() <= 1);
    }
}
```

**Step 2: Run tests**

Run: `cargo test -p visio-ffi --lib blur 2>&1 | tail -15`

**Step 3: Commit**

```
feat(ffi): add BlurProcessor compositing pipeline
```

---

## Task 6: Add blur setting to visio-core

**Files:**
- Modify: `crates/visio-core/src/settings.rs`
- Modify: `crates/visio-core/src/controls.rs`

**Step 1: Add `blur_enabled` to Settings struct**

In `settings.rs`, add a `blur_enabled: bool` field to the settings struct with default `false`. Add `set_blur_enabled()` and `is_blur_enabled()` methods following the existing pattern (e.g., `set_mic_enabled_on_join`).

**Step 2: Add blur control to MeetingControls**

In `controls.rs`, add `set_blur_enabled(enabled: bool)` and `is_blur_enabled() -> bool` methods. These should call `BlurProcessor::set_enabled()` when toggled.

**Step 3: Add tests**

Follow the existing pattern in `settings::tests`:
```rust
#[test]
fn test_set_blur_enabled_persists() {
    // ...similar to existing settings tests
}
```

**Step 4: Run tests**

Run: `cargo test -p visio-core --lib 2>&1 | tail -10`

**Step 5: Commit**

```
feat(core): add blur_enabled setting and control
```

---

## Task 7: Expose blur via UniFFI and hook into camera pipeline

**Files:**
- Modify: `crates/visio-ffi/src/lib.rs`

**Step 1: Add UniFFI methods**

Add `set_blur_enabled(enabled: bool)` and `is_blur_enabled() -> bool` to the VisioClient FFI interface, delegating to `MeetingControls`.

**Step 2: Hook into Android camera path**

In `Java_io_visio_mobile_NativeVideo_nativePushCameraFrame()` (around line 890, after I420 construction, before `capture_frame()`):

```rust
// Apply background blur if enabled
blur::BlurProcessor::process_i420(
    &mut y_data, &mut u_data, &mut v_data,
    width as usize, height as usize,
    stride_y as usize, stride_u as usize, stride_v as usize,
);
// Then create VideoFrame from (possibly modified) data and call capture_frame
```

**Step 3: Hook into iOS camera path**

In `visio_push_ios_camera_frame()` (around line 1145, same pattern).

**Step 4: Verify it compiles**

Run: `cargo build -p visio-ffi 2>&1 | tail -10`

**Step 5: Commit**

```
feat(ffi): expose blur setting and hook into camera pipeline
```

---

## Task 8: Hook into Desktop camera path

**Files:**
- Modify: `crates/visio-desktop/src/camera_macos.rs`
- Modify: `crates/visio-desktop/src/lib.rs`

**Step 1: Add blur processing to desktop camera capture**

In `camera_macos.rs`, after I420 buffer construction (around line 145), call `BlurProcessor::process_i420()`.

**Step 2: Add Tauri command for blur toggle**

In `lib.rs`, add `toggle_blur` command:
```rust
#[tauri::command]
fn toggle_blur(state: State<'_, VisioState>, enabled: bool) -> Result<(), String> {
    // ...
}
```

**Step 3: Commit**

```
feat(desktop): integrate background blur into camera pipeline
```

---

## Task 9: Android UI — blur toggle

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/InCallSettingsSheet.kt`
- Modify: `i18n/en.json`
- Modify: `i18n/fr.json`

**Step 1: Add blur toggle to camera tab**

In the camera tab of InCallSettingsSheet, add a Switch toggle:
```kotlin
Row(
    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
    horizontalArrangement = Arrangement.SpaceBetween,
    verticalAlignment = Alignment.CenterVertically,
) {
    Text(Strings.t("settings.incall.blur", lang))
    Switch(
        checked = blurEnabled,
        onCheckedChange = { enabled ->
            blurEnabled = enabled
            coroutineScope.launch(Dispatchers.IO) {
                VisioManager.client.setBlurEnabled(enabled)
            }
        },
    )
}
```

**Step 2: Add i18n keys**

en.json: `"settings.incall.blur": "Background blur"`
fr.json: `"settings.incall.blur": "Flou d'arriere-plan"`

**Step 3: Build and verify**

Run: `cd android && ./gradlew compileDebugKotlin 2>&1 | tail -10`

**Step 4: Commit**

```
feat(android): add background blur toggle in in-call settings
```

---

## Task 10: iOS UI — blur toggle

**Files:**
- Modify: `ios/VisioMobile/Views/InCallSettingsSheet.swift`

**Step 1: Add blur toggle to camera tab**

In the camera section of InCallSettingsSheet, add a Toggle:
```swift
Toggle(Strings.t("settings.incall.blur", lang: lang), isOn: Binding(
    get: { manager.isBlurEnabled },
    set: { enabled in
        DispatchQueue.global(qos: .userInitiated).async {
            manager.client.setBlurEnabled(enabled: enabled)
            DispatchQueue.main.async {
                manager.isBlurEnabled = enabled
            }
        }
    }
))
```

**Step 2: Add `isBlurEnabled` published property to VisioManager**

In `VisioManager.swift`, add `@Published var isBlurEnabled: Bool = false`.

**Step 3: Commit**

```
feat(ios): add background blur toggle in in-call settings
```

---

## Task 11: Desktop UI — blur toggle

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Add blur toggle to settings/camera section**

Add a toggle switch in the camera settings area:
```tsx
<label className="flex items-center gap-2">
  <input
    type="checkbox"
    checked={blurEnabled}
    onChange={async (e) => {
      const enabled = e.target.checked;
      setBlurEnabled(enabled);
      await invoke("toggle_blur", { enabled });
    }}
  />
  {t("settings.incall.blur")}
</label>
```

**Step 2: Commit**

```
feat(desktop): add background blur toggle in settings
```

---

## Task 12: Model bundling and first-run download

**Files:**
- Create: `scripts/download-models.sh`
- Modify: Android `build.gradle` (copy model to assets)
- Modify: iOS Xcode project (add model to bundle)

**Step 1: Create download script**

```bash
#!/bin/bash
# Download selfie segmentation ONNX model
MODEL_DIR="models"
mkdir -p "$MODEL_DIR"
# URL TBD — MediaPipe model zoo or self-hosted
curl -L -o "$MODEL_DIR/selfie_segmentation.onnx" "$MODEL_URL"
```

**Step 2: Platform-specific model bundling**

- Android: copy `models/selfie_segmentation.onnx` to `android/app/src/main/assets/models/`
- iOS: add to Xcode project as a bundle resource
- Desktop: include in Tauri resources

**Step 3: Load model on first blur enable**

In each platform, when blur is first toggled on, call `model::load_model(path)` with the correct asset path.

**Step 4: Commit**

```
feat: add model bundling and loading for background blur
```

---

## Summary

| Task | Component | Est. complexity |
|------|-----------|-----------------|
| 1 | ONNX Runtime + module skeleton | Low |
| 2 | I420 ↔ RGB conversion | Medium |
| 3 | Segmentation inference | Medium |
| 4 | Gaussian blur (box blur approx) | Medium |
| 5 | BlurProcessor compositing | Medium |
| 6 | Settings in visio-core | Low |
| 7 | FFI exposure + camera hooks (Android/iOS) | High |
| 8 | Desktop camera hook + Tauri command | Medium |
| 9 | Android UI toggle | Low |
| 10 | iOS UI toggle | Low |
| 11 | Desktop UI toggle | Low |
| 12 | Model bundling | Medium |

**Total: 12 tasks. Branch: `feat/background-blur`. PR when all tasks pass.**
