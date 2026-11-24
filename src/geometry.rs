//! Where every Inb window goes, in logical points in desktop coordinates, and
//! how a drag on the frame moves or resizes the area.

use crate::spec::{
    BAND, BAR_H, BAR_W, CORNER_ZONE, GAP, HIT, MENU_ITEM_H, MENU_PAD, MIN_H, MIN_W, SCREEN_MARGIN, START_H, START_W,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// The rect grown by `d` on every side.
    pub fn outset(&self, d: f64) -> Self {
        Self::new(self.x - d, self.y - d, self.w + 2.0 * d, self.h + 2.0 * d)
    }
}

/// The four edge windows, in this order everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

pub const SIDES: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// What a press on the frame starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grab {
    Move,
    Resize(Corner),
}

/// Every window rect for one area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Edge windows by `SIDES`. Top and bottom span the corners.
    pub edges: [Rect; 4],
    /// Dim overlays by `SIDES`: together they cover the display minus the area.
    pub dims: [Rect; 4],
    pub bar: Rect,
    /// True when the bar sits below the frame, false when it flipped above.
    pub bar_below: bool,
}

/// The area Inb opens with: `START_W` by `START_H`, centred on the display.
pub fn start_area(screen: Rect) -> Rect {
    let w = START_W.min(screen.w - 2.0 * HIT);
    let h = START_H.min(screen.h - room_for_bar());
    Rect::new((screen.x + (screen.w - w) / 2.0).round(), (screen.y + (screen.h - h) / 2.0).round(), w, h)
}

/// The height the bar needs beside the frame, so the area never grows into it.
fn room_for_bar() -> f64 {
    2.0 * BAND + GAP + BAR_H + SCREEN_MARGIN
}

pub fn layout(area: Rect, screen: Rect) -> Layout {
    let outer = area.outset(HIT);
    let edges = [
        Rect::new(outer.x, outer.y, outer.w, HIT),
        Rect::new(outer.x, area.bottom(), outer.w, HIT),
        Rect::new(outer.x, area.y, HIT, area.h),
        Rect::new(area.right(), area.y, HIT, area.h),
    ];
    let dims = [
        Rect::new(screen.x, screen.y, screen.w, area.y - screen.y),
        Rect::new(screen.x, area.bottom(), screen.w, screen.bottom() - area.bottom()),
        Rect::new(screen.x, area.y, area.x - screen.x, area.h),
        Rect::new(area.right(), area.y, screen.right() - area.right(), area.h),
    ];
    let bar_x = area.right() + BAND - BAR_W;
    let below_y = area.bottom() + BAND + GAP;
    let bar_below = below_y + BAR_H <= screen.bottom() - SCREEN_MARGIN;
    let bar_y = if bar_below { below_y } else { area.y - BAND - GAP - BAR_H };
    Layout { edges, dims, bar: Rect::new(bar_x, bar_y, BAR_W, BAR_H), bar_below }
}

/// The fps menu window: it opens away from the frame when there is room, and
/// towards it otherwise. `chip` is the fps chip in desktop coordinates.
pub fn menu(chip: Rect, item_count: usize, bar_below: bool, screen: Rect) -> Rect {
    let h = 2.0 * MENU_PAD + MENU_ITEM_H * item_count as f64;
    let w = chip.w.max(crate::spec::MENU_ITEM_W + 2.0 * MENU_PAD);
    let below = Rect::new(chip.x, chip.bottom() + MENU_PAD, w, h);
    let above = Rect::new(chip.x, chip.y - MENU_PAD - h, w, h);
    let fits_below = below.bottom() <= screen.bottom() - SCREEN_MARGIN;
    let fits_above = above.y >= screen.y;
    if (bar_below && fits_below) || (!bar_below && !fits_above) { below } else { above }
}

/// What a press at `(x, y)` on the frame does, or `None` off the frame.
pub fn grab_at(area: Rect, x: f64, y: f64) -> Option<Grab> {
    let outer = area.outset(HIT);
    if !outer.contains(x, y) || area.contains(x, y) {
        return None;
    }
    let near = |a: f64, b: f64| (a - b).abs() <= CORNER_ZONE;
    let corner = match (near(x, area.x), near(x, area.right()), near(y, area.y), near(y, area.bottom())) {
        (true, _, true, _) => Some(Corner::TopLeft),
        (_, true, true, _) => Some(Corner::TopRight),
        (true, _, _, true) => Some(Corner::BottomLeft),
        (_, true, _, true) => Some(Corner::BottomRight),
        _ => None,
    };
    Some(corner.map_or(Grab::Move, Grab::Resize))
}

