//! The event loop: creates Inb's windows, turns mouse input into frame drags
//! and session events, and redraws. Platform code is reached through `sys`.

mod recording;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use ab_glyph::FontVec;
use inb::bar::{self, Control};
use inb::geometry::{self, Grab, Layout, Rect, SIDES};
use inb::session::{Effect, Event, Session};
use inb::{draw, spec};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::monitor::MonitorHandle;
use winit::window::{CursorIcon, Window, WindowAttributes, WindowId, WindowLevel};

use inb::platform::sys::{capture, files, window as syswin};
use inb::platform::{CaptureRequest, Role};
use recording::{Recorder, UserEvent};

pub fn run() -> Result<(), winit::error::EventLoopError> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let mut app = App::new(event_loop.create_proxy());
    event_loop.run_app(&mut app)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Target {
    Edge(usize),
    Dim(usize),
    Bar,
    Menu,
}

struct Drag {
    grab: Grab,
    start_area: Rect,
    start_x: f64,
    start_y: f64,
}

struct App {
    proxy: EventLoopProxy<UserEvent>,
    started: Instant,
    session: Session,
    recorder: Recorder,
    font: Option<FontVec>,
    monitor: Option<MonitorHandle>,
    screen: Rect,
    scale: f64,
    area: Rect,
    windows: HashMap<Target, Window>,
    targets: HashMap<WindowId, Target>,
    /// Last cursor position per window, in desktop points.
    cursor: Option<(Target, f64, f64)>,
    drag: Option<Drag>,
    hovered_fps: Option<u32>,
    quit: bool,
}

impl App {
    fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            proxy,
            started: Instant::now(),
            session: Session::default(),
            recorder: Recorder::default(),
            font: files::ui_font().and_then(|bytes| FontVec::try_from_vec(bytes).ok()),
            monitor: None,
            screen: Rect::new(0.0, 0.0, 1.0, 1.0),
            scale: 1.0,
            area: Rect::new(0.0, 0.0, 1.0, 1.0),
            windows: HashMap::new(),
            targets: HashMap::new(),
            cursor: None,
            drag: None,
            hovered_fps: None,
            quit: false,
        }
    }

    fn now(&self) -> Duration {
        self.started.elapsed()
    }

    fn layout(&self) -> Layout {
        geometry::layout(self.area, self.screen)
    }

    /// The fps chip in desktop points.
    fn menu_rect(&self) -> Rect {
        let layout = self.layout();
        let chip = bar::controls(&self.session)
            .into_iter()
            .find(|(c, _)| *c == Control::FpsChip)
            .map_or(Rect::new(0.0, 0.0, 68.0, spec::BUTTON), |(_, r)| r);
        let chip = Rect::new(layout.bar.x + chip.x, layout.bar.y + chip.y, chip.w, chip.h);
        geometry::menu(chip, spec::FPS_OPTIONS.len(), layout.bar_below, self.screen)
    }

    fn rect_of(&self, target: Target) -> Rect {
        let layout = self.layout();
        match target {
            Target::Edge(i) => layout.edges[i],
            Target::Dim(i) => layout.dims[i],
            Target::Bar => layout.bar,
            Target::Menu => self.menu_rect(),
        }
    }

    fn create_windows(&mut self, event_loop: &ActiveEventLoop) {
        let monitor = event_loop.primary_monitor().or_else(|| event_loop.available_monitors().next());
        let Some(monitor) = monitor else { return };
        self.scale = monitor.scale_factor();
        let pos = monitor.position().to_logical::<f64>(self.scale);
        let size = monitor.size().to_logical::<f64>(self.scale);
        self.screen = Rect::new(pos.x, pos.y, size.width, size.height);
        self.area = geometry::start_area(self.screen);
        self.monitor = Some(monitor);
        let targets = (0..4).map(Target::Dim).chain((0..4).map(Target::Edge)).chain([Target::Bar, Target::Menu]);
        for target in targets {
            let role = match target {
                Target::Edge(_) => Role::Edge,
                Target::Dim(_) => Role::Dim,
                Target::Bar => Role::Bar,
                Target::Menu => Role::Menu,
            };
            let rect = self.rect_of(target);
            let attributes = WindowAttributes::default()
                .with_title("Inb")
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_visible(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_position(LogicalPosition::new(rect.x, rect.y))
                .with_inner_size(LogicalSize::new(rect.w.max(1.0), rect.h.max(1.0)));
            let Ok(window) = event_loop.create_window(attributes) else { continue };
            if role == Role::Dim {
                let _ = window.set_cursor_hittest(false);
            }
            if let Err(e) = syswin::prepare(&window, role) {
                eprintln!("Inb: {e}");
            }
            self.targets.insert(window.id(), target);
            self.windows.insert(target, window);
        }
        self.place_and_redraw();
    }

    /// Moves every window to the current layout, then draws and shows it.
    fn place_and_redraw(&mut self) {
        let now = self.now();
        let locked = self.session.frame_locked();
        for (&target, window) in &self.windows {
            let rect = self.rect_of(target);
            let visible = match target {
                Target::Menu => self.session.menu_open,
                _ => rect.w >= 1.0 && rect.h >= 1.0,
            };
            window.set_visible(visible);
            if !visible {
                continue;
            }
            window.set_outer_position(LogicalPosition::new(rect.x, rect.y));
            let _ = window.request_inner_size(LogicalSize::new(rect.w, rect.h));
            let pixmap = match target {
                Target::Edge(i) => draw::edge(SIDES[i], rect, self.session.red(now), !locked, self.scale),
                Target::Dim(_) => draw::dim(rect, self.scale),
                Target::Bar => draw::bar(&self.session, now, self.area, self.font.as_ref(), self.scale),
                Target::Menu => draw::menu(rect, self.session.fps, self.hovered_fps, self.font.as_ref(), self.scale),
            };
            if let Err(e) = syswin::present(window, &pixmap) {
                eprintln!("Inb: {e}");
            }
        }
    }

    fn send(&mut self, event: Event) {
        let now = self.now();
        for effect in self.session.handle(now, event) {
            self.apply(effect);
        }
        self.place_and_redraw();
    }

    fn apply(&mut self, effect: Effect) {
        let failed = match effect {
            Effect::StartCapture => self.capture_request().and_then(|r| self.recorder.start(r)).err(),
            Effect::StopCapture => {
                self.recorder.stop_and_encode(self.session.fps, self.proxy.clone());
                None
            }
            Effect::CancelEncode => {
                self.recorder.cancel_encode();
                None
            }
            Effect::CopyGif => self.recorder.copy().err(),
            Effect::SaveGif => match self.recorder.save() {
                Ok(()) => {
                    self.session.handle(self.now(), Event::Saved);
                    None
                }
                Err(e) => Some(e),
            },
            Effect::Quit => {
                self.quit = true;
                None
            }
        };
        if let Some(e) = failed {
            let now = self.now();
            for effect in self.session.handle(now, Event::Failed(e.0)) {
                self.apply(effect);
            }
        }
    }

    fn capture_request(&self) -> Result<CaptureRequest, inb::platform::PlatformError> {
        let monitor = self.monitor.as_ref().ok_or_else(|| inb::platform::PlatformError::new("No display found"))?;
        let area = Rect::new(self.area.x - self.screen.x, self.area.y - self.screen.y, self.area.w, self.area.h);
        Ok(CaptureRequest { display: capture::display_id(monitor), area, scale: self.scale, fps: self.session.fps })
    }

    fn pointer_moved(&mut self, target: Target, x: f64, y: f64) {
        self.cursor = Some((target, x, y));
        if let Some(drag) = &self.drag {
            let next = geometry::drag(drag.start_area, drag.grab, x - drag.start_x, y - drag.start_y, self.screen);
            if next != self.area {
                self.area = next;
                self.place_and_redraw();
            }
            return;
        }
        match target {
            Target::Edge(_) => {
                let icon = match geometry::grab_at(self.area, x, y) {
                    Some(Grab::Resize(c)) if !self.session.frame_locked() => corner_icon(c),
                    Some(_) if !self.session.frame_locked() => CursorIcon::Move,
                    _ => CursorIcon::Default,
                };
                self.windows[&target].set_cursor(icon);
            }
            Target::Menu => {
                let menu = self.menu_rect();
                let hovered = bar::menu_items(menu.w)
                    .into_iter()
                    .find(|(_, r)| r.contains(x - menu.x, y - menu.y))
                    .map(|(fps, _)| fps);
                if hovered != self.hovered_fps {
                    self.hovered_fps = hovered;
                    self.place_and_redraw();
                }
            }
            _ => {}
        }
    }

    fn pressed(&mut self) {
        let Some((target, x, y)) = self.cursor else { return };
        match target {
            Target::Edge(_) if !self.session.frame_locked() => {
                if let Some(grab) = geometry::grab_at(self.area, x, y) {
                    self.drag = Some(Drag { grab, start_area: self.area, start_x: x, start_y: y });
                }
            }
            Target::Bar => {
                let b = self.layout().bar;
                if let Some(event) = bar::hit(&self.session, x - b.x, y - b.y).and_then(control_event) {
                    self.send(event);
                }
            }
            Target::Menu => {
                if let Some(fps) = self.hovered_fps {
                    self.send(Event::PickFps(fps));
                }
            }
            _ => {}
        }
    }
}

