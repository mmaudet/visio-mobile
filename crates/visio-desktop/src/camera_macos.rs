//! macOS camera capture using AVFoundation.
//!
//! Opens the default camera, forces NV12 pixel format, converts
//! NV12 → I420, and feeds frames into a LiveKit NativeVideoSource.
//! Also emits self-view frames through the visio-video desktop callback.

use std::ffi::{c_char, c_void};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use livekit::webrtc::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use serde::Serialize;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, NSObject};
use objc2::{ClassType, define_class, msg_send};

// ---------------------------------------------------------------------------
// Video device enumeration
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct VideoDeviceInfo {
    pub name: String,
    pub unique_id: String,
    pub is_default: bool,
}

/// List available video capture devices via AVFoundation.
pub fn list_cameras() -> Vec<VideoDeviceInfo> {
    unsafe { list_cameras_avfoundation() }
}

unsafe fn list_cameras_avfoundation() -> Vec<VideoDeviceInfo> {
    let device_cls = match AnyClass::get(c"AVCaptureDevice") {
        Some(cls) => cls,
        None => return Vec::new(),
    };
    let nsstring_cls = match AnyClass::get(c"NSString") {
        Some(cls) => cls,
        None => return Vec::new(),
    };
    let nsarray_cls = match AnyClass::get(c"NSArray") {
        Some(cls) => cls,
        None => return Vec::new(),
    };
    let discovery_cls = match AnyClass::get(c"AVCaptureDeviceDiscoverySession") {
        Some(cls) => cls,
        None => return Vec::new(),
    };

    // Get default device for comparison
    let default_ptr: *mut AnyObject =
        unsafe { msg_send![device_cls, defaultDeviceWithMediaType: AVMediaTypeVideo] };
    let default_uid: Option<String> = if !default_ptr.is_null() {
        let uid_ns: *mut AnyObject = unsafe { msg_send![default_ptr, uniqueID] };
        if !uid_ns.is_null() {
            let cstr: *const c_char = unsafe { msg_send![uid_ns, UTF8String] };
            if !cstr.is_null() {
                Some(
                    unsafe { std::ffi::CStr::from_ptr(cstr) }
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Create device type strings
    let builtin_type: Retained<AnyObject> = unsafe {
        msg_send![nsstring_cls, stringWithUTF8String: c"AVCaptureDeviceTypeBuiltInWideAngleCamera".as_ptr()]
    };
    let external_type: Retained<AnyObject> = unsafe {
        msg_send![nsstring_cls, stringWithUTF8String: c"AVCaptureDeviceTypeExternal".as_ptr()]
    };

    // Create NSArray with both device types
    let types_array: Retained<AnyObject> = unsafe {
        let objects: [*const AnyObject; 2] = [&*builtin_type, &*external_type];
        msg_send![nsarray_cls, arrayWithObjects: objects.as_ptr(), count: 2usize]
    };

    // Create discovery session
    // position: 0 = AVCaptureDevicePositionUnspecified
    let session: *mut AnyObject = unsafe {
        msg_send![
            discovery_cls,
            discoverySessionWithDeviceTypes: &*types_array,
            mediaType: AVMediaTypeVideo,
            position: 0i64
        ]
    };
    if session.is_null() {
        return Vec::new();
    }

    // Get devices array
    let devices: *mut AnyObject = unsafe { msg_send![session, devices] };
    if devices.is_null() {
        return Vec::new();
    }

    let count: usize = unsafe { msg_send![devices, count] };
    let mut result = Vec::with_capacity(count);

    for i in 0..count {
        let device: *mut AnyObject = unsafe { msg_send![devices, objectAtIndex: i] };
        if device.is_null() {
            continue;
        }

        // Get localizedName
        let name_ns: *mut AnyObject = unsafe { msg_send![device, localizedName] };
        let name = if !name_ns.is_null() {
            let cstr: *const c_char = unsafe { msg_send![name_ns, UTF8String] };
            if !cstr.is_null() {
                unsafe { std::ffi::CStr::from_ptr(cstr) }
                    .to_string_lossy()
                    .into_owned()
            } else {
                String::from("Unknown")
            }
        } else {
            String::from("Unknown")
        };

        // Get uniqueID
        let uid_ns: *mut AnyObject = unsafe { msg_send![device, uniqueID] };
        let unique_id = if !uid_ns.is_null() {
            let cstr: *const c_char = unsafe { msg_send![uid_ns, UTF8String] };
            if !cstr.is_null() {
                unsafe { std::ffi::CStr::from_ptr(cstr) }
                    .to_string_lossy()
                    .into_owned()
            } else {
                continue;
            }
        } else {
            continue;
        };

        let is_default = default_uid.as_deref() == Some(&unique_id);

        result.push(VideoDeviceInfo {
            name,
            unique_id,
            is_default,
        });
    }

    result
}

// ---------------------------------------------------------------------------
// CoreMedia / CoreVideo C FFI
// ---------------------------------------------------------------------------

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    fn CMSampleBufferGetImageBuffer(sbuf: *const c_void) -> *const c_void;
}

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVPixelBufferLockBaseAddress(pxbuf: *const c_void, flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pxbuf: *const c_void, flags: u64) -> i32;
    fn CVPixelBufferGetBaseAddressOfPlane(pxbuf: *const c_void, plane: usize) -> *const u8;
    fn CVPixelBufferGetBytesPerRowOfPlane(pxbuf: *const c_void, plane: usize) -> usize;
    fn CVPixelBufferGetWidth(pxbuf: *const c_void) -> usize;
    fn CVPixelBufferGetHeight(pxbuf: *const c_void) -> usize;
}

// ---------------------------------------------------------------------------
// AVFoundation constants
// ---------------------------------------------------------------------------

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVMediaTypeVideo: *const AnyObject;
    static AVCaptureSessionPreset1280x720: *const AnyObject;
}

/// NV12 full-range: kCVPixelFormatType_420YpCbCr8BiPlanarFullRange = '420f'
const PIXEL_FORMAT_NV12: u32 = 0x34323066;

// ---------------------------------------------------------------------------
// libdispatch
// ---------------------------------------------------------------------------

unsafe extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
    fn dispatch_release(queue: *mut c_void);
}

