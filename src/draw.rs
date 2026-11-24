//! Pictures of each Inb window, as premultiplied RGBA pixmaps at the window's
//! size in physical pixels. `platform::window::present` puts them on screen.
//!
//! shortcut: these are flat placeholders (no text, icons or animation), the
//! real drawing is #8.

use std::time::Duration;

use ab_glyph::FontVec;
use tiny_skia::{Color, Pixmap};

use crate::geometry::{Rect, Side};
use crate::session::Session;
use crate::spec::{BG, DIM_ALPHA, REC_BG, Rgba};

/// The pixel size of a window `rect` points large at `scale`.
pub fn pixel_size(rect: Rect, scale: f64) -> (u32, u32) {
    (((rect.w * scale).ceil() as u32).max(1), ((rect.h * scale).ceil() as u32).max(1))
}

fn color(c: Rgba) -> Color {
    Color::from_rgba8(c.0, c.1, c.2, c.3)
}

fn filled(size: (u32, u32), c: Color) -> Pixmap {
    let mut p = Pixmap::new(size.0, size.1).expect("window sizes are never zero");
    p.fill(c);
    p
}

/// The bar for the session's state. `area` is the recorded area in points, for
/// the size label.
pub fn bar(session: &Session, now: Duration, area: Rect, font: Option<&FontVec>, scale: f64) -> Pixmap {
    let _ = (session, now, area, font);
    let size = pixel_size(Rect::new(0.0, 0.0, crate::spec::BAR_W, crate::spec::BAR_H), scale);
    filled(size, color(BG))
}

/// The fps menu, `rect` points large, with `hovered` highlighted.
pub fn menu(rect: Rect, fps: u32, hovered: Option<u32>, font: Option<&FontVec>, scale: f64) -> Pixmap {
    let _ = (fps, hovered, font);
    filled(pixel_size(rect, scale), color(BG))
}

/// One edge window, `rect` points large. `red` is `Session::red`; corner
/// handles are drawn while `handles` is true.
pub fn edge(side: Side, rect: Rect, red: f32, handles: bool, scale: f64) -> Pixmap {
    let _ = (side, handles);
    let band = if red > 0.5 { REC_BG } else { BG };
    filled(pixel_size(rect, scale), color(band))
}

/// One dim overlay, `rect` points large.
pub fn dim(rect: Rect, scale: f64) -> Pixmap {
    filled(pixel_size(rect, scale), Color::from_rgba(0.0, 0.0, 0.0, DIM_ALPHA).expect("valid alpha"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixmaps_match_the_window_size_in_pixels() {
        let r = Rect::new(0.0, 0.0, 10.5, 3.0);
        assert_eq!(pixel_size(r, 2.0), (21, 6));
        let p = edge(Side::Top, r, 0.0, true, 1.5);
        assert_eq!((p.width(), p.height()), (16, 5));
    }
}
