//! Screen capture through ScreenCaptureKit. The stream already crops to the
//! area and scales to its size in points, so each sample becomes one `Frame`.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_32BGRA, kCVReturnSuccess,
};
use objc2_foundation::{NSArray, NSError, NSObject, NSObjectProtocol, NSString};
use objc2_screen_capture_kit::{
    SCContentFilter, SCFrameStatus, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamDelegate,
    SCStreamFrameInfoStatus, SCStreamOutput, SCStreamOutputType,
};
use winit::monitor::MonitorHandle;
use winit::platform::macos::MonitorHandleExtMacOS;

use crate::frame::Frame;
use crate::image::{PixelRect, crop_scale_bgra};
use crate::platform::{CaptureRequest, PlatformError};

/// How long ScreenCaptureKit gets to answer a request.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

/// The display's `CGDirectDisplayID`.
pub fn display_id(monitor: &MonitorHandle) -> u64 {
    u64::from(monitor.native_id())
}

/// What the sample handler, the repeat thread and `stop` share.
struct Shared {
    started: Instant,
    inner: Mutex<Inner>,
    /// The first error the stream reported on its own.
    stream_error: Mutex<Option<String>>,
}

struct Inner {
    /// `None` once stopped: the spool ends when this drops.
    frames: Option<Sender<Frame>>,
    last: Option<Frame>,
    /// A sample arrived since the repeat thread last looked.
    fresh: bool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn push(&self, width: u32, height: u32, rgba: Vec<u8>) {
        let mut inner = self.lock();
        let Some(frames) = &inner.frames else { return };
        let frame = Frame { width, height, rgba, at: self.started.elapsed() };
        let _ = frames.send(frame.clone());
        inner.last = Some(frame);
        inner.fresh = true;
    }

    /// Resends the last picture when no sample came since the last call, so
    /// time keeps flowing on a still screen. False once the capture stopped.
    fn repeat_last(&self) -> bool {
        let mut inner = self.lock();
        let Some(frames) = &inner.frames else { return false };
        if let (false, Some(last)) = (inner.fresh, &inner.last) {
            let frame = Frame { at: self.started.elapsed(), ..last.clone() };
            if frames.send(frame).is_err() {
                return false;
            }
        }
        inner.fresh = false;
        true
    }

    fn close(&self) {
        self.lock().frames = None;
    }
}

struct SinkState {
    shared: Arc<Shared>,
    width: u32,
    height: u32,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and the class has no Drop.
    #[unsafe(super(NSObject))]
    #[name = "InbFrameSink"]
    #[ivars = SinkState]
    struct FrameSink;

    unsafe impl NSObjectProtocol for FrameSink {}

    unsafe impl SCStreamOutput for FrameSink {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        #[allow(non_snake_case)]
        unsafe fn stream_didOutputSampleBuffer_ofType(
            &self,
            _stream: &SCStream,
            sample_buffer: &CMSampleBuffer,
            r#type: SCStreamOutputType,
        ) {
            if r#type == SCStreamOutputType::Screen {
                handle_sample(self.ivars(), sample_buffer);
            }
        }
    }

    unsafe impl SCStreamDelegate for FrameSink {
        #[unsafe(method(stream:didStopWithError:))]
        #[allow(non_snake_case)]
        unsafe fn stream_didStopWithError(&self, _stream: &SCStream, error: &NSError) {
            let mut slot = self.ivars().shared.stream_error.lock().unwrap_or_else(|p| p.into_inner());
            slot.get_or_insert_with(|| error.localizedDescription().to_string());
        }
    }
);

impl FrameSink {
    fn new(state: SinkState) -> Retained<Self> {
        let this = Self::alloc().set_ivars(state);
        // SAFETY: NSObject's `init` takes no arguments and returns the object.
        unsafe { msg_send![super(this), init] }
    }
}

fn handle_sample(state: &SinkState, sample: &CMSampleBuffer) {
    if !is_complete(sample) {
        return;
    }
    // SAFETY: `sample` is a valid sample buffer for the length of the callback.
    let Some(buffer) = (unsafe { sample.image_buffer() }) else { return };
    if let Some(rgba) = to_rgba(&buffer, state.width, state.height) {
        state.shared.push(state.width, state.height, rgba);
    }
}

