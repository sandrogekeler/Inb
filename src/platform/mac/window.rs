//! macOS windows: winit makes them, this module turns them into transparent,
//! click-through-where-clear overlays that float over every Space.
// aislop-ignore-file hallucinated-import

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Once;

use objc2::MainThreadMarker;
use objc2::runtime::{AnyObject, Bool, Imp, Sel};
use objc2::sel;
use objc2_app_kit::{
    NSColor, NSStatusWindowLevel, NSView, NSViewLayerContentsRedrawPolicy, NSWindow, NSWindowCollectionBehavior,
    NSWindowSharingType,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo, kCGColorSpaceSRGB,
};
use objc2_quartz_core::kCAGravityTopLeft;
use tiny_skia::Pixmap;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::platform::{PlatformError, Role};

/// Sets up a window Inb just created, before it is shown: above other apps,
/// no shadow, never in a screen capture, and click-through for `Role::Dim`.
pub fn prepare(window: &Window, role: Role) -> Result<(), PlatformError> {
    let view = content_view(window)?;
    let ns_window = view.window().ok_or_else(|| PlatformError::new("A window has no native window"))?;
    never_key_or_main(&ns_window);
    ns_window.setOpaque(false);
    ns_window.setBackgroundColor(Some(&NSColor::clearColor()));
    ns_window.setHasShadow(false);
    ns_window.setLevel(NSStatusWindowLevel);
    ns_window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary,
    );
    ns_window.setSharingType(NSWindowSharingType::None);
    ns_window.setIgnoresMouseEvents(role == Role::Dim);
    // winit's view is layer-backed and has a `drawRect:`, so AppKit would
    // replace the layer's contents on every resize. `Never` leaves them alone.
    view.setWantsLayer(true);
    view.setLayerContentsRedrawPolicy(NSViewLayerContentsRedrawPolicy::Never);
    Ok(())
}

/// Shows `pixmap` (premultiplied RGBA, the window's size in physical pixels)
/// in the window with per-pixel transparency.
pub fn present(window: &Window, pixmap: &Pixmap) -> Result<(), PlatformError> {
    let view = content_view(window)?;
    let layer = view.layer().ok_or_else(|| PlatformError::new("A window has no layer"))?;
    let image = image_from(pixmap)?;
    let scale = view.window().map_or(1.0, |w| w.backingScaleFactor());
    // The image keeps its own size and sticks to the top-left corner, so the
    // pixmap decides what is drawn even before winit has resized the window.
    // SAFETY: reading an AppKit constant that is set up before `main`.
    layer.setContentsGravity(unsafe { kCAGravityTopLeft });
    layer.setContentsScale(scale);
    let object = CFRetained::as_ptr(&image).cast::<AnyObject>();
    // SAFETY: a CGImage is the object a layer's `contents` takes (toll-free
    // bridged), and the layer retains it, so `image` can drop afterwards.
    unsafe { layer.setContents(Some(object.as_ref())) };
    Ok(())
}

/// winit's content view, from the window handle. Main thread only.
fn content_view(window: &Window) -> Result<&NSView, PlatformError> {
    if MainThreadMarker::new().is_none() {
        return Err(PlatformError::new("Windows can only change on the main thread"));
    }
    let handle = window.window_handle().map_err(|e| PlatformError::new(format!("No window handle: {e}")))?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err(PlatformError::new("A window is not an AppKit window"));
    };
    // SAFETY: winit hands out the pointer of the `NSView` it owns, which lives
    // as long as `window`, and we are on the main thread.
    Ok(unsafe { handle.ns_view.cast::<NSView>().as_ref() })
}

extern "C-unwind" fn answer_no(_this: &AnyObject, _cmd: Sel) -> Bool {
    Bool::NO
}

/// Inb's windows never take keyboard focus, so a click on the bar leaves the
/// recorded app in front. winit's window class says yes to both, and every
/// window in this process is one of Inb's, so its answers are replaced once.
fn never_key_or_main(window: &NSWindow) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let object: &AnyObject = window;
        for selector in [sel!(canBecomeKeyWindow), sel!(canBecomeMainWindow)] {
            let Some(method) = object.class().instance_method(selector) else { continue };
            let imp = answer_no as extern "C-unwind" fn(&AnyObject, Sel) -> Bool;
            // SAFETY: both selectors take no arguments and return BOOL, which
            // `answer_no` matches, and answering NO is always a valid answer.
            unsafe {
                method
                    .set_implementation(std::mem::transmute::<extern "C-unwind" fn(&AnyObject, Sel) -> Bool, Imp>(imp));
            }
        }
    });
}

/// Frees the pixel copy that `image_from` gave to a data provider.
unsafe extern "C-unwind" fn release_pixels(_info: *mut c_void, data: NonNull<c_void>, size: usize) {
    // SAFETY: `data` and `size` are the boxed slice leaked in `image_from`.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data.as_ptr().cast::<u8>(), size)) });
}

/// A `CGImage` of the pixmap, sRGB, premultiplied alpha. The pixels are
/// copied because the layer keeps the image after the pixmap is gone.
fn image_from(pixmap: &Pixmap) -> Result<CFRetained<CGImage>, PlatformError> {
    let (width, height) = (pixmap.width() as usize, pixmap.height() as usize);
    let pixels: Box<[u8]> = pixmap.data().into();
    let size = pixels.len();
    let data = Box::into_raw(pixels).cast::<c_void>();
    // SAFETY: `data` stays valid until `release_pixels` runs, which frees it.
    let provider = unsafe { CGDataProvider::with_data(std::ptr::null_mut(), data, size, Some(release_pixels)) };
    // SAFETY: reading an AppKit constant that is set up before `main`.
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }));
    let (Some(provider), Some(space)) = (provider, space) else {
        return Err(PlatformError::new("Could not make an image for a window"));
    };
    // SAFETY: `decode` is null, and `provider` holds `height` rows of
    // `width * 4` bytes, as the pixmap does.
    let image = unsafe {
        CGImage::new(
            width,
            height,
            8,
            32,
            width * 4,
            Some(&space),
            CGBitmapInfo::from_bits_retain(CGImageAlphaInfo::PremultipliedLast.0),
            Some(&provider),
            std::ptr::null(),
            false,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    };
    image.ok_or_else(|| PlatformError::new("Could not make an image for a window"))
}