// ---------------------------------------------------------------------------
// Shared camera state
// ---------------------------------------------------------------------------

struct CameraState {
    video_source: NativeVideoSource,
    frame_count: AtomicU64,
}

static CAMERA_STATE: Mutex<Option<CameraState>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Frame processing
// ---------------------------------------------------------------------------

/// Called from the delegate callback on the dispatch queue thread.
fn process_camera_frame(sample_buffer: *const c_void) {
    if sample_buffer.is_null() {
        return;
    }

    let state_guard = CAMERA_STATE.lock().unwrap();
    let Some(state) = state_guard.as_ref() else {
        return;
    };

    let count = state.frame_count.fetch_add(1, Ordering::Relaxed);

    // Get CVPixelBuffer from CMSampleBuffer
    let pxbuf = unsafe { CMSampleBufferGetImageBuffer(sample_buffer) };
    if pxbuf.is_null() {
        return;
    }

    // Lock pixel buffer for read access (1 = kCVPixelBufferLock_ReadOnly)
    let status = unsafe { CVPixelBufferLockBaseAddress(pxbuf, 1) };
    if status != 0 {
        tracing::warn!("CVPixelBufferLockBaseAddress failed: {status}");
        return;
    }

    let width = unsafe { CVPixelBufferGetWidth(pxbuf) } as u32;
    let height = unsafe { CVPixelBufferGetHeight(pxbuf) } as u32;

    // NV12: plane 0 = Y (full res), plane 1 = UV interleaved (half res)
    let y_ptr = unsafe { CVPixelBufferGetBaseAddressOfPlane(pxbuf, 0) };
    let y_stride = unsafe { CVPixelBufferGetBytesPerRowOfPlane(pxbuf, 0) };
    let uv_ptr = unsafe { CVPixelBufferGetBaseAddressOfPlane(pxbuf, 1) };
    let uv_stride = unsafe { CVPixelBufferGetBytesPerRowOfPlane(pxbuf, 1) };

    if y_ptr.is_null() || uv_ptr.is_null() {
        unsafe { CVPixelBufferUnlockBaseAddress(pxbuf, 1) };
        return;
    }

    let h = height as usize;
    let w = width as usize;

    // Build I420 buffer from NV12 planes
    let mut i420 = I420Buffer::new(width, height);

    let strides = i420.strides();
    let (y_dst, u_dst, v_dst) = i420.data_mut();

    // Copy Y plane
    for row in 0..h {
        let src_start = row * y_stride;
        let dst_start = row * strides.0 as usize;
        let src_slice = unsafe { std::slice::from_raw_parts(y_ptr.add(src_start), w) };
        y_dst[dst_start..dst_start + w].copy_from_slice(src_slice);
    }

    // Deinterleave UV plane into U and V
    let chroma_h = h / 2;
    let chroma_w = w / 2;
    for row in 0..chroma_h {
        let src_row =
            unsafe { std::slice::from_raw_parts(uv_ptr.add(row * uv_stride), chroma_w * 2) };
        let dst_row_offset = row * strides.1 as usize;
        for col in 0..chroma_w {
            u_dst[dst_row_offset + col] = src_row[col * 2];
            v_dst[dst_row_offset + col] = src_row[col * 2 + 1];
        }
    }

    // Release mutable borrows before immutable use
    let _ = y_dst;
    let _ = u_dst;
    let _ = v_dst;

    unsafe { CVPixelBufferUnlockBaseAddress(pxbuf, 1) };

    // Apply background processing (blur/replacement) if enabled
    {
        let (y_data, u_data, v_data) = i420.data_mut();
        visio_ffi::blur::BlurProcessor::process_i420(
            y_data,
            u_data,
            v_data,
            w,
            h,
            strides.0 as usize,
            strides.1 as usize,
            strides.2 as usize,
            0, // Desktop camera frames have no rotation metadata
        );
    }

    // Feed frame into LiveKit
    let frame = VideoFrame {
        rotation: VideoRotation::VideoRotation0,
        timestamp_us: 0,
        buffer: i420,
    };
    state.video_source.capture_frame(&frame);

    // Self-view: render every 3rd frame (~10 fps) through desktop callback
    if count % 3 == 0 {
        visio_video::render_local_i420(&frame.buffer, "local-camera");
    }
}

