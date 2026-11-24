//! shortcut: placeholder until #14 lands.

use std::path::Path;

use crate::platform::PlatformError;

/// Puts the GIF file at `path` on the clipboard so that pasting it into a chat
/// or a folder pastes the animated file, not a still of its first frame.
pub fn copy_gif(path: &Path) -> Result<(), PlatformError> {
    let _ = path;
    Err(PlatformError::new("Copying is not built yet"))
}
