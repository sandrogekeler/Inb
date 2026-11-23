//! The save folder and the UI font.

use std::path::PathBuf;

use objc2_foundation::{NSFileManager, NSSearchPathDirectory, NSSearchPathDomainMask};

use crate::platform::PlatformError;

/// The bytes of the system's UI font, or `None` if it cannot be read. SFNS.ttf
/// is a variable font; ab_glyph reads it at its default weight.
pub fn ui_font() -> Option<Vec<u8>> {
    std::fs::read("/System/Library/Fonts/SFNS.ttf").ok()
}

/// The folder Save copies GIFs into: the Desktop.
pub fn save_dir() -> Result<PathBuf, PlatformError> {
    let fail = || PlatformError::new("Could not find the Desktop folder");
    let urls = NSFileManager::defaultManager()
        .URLsForDirectory_inDomains(NSSearchPathDirectory::DesktopDirectory, NSSearchPathDomainMask::UserDomainMask);
    let path = urls.firstObject().and_then(|url| url.path()).ok_or_else(fail)?;
    Ok(PathBuf::from(path.to_string()))
}