/// True when the sample carries a new picture. Idle and blank samples do not.
fn is_complete(sample: &CMSampleBuffer) -> bool {
    // SAFETY: `sample` is a valid sample buffer for the length of the callback.
    let Some(array) = (unsafe { sample.sample_attachments_array(false) }) else { return false };
    // SAFETY: ScreenCaptureKit documents the attachments as an array of dictionaries from
    // string keys to numbers.
    let array = unsafe { CFRetained::cast_unchecked::<CFArray<CFDictionary<CFString, CFNumber>>>(array) };
    let Some(info) = array.get(0) else { return false };
    // SAFETY: an NSString is a CFString (toll-free bridged) and the static lives for the program.
    let key = unsafe { &*std::ptr::from_ref::<NSString>(SCStreamFrameInfoStatus).cast::<CFString>() };
    info.get(key).and_then(|n| n.as_isize()) == Some(SCFrameStatus::Complete.0)
}

/// The buffer as straight RGBA at `width` by `height`, or `None` when it cannot be read.
fn to_rgba(buffer: &CVPixelBuffer, width: u32, height: u32) -> Option<Vec<u8>> {
    let flags = CVPixelBufferLockFlags::ReadOnly;
    // SAFETY: `buffer` is a live pixel buffer; the lock is released below on every path.
    if unsafe { CVPixelBufferLockBaseAddress(buffer, flags) } != kCVReturnSuccess {
        return None;
    }
    let (w, h) = (CVPixelBufferGetWidth(buffer), CVPixelBufferGetHeight(buffer));
    let stride = CVPixelBufferGetBytesPerRow(buffer);
    let base = CVPixelBufferGetBaseAddress(buffer);
    let bgra = CVPixelBufferGetPixelFormatType(buffer) == kCVPixelFormatType_32BGRA;
    let rgba = if base.is_null() || !bgra || w == 0 || h == 0 {
        None
    } else {
        // SAFETY: while locked, `base` points at `stride * h` readable bytes.
        let bytes = unsafe { std::slice::from_raw_parts(base.cast::<u8>(), stride * h) };
        let crop = PixelRect { x: 0, y: 0, w: w as u32, h: h as u32 };
        Some(crop_scale_bgra(bytes, stride, crop, width, height))
    };
    // SAFETY: the buffer was locked above with the same flags.
    unsafe { CVPixelBufferUnlockBaseAddress(buffer, flags) };
    rgba
}

type Completion = (RcBlock<dyn Fn(*mut NSError)>, Receiver<Option<String>>);

/// A completion block and the receiver of what it reports: the error text, if any.
fn completion() -> Completion {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |error: *mut NSError| {
        // SAFETY: ScreenCaptureKit passes null or a valid error for the call.
        let message = unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string());
        let _ = tx.send(message);
    });
    (block, rx)
}

fn wait<T>(rx: &Receiver<T>) -> Result<T, PlatformError> {
    rx.recv_timeout(ANSWER_TIMEOUT).map_err(|_| PlatformError::new("The screen did not answer in time"))
}

fn finished(rx: &Receiver<Option<String>>) -> Result<(), PlatformError> {
    wait(rx)?.map_or(Ok(()), |message| Err(PlatformError(message)))
}

/// The content filter for display `id` that leaves Inb's own windows out.
fn content_filter(id: u64) -> Result<Retained<SCContentFilter>, PlatformError> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |content: *mut SCShareableContent, error: *mut NSError| {
        // SAFETY: both pointers are null or valid for the call; `retain` takes our own reference.
        let result = match unsafe { Retained::retain(content) } {
            Some(content) => Ok(content),
            None => Err(unsafe { error.as_ref() }
                .map_or_else(|| "No screen to record".to_string(), |e| e.localizedDescription().to_string())),
        };
        let _ = tx.send(result);
    });
    // SAFETY: the block outlives the call and the wait below.
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
    let content = wait(&rx)?.map_err(PlatformError)?;
    // SAFETY: plain property reads on a content object we hold.
    let display = unsafe { content.displays() }
        .iter()
        .find(|d| u64::from(unsafe { d.displayID() }) == id)
        .ok_or_else(|| PlatformError::new("The display to record is gone"))?;
    let own = std::process::id() as i32;
    // SAFETY: as above.
    let mine: Vec<_> = unsafe { content.applications() }.iter().filter(|a| unsafe { a.processID() } == own).collect();
    // SAFETY: the display, applications and the empty window list are valid objects.
    Ok(unsafe {
        SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
            SCContentFilter::alloc(),
            &display,
            &NSArray::from_retained_slice(&mine),
            &NSArray::new(),
        )
    })
}

