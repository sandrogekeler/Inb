//! The only code that names an operating system. Each system has the same
//! four modules with the same functions; `app` calls them through `sys`.

#[cfg(target_os = "macos")]
pub mod mac;
#[cfg(target_os = "macos")]
pub use mac as sys;

#[cfg(windows)]
pub mod win;
#[cfg(windows)]
pub use win as sys;

/// A failure the bar can show. The text is a short sentence a user reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformError(pub String);

impl PlatformError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a window is for, so each system can set it up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// One of the four frame edges: catches the mouse, never takes focus.
    Edge,
    /// A dim overlay: lets every click through to the apps below.
    Dim,
    Bar,
    Menu,
}

/// One capture, as `capture::start` takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureRequest {
    /// From `capture::display_id`.
    pub display: u64,
    /// The recorded area in points, relative to the display's top-left corner.
    pub area: crate::geometry::Rect,
    /// Physical pixels per point on that display.
    pub scale: f64,
    pub fps: u32,
}
