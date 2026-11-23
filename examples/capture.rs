//! Records an area of the main display and writes its first and last frame.
//!
//! cargo run --release --example capture -- <x> <y> <w> <h> <seconds> <fps>
//!
//! The area is in points from the display's top-left corner. Writes
//! target/capture/first.png and last.png and prints the frame count and the
//! mean interval between frames.

#[cfg(any(windows, target_os = "macos"))]
fn main() {
    let args: Vec<f64> = std::env::args().skip(1).map(|a| a.parse().expect("numbers only")).collect();
    let [x, y, w, h, seconds, fps] = args[..] else {
        eprintln!("usage: capture <x> <y> <w> <h> <seconds> <fps>");
        std::process::exit(2);
    };
    // winit is the one place Inb learns about displays, so the example asks it too.
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    let mut app = Once { area: inb::geometry::Rect::new(x, y, w, h), seconds, fps: fps as u32 };
    event_loop.run_app(&mut app).expect("event loop");
}

#[cfg(any(windows, target_os = "macos"))]
struct Once {
    area: inb::geometry::Rect,
    seconds: f64,
    fps: u32,
}

#[cfg(any(windows, target_os = "macos"))]
impl winit::application::ApplicationHandler for Once {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let monitor = event_loop.primary_monitor().expect("a display");
        record(&monitor, self.area, self.seconds, self.fps);
        event_loop.exit();
    }

    fn window_event(
        &mut self,
        _: &winit::event_loop::ActiveEventLoop,
        _: winit::window::WindowId,
        _: winit::event::WindowEvent,
    ) {
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn record(monitor: &winit::monitor::MonitorHandle, area: inb::geometry::Rect, seconds: f64, fps: u32) {
    use inb::platform::CaptureRequest;
    use inb::platform::sys::capture;

    let request = CaptureRequest { display: capture::display_id(monitor), area, scale: monitor.scale_factor(), fps };
    let (tx, rx) = std::sync::mpsc::channel();
    let session = capture::start(request, tx).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
    if let Err(e) = session.stop() {
        eprintln!("{e}");
    }
    let frames: Vec<_> = rx.into_iter().collect();
    let (Some(first), Some(last)) = (frames.first(), frames.last()) else {
        eprintln!("no frames");
        std::process::exit(1);
    };
    std::fs::create_dir_all("target/capture").expect("target/capture");
    for (name, frame) in [("first", first), ("last", last)] {
        let size = tiny_skia::IntSize::from_wh(frame.width, frame.height).expect("non-empty frame");
        let pixmap = tiny_skia::Pixmap::from_vec(premultiply(&frame.rgba), size).expect("rgba length");
        pixmap.save_png(format!("target/capture/{name}.png")).expect("png");
    }
    let span = last.at.saturating_sub(first.at).as_secs_f64();
    println!("{} frames of {}x{}", frames.len(), first.width, first.height);
    println!("mean interval {:.1} ms", 1000.0 * span / (frames.len().max(2) - 1) as f64);
}

#[cfg(any(windows, target_os = "macos"))]
fn premultiply(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|p| {
            let a = u16::from(p[3]);
            [0, 1, 2].map(|i| (u16::from(p[i]) * a / 255) as u8).into_iter().chain([p[3]])
        })
        .collect()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn main() {
    eprintln!("capture runs on macOS and Windows.");
}
