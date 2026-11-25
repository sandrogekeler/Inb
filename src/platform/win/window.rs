//! Inb's windows on Windows: layered windows that show a pixmap with
//! per-pixel alpha, never take focus and never appear in a capture.

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;
use std::slice;

use tiny_skia::Pixmap;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_NCRENDERING_POLICY, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_DONOTROUND, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HDC, HGDIOBJ, ReleaseDC, SelectObject,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowDisplayAffinity, SetWindowLongPtrW, SetWindowPos, ULW_ALPHA, UpdateLayeredWindow,
    WDA_EXCLUDEFROMCAPTURE, WINDOWPOS, WM_NCDESTROY, WM_STYLECHANGING, WM_WINDOWPOSCHANGING, WS_EX_APPWINDOW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::platform::{PlatformError, Role};

/// The id of Inb's subclass on a window.
const SUBCLASS_ID: usize = 1;

/// The extended styles Inb decides for every window. winit rewrites all
/// extended styles whenever a window is shown or hidden, so the subclass puts
/// these back each time.
const OWNED: u32 = WS_EX_NOACTIVATE.0 | WS_EX_LAYERED.0 | WS_EX_TOOLWINDOW.0 | WS_EX_APPWINDOW.0 | WS_EX_TRANSPARENT.0;

/// Sets up a window Inb just created, before it is shown: above other apps,
/// no shadow, never in a screen capture, and click-through for `Role::Dim`.
pub fn prepare(window: &Window, role: Role) -> Result<(), PlatformError> {
    let hwnd = hwnd(window)?;
    let wanted = wanted_styles(role);
    // SAFETY: `hwnd` is the live window `window` owns, and this runs on the
    // thread that created it, as `SetWindowSubclass` requires.
    unsafe {
        SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, wanted as usize)
            .ok()
            .map_err(|e| fail("Could not set up the windows", e))?;
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, with_styles(wanted, current) as isize);
        let refresh = SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED;
        SetWindowPos(hwnd, None, 0, 0, 0, 0, refresh).map_err(|e| fail("Could not set up the windows", e))?;
        SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
            .map_err(|e| fail("Could not hide the windows from the recording", e))?;
    }
    remove_decoration(hwnd);
    Ok(())
}

/// Shows `pixmap` (premultiplied RGBA, the window's size in physical pixels)
/// in the window with per-pixel transparency.
pub fn present(window: &Window, pixmap: &Pixmap) -> Result<(), PlatformError> {
    let hwnd = hwnd(window)?;
    // SAFETY: the two device contexts are created and freed in this block and
    // `show_pixmap` only uses them while they exist.
    unsafe {
        let screen = GetDC(None);
        let memory = CreateCompatibleDC(Some(screen));
        let result = show_pixmap(hwnd, screen, memory, pixmap);
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        result
    }
}

/// Copies `pixmap` into a top-down 32-bit DIB section and hands it to the window.
///
/// # Safety
/// `screen` and `memory` are valid device contexts, and `hwnd` is a layered window.
unsafe fn show_pixmap(hwnd: HWND, screen: HDC, memory: HDC, pixmap: &Pixmap) -> Result<(), PlatformError> {
    let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = null_mut();
    // SAFETY: `info` describes a plain 32-bit RGB bitmap and `bits` receives its pixel memory.
    let dib = unsafe { CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0) }
        .map_err(|e| fail("Could not draw the windows", e))?;
    // SAFETY: the DIB section owns `width * height * 4` bytes at `bits` until it is deleted below.
    let pixels = unsafe { slice::from_raw_parts_mut(bits.cast::<u8>(), pixmap.data().len()) };
    to_bgra(pixmap.data(), pixels);
    let size = SIZE { cx: width, cy: height };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    // SAFETY: `memory` is a valid device context and `dib` a bitmap nothing else uses; it is
    // selected out again and deleted before the function returns. The position is not passed,
    // so the window stays where winit put it and only takes the pixmap's size.
    let shown = unsafe {
        let old = SelectObject(memory, HGDIOBJ::from(dib));
        let shown = UpdateLayeredWindow(
            hwnd,
            Some(screen),
            None,
            Some(&size),
            Some(memory),
            Some(&POINT { x: 0, y: 0 }),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        SelectObject(memory, old);
        let _ = DeleteObject(HGDIOBJ::from(dib));
        shown
    };
    shown.map_err(|e| fail("Could not draw the windows", e))
}

