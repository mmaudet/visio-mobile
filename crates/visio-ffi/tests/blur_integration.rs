//! Integration test: load the ONNX model and run segmentation on a synthetic frame.
//! Requires the model file at `models/selfie_segmentation.onnx` (project root).

use std::path::PathBuf;

fn project_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/
    p.pop(); // project root
    p
}

fn model_path() -> PathBuf {
    project_root().join("models/selfie_segmentation.onnx")
}

#[test]
fn test_model_loads_successfully() {
    let path = model_path();
    if !path.exists() {
        eprintln!("Skipping: model not found at {}", path.display());
        return;
    }
    let result = visio_ffi::blur::model::load_model(&path);
    assert!(result.is_ok(), "Model failed to load: {:?}", result.err());
}

#[test]
fn test_blur_mode_on_synthetic_frame() {
    let path = model_path();
    if !path.exists() {
        eprintln!("Skipping: model not found at {}", path.display());
        return;
    }
    let _ = visio_ffi::blur::model::load_model(&path);

    visio_ffi::blur::BlurProcessor::set_mode(visio_ffi::blur::process::BackgroundMode::Blur);

    let width = 640usize;
    let height = 480usize;
    let uv_w = width / 2;
    let uv_h = height / 2;

    let mut y = vec![128u8; width * height];
    let mut u = vec![128u8; uv_w * uv_h];
    let mut v = vec![128u8; uv_w * uv_h];

    let result = visio_ffi::blur::BlurProcessor::process_i420(
        &mut y, &mut u, &mut v,
        width, height,
        width, uv_w, uv_w,
        0,
    );

    assert!(result, "Blur mode should process successfully");
    println!("Blur mode: pipeline OK on {}x{}", width, height);

    visio_ffi::blur::BlurProcessor::set_mode(visio_ffi::blur::process::BackgroundMode::Off);
}

#[test]
fn test_image_replacement_mode() {
    let path = model_path();
    if !path.exists() {
        eprintln!("Skipping: model not found at {}", path.display());
        return;
    }
    let _ = visio_ffi::blur::model::load_model(&path);

    // Load a background image
    let bg_path = project_root().join("assets/backgrounds/1.jpg");
    if !bg_path.exists() {
        eprintln!("Skipping: background image not found at {}", bg_path.display());
        return;
    }
    let jpeg_bytes = std::fs::read(&bg_path).unwrap();

    let width = 640usize;
    let height = 480usize;

    // Load replacement image
    let load_result = visio_ffi::blur::BlurProcessor::load_replacement_image(1, &jpeg_bytes, width, height);
    assert!(load_result.is_ok(), "Image load failed: {:?}", load_result.err());

    // Set image mode
    visio_ffi::blur::BlurProcessor::set_mode(visio_ffi::blur::process::BackgroundMode::Image(1));

    let uv_w = width / 2;
    let uv_h = height / 2;
    let mut y = vec![128u8; width * height];
    let mut u = vec![128u8; uv_w * uv_h];
    let mut v = vec![128u8; uv_w * uv_h];

    let result = visio_ffi::blur::BlurProcessor::process_i420(
        &mut y, &mut u, &mut v,
        width, height,
        width, uv_w, uv_w,
        0,
    );

    assert!(result, "Image replacement mode should process successfully");
    println!("Image replacement mode: pipeline OK on {}x{}", width, height);

    visio_ffi::blur::BlurProcessor::set_mode(visio_ffi::blur::process::BackgroundMode::Off);
}
