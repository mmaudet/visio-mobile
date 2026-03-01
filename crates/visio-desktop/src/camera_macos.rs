//! macOS camera capture using AVFoundation.
//!
//! Opens the default camera, forces NV12 pixel format, converts
//! NV12 → I420, and feeds frames into a LiveKit NativeVideoSource.
//! Also emits self-view frames through the visio-video desktop callback.

use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use livekit::webrtc::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, NSObject};
use objc2::{define_class, msg_send, msg_send_id, ClassType};

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
    static AVCaptureSessionPresetHigh: *const AnyObject;
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
        let src_slice =
            unsafe { std::slice::from_raw_parts(y_ptr.add(src_start), w) };
        y_dst[dst_start..dst_start + w].copy_from_slice(src_slice);
    }

    // Deinterleave UV plane into U and V
    let chroma_h = h / 2;
    let chroma_w = w / 2;
    for row in 0..chroma_h {
        let src_row = unsafe {
            std::slice::from_raw_parts(uv_ptr.add(row * uv_stride), chroma_w * 2)
        };
        let dst_row_offset = row * strides.1 as usize;
        for col in 0..chroma_w {
            u_dst[dst_row_offset + col] = src_row[col * 2];
            v_dst[dst_row_offset + col] = src_row[col * 2 + 1];
        }
    }

    // Drop mutable borrows before immutable use
    drop(y_dst);
    drop(u_dst);
    drop(v_dst);

    unsafe { CVPixelBufferUnlockBaseAddress(pxbuf, 1) };

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

    unsafe fn start_avfoundation() -> Result<Self, String> {
        // --- Create session ---
        let session_cls = AnyClass::get(c"AVCaptureSession")
            .ok_or("AVCaptureSession class not found")?;
        let session: Retained<AnyObject> = unsafe { msg_send_id![session_cls, new] };

        // Set session preset
        let _: () = unsafe {
            msg_send![&*session, setSessionPreset: AVCaptureSessionPresetHigh]
        };

        // --- Find camera device ---
        let device_cls = AnyClass::get(c"AVCaptureDevice")
            .ok_or("AVCaptureDevice class not found")?;
        let device_ptr: *mut AnyObject = unsafe {
            msg_send![device_cls, defaultDeviceWithMediaType: AVMediaTypeVideo]
        };
        if device_ptr.is_null() {
            return Err("No camera device found".into());
        }
        let device = unsafe { Retained::retain(device_ptr) }
            .ok_or("Failed to retain camera device")?;

        // --- Create device input ---
        let input_cls = AnyClass::get(c"AVCaptureDeviceInput")
            .ok_or("AVCaptureDeviceInput class not found")?;
        let mut error_ptr: *mut AnyObject = std::ptr::null_mut();
        let input_ptr: *mut AnyObject = unsafe {
            msg_send![input_cls, deviceInputWithDevice: &*device error: &mut error_ptr]
        };
        if input_ptr.is_null() {
            return Err("Failed to create camera input".into());
        }
        let input = unsafe { Retained::retain(input_ptr) }
            .ok_or("Failed to retain camera input")?;

        // --- Create video data output ---
        let output_cls = AnyClass::get(c"AVCaptureVideoDataOutput")
            .ok_or("AVCaptureVideoDataOutput class not found")?;
        let output: Retained<AnyObject> = unsafe { msg_send_id![output_cls, new] };

        // Force NV12 pixel format via videoSettings dictionary
        let nsnumber_cls = AnyClass::get(c"NSNumber").unwrap();
        let format_num: Retained<AnyObject> = unsafe {
            msg_send_id![nsnumber_cls, numberWithUnsignedInt: PIXEL_FORMAT_NV12]
        };

        // kCVPixelBufferPixelFormatTypeKey = "PixelFormatType"
        let key_bytes = c"PixelFormatType";
        let key_cls = AnyClass::get(c"NSString").unwrap();
        let format_key: Retained<AnyObject> = unsafe {
            msg_send_id![key_cls, stringWithUTF8String: key_bytes.as_ptr()]
        };

        let dict_cls = AnyClass::get(c"NSDictionary").unwrap();
        let video_settings: Retained<AnyObject> = unsafe {
            msg_send_id![dict_cls, dictionaryWithObject: &*format_num forKey: &*format_key]
        };
        let _: () = unsafe {
            msg_send![&*output, setVideoSettings: &*video_settings]
        };

        // Discard late frames
        let _: () = unsafe {
            msg_send![&*output, setAlwaysDiscardsLateVideoFrames: Bool::YES]
        };

        // --- Create delegate and dispatch queue ---
        let delegate: Retained<VisioCameraDelegate> = unsafe {
            msg_send_id![VisioCameraDelegate::class(), new]
        };

        let queue = unsafe {
            dispatch_queue_create(
                c"io.visio.camera".as_ptr(),
                std::ptr::null(),
            )
        };

        let _: () = unsafe {
            msg_send![&*output, setSampleBufferDelegate: &*delegate, queue: queue]
        };

        // --- Add input and output to session ---
        let can_add_input: Bool = unsafe {
            msg_send![&*session, canAddInput: &*input]
        };
        if !can_add_input.as_bool() {
            return Err("Cannot add camera input to session".into());
        }
        let _: () = unsafe { msg_send![&*session, addInput: &*input] };

        let can_add_output: Bool = unsafe {
            msg_send![&*session, canAddOutput: &*output]
        };
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
