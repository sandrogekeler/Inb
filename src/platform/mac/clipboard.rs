//! The clipboard: one item holding the GIF as a file URL (what Finder, Slack
//! and Mail paste) and as GIF data (what apps that read images paste).

use std::path::Path;

use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeFileURL, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString, NSURL};

use crate::platform::PlatformError;

/// The uniform type identifier of GIF data.
const GIF_TYPE: &str = "com.compuserve.gif";

/// Puts the GIF file at `path` on the clipboard so that pasting it into a chat
/// or a folder pastes the animated file, not a still of its first frame.
pub fn copy_gif(path: &Path) -> Result<(), PlatformError> {
    let fail = || PlatformError::new("Could not copy the GIF");
    let path = std::path::absolute(path).map_err(|_| fail())?;
    let bytes = std::fs::read(&path).map_err(|_| fail())?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let url_text = url.absoluteString().ok_or_else(fail)?;

    let item = NSPasteboardItem::new();
    // SAFETY: a constant string AppKit provides.
    let file_type = unsafe { NSPasteboardTypeFileURL };
    let url_set = item.setString_forType(&url_text, file_type);
    let data_set = item.setData_forType(&NSData::with_bytes(&bytes), &NSString::from_str(GIF_TYPE));
    if !url_set || !data_set {
        return Err(fail());
    }

    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    let writer: &ProtocolObject<dyn NSPasteboardWriting> = ProtocolObject::from_ref(&*item);
    if pasteboard.writeObjects(&NSArray::from_slice(&[writer])) { Ok(()) } else { Err(fail()) }
}