// ---------------------------------------------------------------------------
// ObjC delegate class
// ---------------------------------------------------------------------------

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "VisioCameraDelegate"]
    struct VisioCameraDelegate;

    impl VisioCameraDelegate {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        #[allow(non_snake_case)]
        fn captureOutput_didOutputSampleBuffer_fromConnection(
            &self,
            _output: *const AnyObject,
            sample_buffer: *const c_void,
            _connection: *const AnyObject,
        ) {
            process_camera_frame(sample_buffer);
        }
    }
);

// ---------------------------------------------------------------------------
// Permission helpers
// ---------------------------------------------------------------------------

/// Request camera permission synchronously (blocks until user responds).
/// Returns true if permission was granted.
pub fn request_camera_permission() -> bool {
    use std::sync::mpsc;
    unsafe {
        let cls = match AnyClass::get(c"AVCaptureDevice") {
            Some(c) => c,
            None => return false,
        };
        // Check current status first: 0=notDetermined, 3=authorized
        let status: i64 = msg_send![cls, authorizationStatusForMediaType: AVMediaTypeVideo];
        if status == 3 {
            return true;
        }
        if status == 2 || status == 1 {
            // denied or restricted
            return false;
        }
        // notDetermined — request access
        let (tx, rx) = mpsc::channel();
        let block = block2::StackBlock::new(move |granted: Bool| {
            let _ = tx.send(granted.as_bool());
        });
        let _: () = msg_send![cls, requestAccessForMediaType: AVMediaTypeVideo, completionHandler: &*block];
        rx.recv().unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Manages an AVCaptureSession for camera capture on macOS.
pub struct MacCameraCapture {
    session: Retained<AnyObject>,
    // CRITICAL: delegate must be retained here. AVCaptureVideoDataOutput
    // holds a weak reference — if we drop the delegate, callbacks stop.
    _delegate: Retained<VisioCameraDelegate>,
    queue: *mut c_void,
}

// The session and delegate are ObjC objects managed by the runtime.
// We only touch them from the main thread (start/stop) and the dispatch
// queue thread (delegate callback). This is safe.
unsafe impl Send for MacCameraCapture {}

impl MacCameraCapture {
    /// Start capturing from the default camera and feeding frames into `source`.
    pub fn start(source: NativeVideoSource) -> Result<Self, String> {
        // Store the source in global state for the delegate callback
        {
            let mut state = CAMERA_STATE.lock().unwrap();
            *state = Some(CameraState {
                video_source: source,
                frame_count: AtomicU64::new(0),
            });
        }

        unsafe { Self::start_avfoundation() }
    }

    /// Start capturing from a camera identified by its unique ID.
    pub fn start_with_unique_id(
        unique_id: &str,
        source: NativeVideoSource,
    ) -> Result<Self, String> {
        {
            let mut state = CAMERA_STATE.lock().unwrap();
            *state = Some(CameraState {
                video_source: source,
                frame_count: AtomicU64::new(0),
            });
        }
        unsafe { Self::start_avfoundation_with_uid(unique_id) }
    }

    unsafe fn start_avfoundation_with_uid(unique_id: &str) -> Result<Self, String> {
        // --- Create session ---
        let session_cls =
            AnyClass::get(c"AVCaptureSession").ok_or("AVCaptureSession class not found")?;
        let session: Retained<AnyObject> = unsafe { msg_send![session_cls, new] };

        // Set session preset
        let _: () = unsafe { msg_send![&*session, setSessionPreset: AVCaptureSessionPreset1280x720] };

        // --- Find camera device by uniqueID ---
        let device_cls =
            AnyClass::get(c"AVCaptureDevice").ok_or("AVCaptureDevice class not found")?;
        let ns_cls = AnyClass::get(c"NSString").unwrap();
        let uid_ns: Retained<AnyObject> = unsafe {
            msg_send![
                ns_cls,
                stringWithUTF8String: std::ffi::CString::new(unique_id)
                    .map_err(|_| "invalid unique_id")?.as_ptr()
            ]
        };
        let device_ptr: *mut AnyObject =
            unsafe { msg_send![device_cls, deviceWithUniqueID: &*uid_ns] };
        if device_ptr.is_null() {
            return Err(format!(
                "No camera device found with unique_id: {unique_id}"
            ));
        }
        let device =
            unsafe { Retained::retain(device_ptr) }.ok_or("Failed to retain camera device")?;

        // --- Create device input ---
        let input_cls =
            AnyClass::get(c"AVCaptureDeviceInput").ok_or("AVCaptureDeviceInput class not found")?;
        let mut error_ptr: *mut AnyObject = std::ptr::null_mut();
        let input_ptr: *mut AnyObject =
            unsafe { msg_send![input_cls, deviceInputWithDevice: &*device, error: &mut error_ptr] };
        if input_ptr.is_null() {
            return Err("Failed to create camera input".into());
        }
        let input =
            unsafe { Retained::retain(input_ptr) }.ok_or("Failed to retain camera input")?;

        // --- Create video data output ---
        let output_cls = AnyClass::get(c"AVCaptureVideoDataOutput")
            .ok_or("AVCaptureVideoDataOutput class not found")?;
        let output: Retained<AnyObject> = unsafe { msg_send![output_cls, new] };

        // Force NV12 pixel format via videoSettings dictionary
        let nsnumber_cls = AnyClass::get(c"NSNumber").unwrap();
        let format_num: Retained<AnyObject> =
            unsafe { msg_send![nsnumber_cls, numberWithUnsignedInt: PIXEL_FORMAT_NV12] };

        let key_bytes = c"PixelFormatType";
        let key_cls = AnyClass::get(c"NSString").unwrap();
        let format_key: Retained<AnyObject> =
            unsafe { msg_send![key_cls, stringWithUTF8String: key_bytes.as_ptr()] };

        let dict_cls = AnyClass::get(c"NSDictionary").unwrap();
        let video_settings: Retained<AnyObject> = unsafe {
            msg_send![dict_cls, dictionaryWithObject: &*format_num, forKey: &*format_key]
        };
        let _: () = unsafe { msg_send![&*output, setVideoSettings: &*video_settings] };

        // Discard late frames
        let _: () = unsafe { msg_send![&*output, setAlwaysDiscardsLateVideoFrames: Bool::YES] };

        // --- Create delegate and dispatch queue ---
        let delegate: Retained<VisioCameraDelegate> =
            unsafe { msg_send![VisioCameraDelegate::class(), new] };

        let queue = unsafe { dispatch_queue_create(c"io.visio.camera".as_ptr(), std::ptr::null()) };

        let _: () =
            unsafe { msg_send![&*output, setSampleBufferDelegate: &*delegate, queue: queue] };

        // --- Add input and output to session ---
        let can_add_input: Bool = unsafe { msg_send![&*session, canAddInput: &*input] };
        if !can_add_input.as_bool() {
            return Err("Cannot add camera input to session".into());
        }
        let _: () = unsafe { msg_send![&*session, addInput: &*input] };

        let can_add_output: Bool = unsafe { msg_send![&*session, canAddOutput: &*output] };
        if !can_add_output.as_bool() {
            return Err("Cannot add video output to session".into());
        }
        let _: () = unsafe { msg_send![&*session, addOutput: &*output] };

        // --- Start ---
        let _: () = unsafe { msg_send![&*session, startRunning] };
        tracing::info!("macOS camera capture started (uid={unique_id})");

        Ok(MacCameraCapture {
            session,
            _delegate: delegate,
            queue,
        })
    }

    unsafe fn start_avfoundation() -> Result<Self, String> {
        // --- Create session ---
        let session_cls =
            AnyClass::get(c"AVCaptureSession").ok_or("AVCaptureSession class not found")?;
        let session: Retained<AnyObject> = unsafe { msg_send![session_cls, new] };

        // Set session preset
        let _: () = unsafe { msg_send![&*session, setSessionPreset: AVCaptureSessionPreset1280x720] };

        // --- Find camera device ---
        let device_cls =
            AnyClass::get(c"AVCaptureDevice").ok_or("AVCaptureDevice class not found")?;
        let device_ptr: *mut AnyObject =
            unsafe { msg_send![device_cls, defaultDeviceWithMediaType: AVMediaTypeVideo] };
        if device_ptr.is_null() {
            return Err("No camera device found".into());
        }
        let device =
            unsafe { Retained::retain(device_ptr) }.ok_or("Failed to retain camera device")?;

        // --- Create device input ---
        let input_cls =
            AnyClass::get(c"AVCaptureDeviceInput").ok_or("AVCaptureDeviceInput class not found")?;
        let mut error_ptr: *mut AnyObject = std::ptr::null_mut();
        let input_ptr: *mut AnyObject =
            unsafe { msg_send![input_cls, deviceInputWithDevice: &*device, error: &mut error_ptr] };
        if input_ptr.is_null() {
            return Err("Failed to create camera input".into());
        }
        let input =
            unsafe { Retained::retain(input_ptr) }.ok_or("Failed to retain camera input")?;

        // --- Create video data output ---
        let output_cls = AnyClass::get(c"AVCaptureVideoDataOutput")
            .ok_or("AVCaptureVideoDataOutput class not found")?;
        let output: Retained<AnyObject> = unsafe { msg_send![output_cls, new] };

        // Force NV12 pixel format via videoSettings dictionary
        let nsnumber_cls = AnyClass::get(c"NSNumber").unwrap();
        let format_num: Retained<AnyObject> =
            unsafe { msg_send![nsnumber_cls, numberWithUnsignedInt: PIXEL_FORMAT_NV12] };

        // kCVPixelBufferPixelFormatTypeKey = "PixelFormatType"
        let key_bytes = c"PixelFormatType";
        let key_cls = AnyClass::get(c"NSString").unwrap();
        let format_key: Retained<AnyObject> =
            unsafe { msg_send![key_cls, stringWithUTF8String: key_bytes.as_ptr()] };

        let dict_cls = AnyClass::get(c"NSDictionary").unwrap();
        let video_settings: Retained<AnyObject> = unsafe {
            msg_send![dict_cls, dictionaryWithObject: &*format_num, forKey: &*format_key]
        };
        let _: () = unsafe { msg_send![&*output, setVideoSettings: &*video_settings] };

        // Discard late frames
        let _: () = unsafe { msg_send![&*output, setAlwaysDiscardsLateVideoFrames: Bool::YES] };

        // --- Create delegate and dispatch queue ---
        let delegate: Retained<VisioCameraDelegate> =
            unsafe { msg_send![VisioCameraDelegate::class(), new] };

        let queue = unsafe { dispatch_queue_create(c"io.visio.camera".as_ptr(), std::ptr::null()) };

        let _: () =
            unsafe { msg_send![&*output, setSampleBufferDelegate: &*delegate, queue: queue] };

        // --- Add input and output to session ---
        let can_add_input: Bool = unsafe { msg_send![&*session, canAddInput: &*input] };
        if !can_add_input.as_bool() {
            return Err("Cannot add camera input to session".into());
        }
        let _: () = unsafe { msg_send![&*session, addInput: &*input] };

        let can_add_output: Bool = unsafe { msg_send![&*session, canAddOutput: &*output] };
        if !can_add_output.as_bool() {
            return Err("Cannot add video output to session".into());
        }
        let _: () = unsafe { msg_send![&*session, addOutput: &*output] };

        // --- Start ---
        let _: () = unsafe { msg_send![&*session, startRunning] };
        tracing::info!("macOS camera capture started");

        Ok(MacCameraCapture {
            session,
            _delegate: delegate,
            queue,
        })
    }

    /// Stop camera capture and release resources.
    pub fn stop(&mut self) {
        let _: () = unsafe { msg_send![&*self.session, stopRunning] };
        tracing::info!("macOS camera capture stopped");

        // Clear the shared state
        let mut state = CAMERA_STATE.lock().unwrap();
        *state = None;

        // Release the dispatch queue
        unsafe { dispatch_release(self.queue) };
    }
}

impl Drop for MacCameraCapture {
    fn drop(&mut self) {
        // Ensure session is stopped if MacCameraCapture is dropped
        let running: Bool = unsafe { msg_send![&*self.session, isRunning] };
        if running.as_bool() {
            self.stop();
        }
    }
}