/// Output at the area's size in points; ScreenCaptureKit scales the Retina pixels down.
fn configuration(request: &CaptureRequest, width: u32, height: u32) -> Retained<SCStreamConfiguration> {
    let area = request.area;
    // SAFETY: setters on a configuration object we own; `CMTime::new` only builds a value.
    unsafe {
        let config = SCStreamConfiguration::new();
        config.setSourceRect(CGRect::new(CGPoint::new(area.x, area.y), CGSize::new(area.w, area.h)));
        config.setWidth(width as usize);
        config.setHeight(height as usize);
        config.setMinimumFrameInterval(CMTime::new(1, request.fps.max(1) as i32));
        config.setPixelFormat(kCVPixelFormatType_32BGRA);
        config.setShowsCursor(true);
        config
    }
}

/// A running capture. Dropping it without `stop` also stops it.
pub struct Capture {
    stream: Option<Retained<SCStream>>,
    shared: Arc<Shared>,
    repeater: Option<JoinHandle<()>>,
    _sink: Retained<FrameSink>,
    _queue: DispatchRetained<DispatchQueue>,
}

impl Capture {
    /// Stops capturing. Returns the first error the capture hit, if any.
    pub fn stop(mut self) -> Result<(), PlatformError> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> Result<(), PlatformError> {
        let Some(stream) = self.stream.take() else { return Ok(()) };
        let (block, rx) = completion();
        // SAFETY: the block outlives the call and the wait below.
        unsafe { stream.stopCaptureWithCompletionHandler(Some(&block)) };
        let stopped = finished(&rx);
        self.shared.close();
        if let Some(repeater) = self.repeater.take() {
            let _ = repeater.join();
        }
        let own = self.shared.stream_error.lock().unwrap_or_else(|p| p.into_inner()).take();
        own.map_or(stopped, |message| Err(PlatformError(message)))
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// Starts capturing `request.area` at `request.fps` and sends each frame,
/// cropped and scaled to the area's size in points, to `frames`. Asks for the
/// screen-recording permission here and nowhere else.
pub fn start(request: CaptureRequest, frames: Sender<Frame>) -> Result<Capture, PlatformError> {
    if !CGPreflightScreenCaptureAccess() {
        CGRequestScreenCaptureAccess();
        return Err(PlatformError::new("Allow Inb under Screen Recording in System Settings"));
    }
    let width = request.area.w.round().max(1.0) as u32;
    let height = request.area.h.round().max(1.0) as u32;
    let filter = content_filter(request.display)?;
    let config = configuration(&request, width, height);
    let shared = Arc::new(Shared {
        started: Instant::now(),
        inner: Mutex::new(Inner { frames: Some(frames), last: None, fresh: false }),
        stream_error: Mutex::new(None),
    });
    let sink = FrameSink::new(SinkState { shared: Arc::clone(&shared), width, height });
    let queue = DispatchQueue::new("inb.capture", None);
    // SAFETY: filter, configuration and the sink (output and delegate) are valid objects.
    let stream = unsafe {
        let delegate = ProtocolObject::from_ref(&*sink);
        let stream =
            SCStream::initWithFilter_configuration_delegate(SCStream::alloc(), &filter, &config, Some(delegate));
        stream
            .addStreamOutput_type_sampleHandlerQueue_error(
                ProtocolObject::from_ref(&*sink),
                SCStreamOutputType::Screen,
                Some(&queue),
            )
            .map_err(|e| PlatformError(e.localizedDescription().to_string()))?;
        let (block, rx) = completion();
        stream.startCaptureWithCompletionHandler(Some(&block));
        finished(&rx)?;
        stream
    };
    let interval = Duration::from_secs_f64(1.0 / f64::from(request.fps.max(1)));
    let ticking = Arc::clone(&shared);
    let repeater = std::thread::spawn(move || {
        while {
            std::thread::sleep(interval);
            ticking.repeat_last()
        } {}
    });
    Ok(Capture { stream: Some(stream), shared, repeater: Some(repeater), _sink: sink, _queue: queue })
}
