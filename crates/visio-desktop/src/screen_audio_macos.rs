//! macOS screen share audio capture using ScreenCaptureKit.
//!
//! Uses SCStream with `capturesAudio = true` and `excludesCurrentProcessAudio = true`
//! to capture system audio (everything except our own app). Audio samples from the
//! SCStreamDelegate are converted to 48kHz mono i16 and fed to a NativeAudioSource.
//!
//! Requires macOS 13.0+ (Ventura).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool};

use super::audio_engine::{LK_CHANNELS, LK_SAMPLE_RATE};

/// Screen audio capture session.
pub struct ScreenAudioCapture {
    _stream: Retained<AnyObject>, // SCStream — prevent dealloc
    stop_flag: Arc<AtomicBool>,
}

unsafe impl Send for ScreenAudioCapture {}
unsafe impl Sync for ScreenAudioCapture {}

unsafe extern "C" {
    fn CMSampleBufferGetDataBuffer(sbuf: *const AnyObject) -> *const AnyObject;
    fn CMBlockBufferGetDataLength(block: *const AnyObject) -> usize;
    fn CMBlockBufferGetDataPointer(
        block: *const AnyObject,
        offset: usize,
        length_at_offset: *mut usize,
        total_length: *mut usize,
        data_pointer: *mut *const u8,
    ) -> i32;
    fn CMSampleBufferGetNumSamples(sbuf: *const AnyObject) -> i64;
    fn dispatch_queue_create(
        label: *const std::ffi::c_char,
        attr: *const std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
}

impl ScreenAudioCapture {
    /// Start capturing system audio for the entire display.
    /// The captured audio is fed to `audio_source` as 48kHz mono i16 frames.
    pub fn start(audio_source: NativeAudioSource) -> Result<Self, String> {
        unsafe { Self::start_inner(audio_source) }
    }

    unsafe fn start_inner(audio_source: NativeAudioSource) -> Result<Self, String> {
        // Check ScreenCaptureKit availability (macOS 13+)
        let sc_shareable_cls = AnyClass::get(c"SCShareableContent")
            .ok_or("SCShareableContent not available (requires macOS 13+)")?;
        let sc_stream_cls = AnyClass::get(c"SCStream").ok_or("SCStream not available")?;
        let sc_config_cls =
            AnyClass::get(c"SCStreamConfiguration").ok_or("SCStreamConfiguration not available")?;
        let sc_filter_cls =
            AnyClass::get(c"SCContentFilter").ok_or("SCContentFilter not available")?;

        let content = unsafe { Self::get_shareable_content_sync(sc_shareable_cls) }?;

        // Get the first display
        let displays: *const AnyObject = msg_send![&*content, displays];
        let display_count: usize = msg_send![displays, count];
        if display_count == 0 {
            return Err("no displays found".into());
        }
        let display: *const AnyObject = msg_send![displays, objectAtIndex: 0usize];

        // Create content filter for the display (all windows)
        let nsarray_cls = AnyClass::get(c"NSMutableArray").unwrap();
        let empty_array: *mut AnyObject = msg_send![nsarray_cls, array];

        let filter: *mut AnyObject = msg_send![sc_filter_cls, alloc];
        let filter: *mut AnyObject = msg_send![
            filter,
            initWithDisplay: display,
            excludingWindows: empty_array
        ];

        // Create configuration: audio only, no video
        let config: *mut AnyObject = msg_send![sc_config_cls, new];
        let _: () = msg_send![config, setCapturesAudio: Bool::YES];
        let _: () = msg_send![config, setExcludesCurrentProcessAudio: Bool::YES];
        // Minimal video to save resources
        let _: () = msg_send![config, setWidth: 2u64];
        let _: () = msg_send![config, setHeight: 2u64];
        // Audio settings: 48kHz mono
        let _: () = msg_send![config, setSampleRate: LK_SAMPLE_RATE as i64];
        let _: () = msg_send![config, setChannelCount: LK_CHANNELS as i64];

        // Create the delegate that receives audio buffers
        let stop_flag = Arc::new(AtomicBool::new(false));
        let delegate = ScreenAudioDelegate::new(audio_source, stop_flag.clone());

        // Create SCStream
        let stream: *mut AnyObject = msg_send![sc_stream_cls, alloc];
        let stream: *mut AnyObject = msg_send![
            stream,
            initWithFilter: filter,
            configuration: config,
            delegate: &*delegate.0
        ];
        if stream.is_null() {
            return Err("SCStream init returned nil".into());
        }
        let stream: Retained<AnyObject> = unsafe { Retained::retain(stream) }.unwrap();

        // Add stream output for audio
        // SCStreamOutputType.audio = 1
        let queue =
            unsafe { dispatch_queue_create(c"io.visio.screen-audio".as_ptr(), std::ptr::null()) };
        if queue.is_null() {
            return Err("failed to create dispatch queue".into());
        }
        let dispatch_queue: Retained<AnyObject> =
            unsafe { Retained::retain(queue as *mut AnyObject) }.unwrap();

        let result: Bool = msg_send![
            &*stream,
            addStreamOutput: &*delegate.0,
            type: 1i64,
            sampleHandlerQueue: &*dispatch_queue,
            error: std::ptr::null_mut::<*mut AnyObject>()
        ];
        if !result.as_bool() {
            return Err("failed to add stream audio output".into());
        }

        // Start capture
        let _: () = msg_send![
            &*stream,
            startCaptureWithCompletionHandler: std::ptr::null::<AnyObject>()
        ];

        tracing::info!("macOS screen audio capture started via ScreenCaptureKit");

        Ok(Self {
            _stream: stream,
            stop_flag,
        })
    }

    /// Get SCShareableContent synchronously by blocking on the async callback.
    unsafe fn get_shareable_content_sync(cls: &AnyClass) -> Result<Retained<AnyObject>, String> {
        use std::sync::{Condvar, Mutex};

        let result: Arc<Mutex<Option<Retained<AnyObject>>>> = Arc::new(Mutex::new(None));
        let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let done = Arc::new((Mutex::new(false), Condvar::new()));

        let result_clone = result.clone();
        let error_clone = error.clone();
        let done_clone = done.clone();

        // Build an Objective-C block for the completion handler
        let block = block2::StackBlock::new(move |content: *mut AnyObject, err: *mut AnyObject| {
            if !err.is_null() {
                let desc: *const AnyObject = msg_send![err, localizedDescription];
                let cstr: *const std::ffi::c_char = msg_send![desc, UTF8String];
                let msg = unsafe { std::ffi::CStr::from_ptr(cstr) }
                    .to_string_lossy()
                    .to_string();
                *error_clone.lock().unwrap() = Some(msg);
            } else if !content.is_null() {
                let retained: Retained<AnyObject> = unsafe { Retained::retain(content) }.unwrap();
                *result_clone.lock().unwrap() = Some(retained);
            }
            let (lock, cvar) = &*done_clone;
            *lock.lock().unwrap() = true;
            cvar.notify_one();
        });

        let _: () = msg_send![
            cls,
            getShareableContentExcludingDesktopWindows: Bool::YES,
            onScreenWindowsOnly: Bool::YES,
            completionHandler: &*block
        ];

        // Wait for completion (with timeout)
        let (lock, cvar) = &*done;
        let guard = lock.lock().unwrap();
        let _guard = cvar
            .wait_timeout(guard, std::time::Duration::from_secs(5))
            .unwrap();

        if let Some(err) = error.lock().unwrap().take() {
            return Err(format!("SCShareableContent error: {err}"));
        }

        result
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| "SCShareableContent returned nil".into())
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        unsafe {
            let _: () = msg_send![
                &*self._stream,
                stopCaptureWithCompletionHandler: std::ptr::null::<AnyObject>()
            ];
        }
        tracing::info!("macOS screen audio capture stopped");
    }
}

