use ort::session::Session;
use std::path::Path;
use std::sync::Mutex;

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

/// Load the selfie segmentation ONNX model from the given path.
/// Called once at app startup or first blur enable.
pub fn load_model(model_path: &Path) -> Result<(), String> {
    let mut builder = Session::builder()
        .map_err(|e| format!("ort session builder: {e}"))?
        .with_intra_threads(2)
        .map_err(|e| format!("ort threads: {e}"))?;
    let session = builder
        .commit_from_file(model_path)
        .map_err(|e| format!("ort load model: {e}"))?;
    let mut guard = SESSION.lock().map_err(|e| e.to_string())?;
    *guard = Some(session);
    Ok(())
}

/// Execute a closure with mutable access to the loaded ONNX session.
/// Returns `None` if the model has not been loaded yet.
pub fn with_session<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Session) -> R,
{
    let mut guard = SESSION.lock().ok()?;
    guard.as_mut().map(f)
}