/// Premultiplied RGBA to the premultiplied BGRA that GDI reads.
fn to_bgra(rgba: &[u8], bgra: &mut [u8]) {
    for (to, from) in bgra.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        to.copy_from_slice(&[from[2], from[1], from[0], from[3]]);
    }
}

/// No rounded corners, border or frame shadow. Windows 10 rejects the first
/// two attributes and has nothing to remove, so the results are not checked.
fn remove_decoration(hwnd: HWND) {
    let corners = DWMWCP_DONOTROUND;
    let border = DWMWA_COLOR_NONE;
    let frame = DWMNCRP_DISABLED;
    // SAFETY: each pointer is to a value that lives through its call, with the size the
    // attribute documents (a 4-byte enum or colour).
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, (&raw const corners).cast(), 4);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, (&raw const border).cast(), 4);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_NCRENDERING_POLICY, (&raw const frame).cast(), 4);
    }
}

/// The owned extended styles a role needs. Every window is layered and never
/// activates; only the bar has a taskbar button.
fn wanted_styles(role: Role) -> u32 {
    let base = WS_EX_NOACTIVATE.0 | WS_EX_LAYERED.0;
    match role {
        Role::Bar => base | WS_EX_APPWINDOW.0,
        Role::Dim => base | WS_EX_TOOLWINDOW.0 | WS_EX_TRANSPARENT.0,
        Role::Edge | Role::Menu => base | WS_EX_TOOLWINDOW.0,
    }
}

fn with_styles(wanted: u32, current: u32) -> u32 {
    (current & !OWNED) | wanted
}

/// Runs before winit's own window procedure. `wanted` is the owned styles.
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    wanted: usize,
) -> LRESULT {
    match message {
        // wParam is GWL_EXSTYLE (a negative number) or GWL_STYLE.
        WM_STYLECHANGING if wparam.0 as i32 == GWL_EXSTYLE.0 => {
            // SAFETY: for this message lParam points to a STYLESTRUCT the system lets us edit.
            let styles = unsafe { &mut *(lparam.0 as *mut STYLESTRUCT) };
            styles.styleNew = with_styles(wanted as u32, styles.styleNew);
        }
        WM_WINDOWPOSCHANGING => {
            // SAFETY: for this message lParam points to a WINDOWPOS the system lets us edit.
            let position = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
            position.flags.0 |= SWP_NOACTIVATE.0;
        }
        WM_NCDESTROY => {
            // SAFETY: removes the subclass this module set on the window that is being destroyed.
            let _ = unsafe { RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID) };
        }
        _ => {}
    }
    // SAFETY: forwards the message unchanged to the next procedure in the chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn hwnd(window: &Window) -> Result<HWND, PlatformError> {
    let handle = window.window_handle().map_err(|_| PlatformError::new("A window is not available"))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut c_void)),
        _ => Err(PlatformError::new("A window is not available")),
    }
}

fn fail(what: &str, error: windows::core::Error) -> PlatformError {
    PlatformError::new(format!("{what}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dim_clicks_pass_through_and_hide_from_taskbar() {
        let styles = with_styles(wanted_styles(Role::Dim), WS_EX_APPWINDOW.0);
        assert_eq!(styles & WS_EX_TRANSPARENT.0, WS_EX_TRANSPARENT.0);
        assert_eq!(styles & WS_EX_APPWINDOW.0, 0);
    }

    #[test]
    fn only_the_bar_has_a_taskbar_button() {
        let bar = with_styles(wanted_styles(Role::Bar), WS_EX_TOOLWINDOW.0);
        assert_eq!(bar & WS_EX_APPWINDOW.0, WS_EX_APPWINDOW.0);
        assert_eq!(bar & WS_EX_TRANSPARENT.0, 0);
        let menu = with_styles(wanted_styles(Role::Menu), WS_EX_APPWINDOW.0);
        assert_eq!(menu & WS_EX_APPWINDOW.0, 0);
    }

    #[test]
    fn other_styles_are_kept() {
        let topmost = 0x8;
        assert_eq!(with_styles(wanted_styles(Role::Edge), topmost) & topmost, topmost);
    }

    #[test]
    fn pixels_swap_red_and_blue() {
        let mut out = [0; 4];
        to_bgra(&[1, 2, 3, 4], &mut out);
        assert_eq!(out, [3, 2, 1, 4]);
    }
}