impl Drop for ScreenAudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Wrapper around an NSObject subclass that acts as SCStreamOutput delegate.
struct ScreenAudioDelegate(Retained<AnyObject>);

unsafe impl Send for ScreenAudioDelegate {}
unsafe impl Sync for ScreenAudioDelegate {}

impl ScreenAudioDelegate {
    fn new(audio_source: NativeAudioSource, stop_flag: Arc<AtomicBool>) -> Self {
        let proxy = AudioCallbackProxy::new(audio_source, stop_flag);
        AUDIO_PROXY.lock().unwrap().replace(proxy);

        unsafe {
            let cls = AnyClass::get(c"ScreenAudioOutputDelegate")
                .unwrap_or_else(|| register_delegate_class());
            let obj: Retained<AnyObject> = msg_send![cls, new];
            Self(obj)
        }
    }
}

static AUDIO_PROXY: std::sync::Mutex<Option<AudioCallbackProxy>> = std::sync::Mutex::new(None);

struct AudioCallbackProxy {
    audio_source: NativeAudioSource,
    stop_flag: Arc<AtomicBool>,
    rt: tokio::runtime::Runtime,
}

impl AudioCallbackProxy {
    fn new(audio_source: NativeAudioSource, stop_flag: Arc<AtomicBool>) -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("screen audio runtime");
        Self {
            audio_source,
            stop_flag,
            rt,
        }
    }

    fn handle_audio_buffer(&self, sample_buffer: *const AnyObject) {
        if self.stop_flag.load(Ordering::Relaxed) {
            return;
        }

        unsafe {
            let block_buffer = CMSampleBufferGetDataBuffer(sample_buffer);
            if block_buffer.is_null() {
                return;
            }

            let data_len = CMBlockBufferGetDataLength(block_buffer);
            if data_len == 0 {
                return;
            }

            let mut data_ptr: *const u8 = std::ptr::null();
            let mut length_at_offset: usize = 0;
            let mut total_length: usize = 0;

            let status = CMBlockBufferGetDataPointer(
                block_buffer,
                0,
                &mut length_at_offset,
                &mut total_length,
                &mut data_ptr,
            );
            if status != 0 || data_ptr.is_null() {
                return;
            }

            let num_samples = CMSampleBufferGetNumSamples(sample_buffer) as u32;
            if num_samples == 0 {
                return;
            }

            // SCStream delivers float32 PCM at the configured sample rate
            // Convert f32 → i16 for LiveKit
            let float_samples = std::slice::from_raw_parts(
                data_ptr as *const f32,
                total_length / std::mem::size_of::<f32>(),
            );

            // SCStream may deliver stereo if channel count wasn't respected;
            // mix to mono if needed
            let mono: Vec<i16> = if float_samples.len() > num_samples as usize {
                // Multi-channel — mix to mono
                let channels = float_samples.len() / num_samples as usize;
                (0..num_samples as usize)
                    .map(|i| {
                        let mut sum = 0.0f32;
                        for ch in 0..channels {
                            sum += float_samples[i * channels + ch];
                        }
                        let avg = sum / channels as f32;
                        (avg * 32767.0).clamp(-32768.0, 32767.0) as i16
                    })
                    .collect()
            } else {
                float_samples
                    .iter()
                    .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
                    .collect()
            };

            let frame = AudioFrame {
                data: mono.into(),
                sample_rate: LK_SAMPLE_RATE,
                num_channels: LK_CHANNELS,
                samples_per_channel: num_samples,
            };

            self.rt.block_on(async {
                let _ = self.audio_source.capture_frame(&frame).await;
            });
        }
    }
}

