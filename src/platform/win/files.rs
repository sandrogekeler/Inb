//! The save folder and the UI font.

use std::path::PathBuf;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

use crate::platform::PlatformError;

/// The bytes of the system's UI font, or `None` if it cannot be read.
pub fn ui_font() -> Option<Vec<u8>> {
    let windows = std::env::var_os("WINDIR")?;
    std::fs::read(PathBuf::from(windows).join("Fonts").join("segoeui.ttf")).ok()
}

/// The folder Save copies GIFs into: the Desktop, which OneDrive may have moved.
pub fn save_dir() -> Result<PathBuf, PlatformError> {
    let fail = || PlatformError::new("Could not find the Desktop folder");
    // SAFETY: the folder id is a constant and no token means the current user.
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None) }.map_err(|_| fail())?;
    // SAFETY: `path` is a valid null-terminated string that the call allocated.
    let text = unsafe { path.to_string() };
    // SAFETY: the caller owns the string SHGetKnownFolderPath allocated and frees it once.
    unsafe { CoTaskMemFree(Some(path.0.cast_const().cast())) };
    text.map(PathBuf::from).map_err(|_| fail())
}
