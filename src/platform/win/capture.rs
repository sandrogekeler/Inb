//! shortcut: placeholder until #11 lands.

use std::sync::mpsc::Sender;

use crate::frame::Frame;
use winit::monitor::MonitorHandle;
use winit::platform::windows::MonitorHandleExtWindows;

use crate::platform::{CaptureRequest, PlatformError};

/// The display's `HMONITOR`.
pub fn display_id(monitor: &MonitorHandle) -> u64 {
    monitor.hmonitor() as u64
}

/// A running capture. Dropping it without `stop` also stops it.
pub struct Capture;

impl Capture {
    /// Stops capturing. Returns the first error the capture hit, if any.
    pub fn stop(self) -> Result<(), PlatformError> {
        Ok(())
    }
}

/// Starts capturing `request.area` at `request.fps` and sends each frame,
/// cropped and scaled to the area's size in points, to `frames`. Asks for the
/// screen-recording permission here and nowhere else.
pub fn start(request: CaptureRequest, frames: Sender<Frame>) -> Result<Capture, PlatformError> {
    let _ = (request, frames);
    Err(PlatformError::new("Recording is not built yet"))
}
