//! The clipboard: the GIF as a file (what Explorer, Slack and mail paste) and
//! as raw "GIF" bytes (what apps that read images paste).

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_HDROP;
use windows::core::w;

use crate::platform::PlatformError;

/// Size of the `DROPFILES` header: `pFiles` (4), `pt` (8), `fNC` (4), `fWide` (4).
const DROPFILES_SIZE: u32 = 20;

/// Another app can hold the clipboard for a moment; try this many times.
const OPEN_TRIES: u32 = 5;

/// Puts the GIF file at `path` on the clipboard so that pasting it into a chat
/// or a folder pastes the animated file, not a still of its first frame.
pub fn copy_gif(path: &Path) -> Result<(), PlatformError> {
    let fail = || PlatformError::new("Could not copy the GIF");
    let path = std::path::absolute(path).map_err(|_| fail())?;
    let bytes = std::fs::read(&path).map_err(|_| fail())?;
    let list = drop_files(&path);

    let _clipboard = Clipboard::open().ok_or_else(fail)?;
    // SAFETY: the clipboard is open, and this thread opened it.
    unsafe { EmptyClipboard() }.map_err(|_| fail())?;
    put(u32::from(CF_HDROP.0), &list).ok_or_else(fail)?;
    // SAFETY: a constant, null-terminated wide string.
    let gif_format = unsafe { RegisterClipboardFormatW(w!("GIF")) };
    // The file already pastes; apps that read the "GIF" format are a bonus.
    if gif_format != 0 {
        let _ = put(gif_format, &bytes);
    }
    Ok(())
}

/// A `DROPFILES` block for one file: the header, then the wide path and an
/// extra null that ends the list.
fn drop_files(path: &Path) -> Vec<u8> {
    let mut block = Vec::new();
    block.extend_from_slice(&DROPFILES_SIZE.to_le_bytes());
    block.extend_from_slice(&[0; 8]); // pt
    block.extend_from_slice(&[0; 4]); // fNC
    block.extend_from_slice(&1u32.to_le_bytes()); // fWide
    for unit in path.as_os_str().encode_wide().chain([0, 0]) {
        block.extend_from_slice(&unit.to_le_bytes());
    }
    block
}

/// The open clipboard, closed again when dropped.
struct Clipboard;

impl Clipboard {
    fn open() -> Option<Self> {
        for attempt in 0..OPEN_TRIES {
            // SAFETY: a null owner window is allowed; the guard closes it.
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Some(Self);
            }
            if attempt + 1 < OPEN_TRIES {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        None
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        // SAFETY: `open` succeeded on this thread.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Hands `bytes` to the open clipboard as `format`. The system owns the memory
/// after a successful `SetClipboardData`.
fn put(format: u32, bytes: &[u8]) -> Option<()> {
    // SAFETY: GlobalAlloc, GlobalLock and GlobalUnlock bracket one write of
    // `bytes.len()` bytes into memory allocated with that length.
    let memory = unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).ok()?;
        let target = GlobalLock(memory).cast::<u8>();
        if target.is_null() {
            free(memory);
            return None;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
        let _ = GlobalUnlock(memory);
        memory
    };
    // SAFETY: the clipboard is open and `memory` is a movable global block.
    if unsafe { SetClipboardData(format, Some(HANDLE(memory.0))) }.is_err() {
        free(memory);
        return None;
    }
    Some(())
}

fn free(memory: HGLOBAL) {
    // SAFETY: the block was never handed to the system. GlobalFree reports
    // success as an error value, so the result is ignored.
    let _ = unsafe { GlobalFree(Some(memory)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_files_ends_with_two_nulls() {
        let block = drop_files(Path::new("C:\\a.gif"));
        assert_eq!(&block[..4], &DROPFILES_SIZE.to_le_bytes());
        assert_eq!(block[16], 1);
        assert_eq!(block.len(), 20 + ("C:\\a.gif".len() + 2) * 2);
        assert_eq!(&block[block.len() - 4..], &[0, 0, 0, 0]);
    }
}