/// Register a custom ObjC class that implements SCStreamOutput protocol.
unsafe fn register_delegate_class() -> &'static AnyClass {
    use objc2::runtime::{ClassBuilder, Sel};

    let superclass = AnyClass::get(c"NSObject").unwrap();
    let mut builder = ClassBuilder::new(c"ScreenAudioOutputDelegate", superclass).unwrap();

    // stream:didOutputSampleBuffer:ofType:
    // Use NonNull pointers + raw pointers to avoid lifetime issues with MethodImplementation
    unsafe extern "C" fn did_output_sample_buffer(
        _this: *mut AnyObject,
        _sel: Sel,
        _stream: *const AnyObject,
        sample_buffer: *const AnyObject,
        output_type: i64,
    ) {
        // output_type 1 = audio
        if output_type != 1 {
            return;
        }

        if let Some(proxy) = AUDIO_PROXY.lock().unwrap().as_ref() {
            proxy.handle_audio_buffer(sample_buffer);
        }
    }

    unsafe {
        builder.add_method(
            Sel::register(c"stream:didOutputSampleBuffer:ofType:"),
            did_output_sample_buffer
                as unsafe extern "C" fn(
                    *mut AnyObject,
                    Sel,
                    *const AnyObject,
                    *const AnyObject,
                    i64,
                ),
        )
    };

    builder.register()
}