/// The area after dragging `grab` by `(dx, dy)` from `start`. The area stays
/// on the display with room for the bar and never shrinks below the minimum.
pub fn drag(start: Rect, grab: Grab, dx: f64, dy: f64, screen: Rect) -> Rect {
    let left = screen.x + BAND;
    let top = screen.y + BAND;
    let right = screen.right() - BAND;
    let bottom = screen.bottom() - BAND;
    let max_h = screen.h - room_for_bar();
    let (mut x, mut y, mut w, mut h) = (start.x, start.y, start.w, start.h.min(max_h));
    match grab {
        Grab::Move => {
            x = (start.x + dx).clamp(left, right - w);
            y = (start.y + dy).clamp(top, bottom - h);
        }
        Grab::Resize(corner) => {
            let (west, north) = match corner {
                Corner::TopLeft => (true, true),
                Corner::TopRight => (false, true),
                Corner::BottomLeft => (true, false),
                Corner::BottomRight => (false, false),
            };
            if west {
                x = (start.x + dx).clamp(left, start.right() - MIN_W);
                w = start.right() - x;
            } else {
                w = (start.w + dx).clamp(MIN_W, right - start.x);
            }
            if north {
                y = (start.y + dy).clamp(top.max(start.bottom() - max_h), start.bottom() - MIN_H);
                h = start.bottom() - y;
            } else {
                h = (start.h + dy).clamp(MIN_H, (bottom - start.y).min(max_h));
            }
        }
    }
    Rect::new(x.round(), y.round(), w.round(), h.round())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect::new(0.0, 0.0, 1280.0, 800.0);

    #[test]
    fn start_area_is_centred() {
        assert_eq!(start_area(SCREEN), Rect::new(320.0, 200.0, 640.0, 400.0));
    }

    #[test]
    fn layout_keeps_every_window_outside_the_area() {
        let area = start_area(SCREEN);
        let l = layout(area, SCREEN);
        for r in l.edges.iter().chain(l.dims.iter()).chain([&l.bar]) {
            let overlaps = r.x < area.right() && r.right() > area.x && r.y < area.bottom() && r.bottom() > area.y;
            assert!(!overlaps, "{r:?} overlaps {area:?}");
        }
    }

    #[test]
    fn bar_sits_below_and_right_aligned() {
        let l = layout(start_area(SCREEN), SCREEN);
        assert!(l.bar_below);
        assert_eq!(l.bar, Rect::new(960.0 + BAND - BAR_W, 600.0 + BAND + GAP, BAR_W, BAR_H));
    }

    #[test]
    fn bar_flips_above_near_the_bottom() {
        let area = Rect::new(100.0, 400.0, 640.0, 380.0);
        let l = layout(area, SCREEN);
        assert!(!l.bar_below);
        assert_eq!(l.bar.bottom(), area.y - BAND - GAP);
    }

    #[test]
    fn grab_picks_corner_edge_or_nothing() {
        let a = Rect::new(100.0, 100.0, 400.0, 300.0);
        assert_eq!(grab_at(a, 95.0, 95.0), Some(Grab::Resize(Corner::TopLeft)));
        assert_eq!(grab_at(a, 505.0, 405.0), Some(Grab::Resize(Corner::BottomRight)));
        assert_eq!(grab_at(a, 300.0, 95.0), Some(Grab::Move));
        assert_eq!(grab_at(a, 300.0, 200.0), None);
        assert_eq!(grab_at(a, 300.0, 50.0), None);
    }

    #[test]
    fn move_stays_on_screen() {
        let a = Rect::new(100.0, 100.0, 400.0, 300.0);
        let moved = drag(a, Grab::Move, -500.0, 0.0, SCREEN);
        assert_eq!(moved.x, BAND);
        assert_eq!(moved.w, 400.0);
    }

    #[test]
    fn resize_respects_minimum() {
        let a = Rect::new(100.0, 100.0, 400.0, 300.0);
        let r = drag(a, Grab::Resize(Corner::TopLeft), 1000.0, 1000.0, SCREEN);
        assert_eq!((r.w, r.h), (MIN_W, MIN_H));
        assert_eq!((r.right(), r.bottom()), (a.right(), a.bottom()));
    }

    #[test]
    fn resize_leaves_room_for_the_bar() {
        let a = Rect::new(100.0, 100.0, 400.0, 300.0);
        let r = drag(a, Grab::Resize(Corner::BottomRight), 0.0, 2000.0, SCREEN);
        let l = layout(r, SCREEN);
        assert!(l.bar.y >= SCREEN.y && l.bar.bottom() <= SCREEN.bottom());
    }

    #[test]
    fn menu_opens_away_from_the_frame() {
        let chip = Rect::new(500.0, 400.0, 68.0, 28.0);
        assert!(menu(chip, 6, true, SCREEN).y > chip.y);
        assert!(menu(chip, 6, false, SCREEN).y < chip.y);
    }
}
