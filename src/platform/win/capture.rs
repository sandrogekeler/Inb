//! Screen capture through DXGI Desktop Duplication.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows_capture::dxgi_duplication_api::{DxgiDuplicationApi, DxgiDuplicationFormat, Error as DxgiError};
use windows_capture::monitor::Monitor;

use crate::frame::Frame;
use crate::image::{PixelRect, crop_scale_bgra};
use crate::platform::{CaptureRequest, PlatformError};
use winit::monitor::MonitorHandle;
use winit::platform::windows::MonitorHandleExtWindows;

/// How long one tick waits for the desktop to change before the last picture
/// is sent again.
const ACQUIRE_TIMEOUT_MS: u32 = 5;
/// Pause before the second try to get the duplication back.
const RETRY_PAUSE: Duration = Duration::from_millis(200);
const START_FAILED: &str = "Could not start recording";
const STOPPED: &str = "Recording stopped unexpectedly";

/// The display's `HMONITOR`.
pub fn display_id(monitor: &MonitorHandle) -> u64 {
    monitor.hmonitor() as u64
}

/// A running capture. Dropping it without `stop` also stops it.
pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), PlatformError>>>,
}

impl Capture {
    /// Stops capturing. Returns the first error the capture hit, if any.
    pub fn stop(mut self) -> Result<(), PlatformError> {
        self.finish()
    }

    fn finish(&mut self) -> Result<(), PlatformError> {
        self.stop.store(true, Ordering::Relaxed);
        match self.thread.take() {
            Some(thread) => thread.join().unwrap_or_else(|_| Err(PlatformError::new(STOPPED))),
            None => Ok(()),
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

/// Starts capturing `request.area` at `request.fps` and sends each frame,
/// cropped and scaled to the area's size in points, to `frames`. Asks for the
/// screen-recording permission here and nowhere else.
pub fn start(request: CaptureRequest, frames: Sender<Frame>) -> Result<Capture, PlatformError> {
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread = thread::Builder::new()
        .name("inb-capture".into())
        .spawn({
            let stop = Arc::clone(&stop);
            move || run(&request, &frames, &stop, &ready_tx)
        })
        .map_err(|e| failed(START_FAILED, e))?;
    // The duplication is made on the capture thread; wait only for that.
    match ready_rx.recv() {
        Ok(Ok(())) => Ok(Capture { stop, thread: Some(thread) }),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => Err(PlatformError::new(START_FAILED)),
    }
}

/// Logs the details to stderr and returns the short sentence for the bar.
fn failed(message: &str, detail: impl std::fmt::Display) -> PlatformError {
    eprintln!("capture: {message}: {detail}");
    PlatformError::new(message)
}

fn open(monitor: Monitor) -> Result<DxgiDuplicationApi, DxgiError> {
    // Bgra8 only, so every buffer is the BGRA that `crop_scale_bgra` reads.
    DxgiDuplicationApi::new_options(monitor, &[DxgiDuplicationFormat::Bgra8])
}

/// Gets the duplication back after a lost access. A failed first try waits and
/// tries once more from scratch before giving up.
fn reopen(api: DxgiDuplicationApi, monitor: Monitor) -> Result<DxgiDuplicationApi, DxgiError> {
    if let Ok(api) = api.recreate_options(&[DxgiDuplicationFormat::Bgra8]) {
        return Ok(api);
    }
    thread::sleep(RETRY_PAUSE);
    open(monitor)
}

fn run(
    request: &CaptureRequest,
    frames: &Sender<Frame>,
    stop: &AtomicBool,
    ready: &Sender<Result<(), PlatformError>>,
) -> Result<(), PlatformError> {
    let monitor = Monitor::from_raw_hmonitor(request.display as usize as *mut std::ffi::c_void);
    let api = match open(monitor) {
        Ok(api) => api,
        Err(e) => {
            let error = failed(START_FAILED, e);
            let _ = ready.send(Err(error.clone()));
            return Err(error);
        }
    };
    let _ = ready.send(Ok(()));
    capture_loop(api, monitor, request, frames, stop).map_err(|e| failed(STOPPED, e))
}

fn capture_loop(
    mut api: DxgiDuplicationApi,
    monitor: Monitor,
    request: &CaptureRequest,
    frames: &Sender<Frame>,
    stop: &AtomicBool,
) -> Result<(), DxgiError> {
    let interval = Duration::from_secs_f64(1.0 / f64::from(request.fps.max(1)));
    let (out_w, out_h) = (request.area.w.round().max(1.0) as u32, request.area.h.round().max(1.0) as u32);
    let started = Instant::now();
    let mut next = started;
    let mut last: Option<Vec<u8>> = None;
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if next > now {
            thread::sleep((next - now).min(Duration::from_millis(20)));
            continue;
        }
        // A tick that ran late does not make up the missed ones.
        next = (next + interval).max(now);
        match api.acquire_next_frame(ACQUIRE_TIMEOUT_MS) {
            Ok(mut frame) => {
                let crop = pixel_rect(request, frame.width(), frame.height());
                let mut buffer = frame.buffer_crop(crop.x, crop.y, crop.x + crop.w, crop.y + crop.h)?;
                let stride = buffer.row_pitch() as usize;
                let local = PixelRect { x: 0, y: 0, ..crop };
                last = Some(crop_scale_bgra(buffer.as_raw_buffer(), stride, local, out_w, out_h));
            }
            Err(DxgiError::Timeout) => {}
            Err(DxgiError::AccessLost) => {
                api = reopen(api, monitor)?;
                continue;
            }
            Err(e) => return Err(e),
        }
        let Some(rgba) = &last else { continue };
        let frame = Frame { width: out_w, height: out_h, rgba: rgba.clone(), at: started.elapsed() };
        if frames.send(frame).is_err() {
            return Ok(());
        }
    }
    Ok(())
}

/// The area in physical pixels, clamped to a `width` by `height` output and at
/// least one pixel large.
fn pixel_rect(request: &CaptureRequest, width: u32, height: u32) -> PixelRect {
    let a = request.area;
    let edge = |v: f64, max: u32| ((v * request.scale).round().max(0.0) as u32).min(max);
    let x = edge(a.x, width.saturating_sub(1));
    let y = edge(a.y, height.saturating_sub(1));
    let right = edge(a.x + a.w, width).max(x + 1).min(width.max(1));
    let bottom = edge(a.y + a.h, height).max(y + 1).min(height.max(1));
    PixelRect { x, y, w: right - x, h: bottom - y }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    fn request(area: Rect, scale: f64) -> CaptureRequest {
        CaptureRequest { display: 0, area, scale, fps: 15 }
    }

    #[test]
    fn pixel_rect_scales_by_the_display_scale() {
        let r = pixel_rect(&request(Rect::new(100.0, 50.0, 640.0, 400.0), 1.5), 1920, 1080);
        assert_eq!(r, PixelRect { x: 150, y: 75, w: 960, h: 600 });
    }

    #[test]
    fn pixel_rect_stays_inside_the_output() {
        let r = pixel_rect(&request(Rect::new(1800.0, 1000.0, 640.0, 400.0), 1.0), 1920, 1080);
        assert_eq!(r, PixelRect { x: 1800, y: 1000, w: 120, h: 80 });
    }
}
