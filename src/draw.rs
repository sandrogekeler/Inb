//! Pictures of each Inb window, as premultiplied RGBA pixmaps at the window's
//! size in physical pixels. `platform::window::present` puts them on screen.
//! Everything is drawn in points and scaled by `scale`.

mod bar_art;
mod canvas;

use std::time::Duration;

use ab_glyph::FontVec;
use tiny_skia::{Color, Pixmap};

use self::canvas::{Canvas, Ink, rounded};
use crate::bar::menu_items;
use crate::geometry::{Rect, Side};
use crate::session::Session;
use crate::spec::*;

/// The pixel size of a window `rect` points large at `scale`.
pub fn pixel_size(rect: Rect, scale: f64) -> (u32, u32) {
    (((rect.w * scale).ceil() as u32).max(1), ((rect.h * scale).ceil() as u32).max(1))
}

fn color(c: Rgba) -> Color {
    Color::from_rgba8(c.0, c.1, c.2, c.3)
}

/// `a` mixed towards `b` by `t` (0 to 1).
fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let m = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t.clamp(0.0, 1.0)).round() as u8;
    Rgba(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2), 255)
}

/// The bar for the session's state. `area` is the recorded area in points, for
/// the size label.
pub fn bar(session: &Session, now: Duration, area: Rect, font: Option<&FontVec>, scale: f64) -> Pixmap {
    bar_art::render(session, now, area, font, scale)
}

/// The fps menu, `rect` points large, with `hovered` highlighted.
pub fn menu(rect: Rect, fps: u32, hovered: Option<u32>, font: Option<&FontVec>, scale: f64) -> Pixmap {
    let mut c = Canvas::new(pixel_size(rect, scale), scale, font);
    let full = Rect::new(0.0, 0.0, rect.w, rect.h);
    let radius = RADIUS + MENU_EXTRA_RADIUS;
    c.fill_rect(full, radius, BG);
    for (value, row) in menu_items(rect.w) {
        if hovered == Some(value) {
            c.fill_rect(row, RADIUS, HOVER);
        }
        let cy = row.y + row.h / 2.0;
        let current = value == fps;
        c.text(&format!("{value} fps"), row.x + MENU_TEXT_PAD, cy, Ink::new(if current { FG } else { MUTED }));
        if current {
            bar_art::check(&mut c, row.right() - MENU_TEXT_PAD - CHECK_W, cy, FG);
        }
    }
    c.outline(full, radius, OUTLINE, EDGE);
    c.pm
}

/// One edge window, `rect` points large. `red` is `Session::red`; corner
/// handles are drawn while `handles` is true.
///
/// Every window draws the same frame ring and keeps its own part, so the four
/// meet without seams. The window reaches `HIT` points out from the area; only
/// the `BAND` nearest the area is painted, and a one-point outline around its
/// outer side. Every other pixel has alpha 1 so it still catches the mouse.
pub fn edge(side: Side, rect: Rect, red: f32, handles: bool, scale: f64) -> Pixmap {
    let mut c = Canvas::new(pixel_size(rect, scale), scale, None);
    c.pm.fill(Color::from_rgba8(0, 0, 0, 1));
    // The area in window coordinates. A side the window does not show is
    // placed far away: nothing of it reaches the window.
    let far = 4.0 * HIT;
    let area = match side {
        Side::Top => Rect::new(HIT, HIT, rect.w - 2.0 * HIT, far),
        Side::Bottom => Rect::new(HIT, -far, rect.w - 2.0 * HIT, far),
        Side::Left => Rect::new(HIT, 0.0, far, rect.h),
        Side::Right => Rect::new(-far, 0.0, far, rect.h),
    };
    let hole = rounded(area, [0.0; 4]);
    let band = rounded(area.outset(BAND), [RADIUS; 4]);
    let outline = rounded(area.outset(BAND + OUTLINE), [RADIUS + OUTLINE; 4]);
    c.fill_ring(&outline, &band, EDGE);
    c.fill_ring(&band, &hole, mix(BG, REC_BG, red));
    if handles && matches!(side, Side::Top | Side::Bottom) {
        let y = if side == Side::Top { area.y - HANDLE } else { area.bottom() };
        for x in [area.x - HANDLE, area.right()] {
            c.fill_rect(Rect::new(x, y, HANDLE, HANDLE), 0.0, FG);
        }
    }
    c.pm
}

/// One dim overlay, `rect` points large.
pub fn dim(rect: Rect, scale: f64) -> Pixmap {
    let mut p = Pixmap::new(pixel_size(rect, scale).0, pixel_size(rect, scale).1).expect("window sizes are never zero");
    p.fill(Color::from_rgba(0.0, 0.0, 0.0, DIM_ALPHA).expect("valid alpha"));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let c = p.pixel(x, y).expect("inside the pixmap");
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    #[test]
    fn pixmaps_match_the_window_size_in_pixels() {
        let r = Rect::new(0.0, 0.0, 10.5, 3.0);
        assert_eq!(pixel_size(r, 2.0), (21, 6));
        let p = edge(Side::Top, r, 0.0, true, 1.5);
        assert_eq!((p.width(), p.height()), (16, 5));
        let session = Session::default();
        let area = Rect::new(0.0, 0.0, 640.0, 400.0);
        assert_eq!(bar(&session, Duration::ZERO, area, None, 2.0).width(), 448);
        assert_eq!(menu(Rect::new(0.0, 0.0, 84.0, 176.0), 15, None, None, 2.0).height(), 352);
    }

    #[test]
    fn edge_band_is_opaque_and_the_margin_catches_the_mouse() {
        let r = Rect::new(0.0, 0.0, 220.0, HIT);
        for scale in [1.0, 2.0] {
            let p = edge(Side::Top, r, 0.0, false, scale);
            let s = scale as u32;
            let band_row = (HIT as u32 - 1) * s;
            assert_eq!(px(&p, 100 * s, band_row), [0x1c, 0x1c, 0x1e, 255]);
            assert_eq!(px(&p, 100 * s, 0), [0, 0, 0, 1]);
            // The outline sits on the outer side of the band only.
            let outline_row = (HIT as u32 - BAND as u32 - 1) * s;
            // 140 over the 1/255 backdrop.
            assert!((140..=141).contains(&px(&p, 100 * s, outline_row)[3]));
        }
    }

    #[test]
    fn red_mixes_the_band_towards_rec_bg() {
        let r = Rect::new(0.0, 0.0, 10.0, 100.0);
        let p = edge(Side::Left, r, 1.0, false, 1.0);
        assert_eq!(px(&p, 9, 50), [0xe5, 0x34, 0x2b, 255]);
    }

    #[test]
    fn handles_sit_outside_the_area_on_top_and_bottom() {
        let r = Rect::new(0.0, 0.0, 220.0, HIT);
        let with = edge(Side::Top, r, 0.0, true, 1.0);
        assert_eq!(px(&with, 7, 7), [0xf5, 0xf5, 0xf7, 255]);
        assert_ne!(px(&edge(Side::Top, r, 0.0, false, 1.0), 7, 7), px(&with, 7, 7));
    }
}
