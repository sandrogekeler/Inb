//! Carrying out the session's capture, encode, copy and save effects. Capture
//! and encoding run on their own threads and report back as `UserEvent`s.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use inb::encode::{self, Spool};
use winit::event_loop::EventLoopProxy;

use inb::platform::sys::{capture, clipboard, files};
use inb::platform::{CaptureRequest, PlatformError};

#[derive(Debug)]
pub enum UserEvent {
    Progress(f32),
    Encoded(Result<u64, String>),
}

#[derive(Default)]
pub struct Recorder {
    running: Option<(capture::Capture, JoinHandle<Result<Spool, String>>)>,
    cancel: Arc<AtomicBool>,
    gif: Option<PathBuf>,
}

impl Recorder {
    pub fn start(&mut self, request: CaptureRequest) -> Result<(), PlatformError> {
        let (tx, rx) = mpsc::channel();
        let capture = capture::start(request, tx)?;
        let spooler = std::thread::spawn(move || {
            let mut spool = Spool::default();
            for frame in rx {
                spool.push(frame).map_err(|e| e.to_string())?;
            }
            Ok(spool)
        });
        self.running = Some((capture, spooler));
        Ok(())
    }

    /// Stops the capture and encodes on a new thread. The spool ends when the
    /// capture drops its sender.
    pub fn stop_and_encode(&mut self, fps: u32, proxy: EventLoopProxy<UserEvent>) {
        let Some((capture, spooler)) = self.running.take() else { return };
        let stopped = capture.stop();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel);
        let out = temp_gif();
        self.gif = Some(out.clone());
        std::thread::spawn(move || {
            let result = stopped
                .map_err(|e| e.0)
                .and_then(|()| spooler.join().map_err(|_| "Recording stopped unexpectedly".to_string())?)
                .and_then(|spool| {
                    let progress_proxy = proxy.clone();
                    let mut progress = |p| {
                        let _ = progress_proxy.send_event(UserEvent::Progress(p));
                    };
                    encode::encode(spool, &out, fps, &mut progress, &cancel).map_err(|e| e.to_string())
                });
            let _ = proxy.send_event(UserEvent::Encoded(result));
        });
    }

    pub fn cancel_encode(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn copy(&self) -> Result<(), PlatformError> {
        clipboard::copy_gif(self.gif.as_deref().ok_or_else(|| PlatformError::new("No GIF to copy"))?)
    }

    /// Copies the GIF into the save folder as "Inb N.gif", N the first free number.
    pub fn save(&self) -> Result<(), PlatformError> {
        let gif = self.gif.as_deref().ok_or_else(|| PlatformError::new("No GIF to save"))?;
        let dir = files::save_dir()?;
        let target = (1..).map(|n| dir.join(format!("Inb {n}.gif"))).find(|p| !p.exists()).expect("unbounded range");
        std::fs::copy(gif, target).map(|_| ()).map_err(|e| PlatformError::new(format!("Could not save: {e}")))
    }
}

/// A new file per recording, so a GIF already on the clipboard stays intact.
fn temp_gif() -> PathBuf {
    let dir = std::env::temp_dir().join("Inb");
    let _ = std::fs::create_dir_all(&dir);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    dir.join(format!("Inb-{stamp}.gif"))
}