fn control_event(control: Control) -> Option<Event> {
    Some(match control {
        Control::FpsChip => Event::ToggleMenu,
        Control::Record => Event::Record,
        Control::Close => Event::Close,
        Control::CancelQuit => Event::CancelQuit,
        Control::ConfirmQuit => Event::ConfirmQuit,
        Control::Stop => Event::Stop,
        Control::Save => Event::Save,
        Control::Dismiss => Event::Dismiss,
        Control::Size | Control::QuitQuestion | Control::Time | Control::Status => return None,
    })
}

fn corner_icon(corner: geometry::Corner) -> CursorIcon {
    match corner {
        geometry::Corner::TopLeft | geometry::Corner::BottomRight => CursorIcon::NwseResize,
        geometry::Corner::TopRight | geometry::Corner::BottomLeft => CursorIcon::NeswResize,
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windows.is_empty() {
            self.create_windows(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(&target) = self.targets.get(&id) else { return };
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                // Desktop points: the window's own origin plus the cursor's offset in it.
                let rect = self.rect_of(target);
                let p = position.to_logical::<f64>(self.scale);
                self.pointer_moved(target, rect.x + p.x, rect.y + p.y);
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => self.pressed(),
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.drag = None;
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Progress(p) => self.send(Event::EncodeProgress(p)),
            UserEvent::Encoded(Ok(bytes)) => self.send(Event::Encoded { bytes }),
            UserEvent::Encoded(Err(e)) => self.send(Event::Failed(e)),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.quit {
            event_loop.exit();
            return;
        }
        let now = self.now();
        let animating = self.session.animating(now);
        if animating {
            self.send(Event::Tick);
        }
        let frame = if animating { Duration::from_millis(16) } else { Duration::from_millis(250) };
        let recording_or_busy = self.session.frame_locked();
        event_loop.set_control_flow(if animating || recording_or_busy {
            ControlFlow::WaitUntil(Instant::now() + frame)
        } else {
            ControlFlow::Wait
        });
        if recording_or_busy && !animating {
            self.place_and_redraw();
        }
    }
}
