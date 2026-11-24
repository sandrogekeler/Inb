//! shortcut: placeholder, windows show nothing until #9 (Windows) and #10 (macOS) land.

use tiny_skia::Pixmap;
use winit::window::Window;

use crate::platform::{PlatformError, Role};

/// Sets up a window Inb just created, before it is shown: above other apps,
/// no shadow, never in a screen capture, and click-through for `Role::Dim`.
pub fn prepare(window: &Window, role: Role) -> Result<(), PlatformError> {
    let _ = (window, role);
    Ok(())
}

/// Shows `pixmap` (premultiplied RGBA, the window's size in physical pixels)
/// in the window with per-pixel transparency.
pub fn present(window: &Window, pixmap: &Pixmap) -> Result<(), PlatformError> {
    let _ = (window, pixmap);
    Ok(())
}
