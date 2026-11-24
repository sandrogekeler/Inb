//! shortcut: placeholder until #14 lands.

use std::path::PathBuf;

use crate::platform::PlatformError;

/// The bytes of the system's UI font, or `None` if it cannot be read.
pub fn ui_font() -> Option<Vec<u8>> {
    None
}

/// The folder Save copies GIFs into.
pub fn save_dir() -> Result<PathBuf, PlatformError> {
    Err(PlatformError::new("Saving is not built yet"))
}
