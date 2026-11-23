//! Where each control sits on the bar and in the fps menu, in points relative
//! to the bar's or the menu's top-left corner. Drawing and hit testing both
//! read this, so a click always lands on what was drawn.

use crate::geometry::Rect;
use crate::session::{Mode, Session};
use crate::spec::{BAR_H, BAR_PAD_LEFT, BAR_PAD_RIGHT, BAR_SPACING, BAR_W, BUTTON, FPS_OPTIONS, MENU_ITEM_H, MENU_PAD};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// "640 × 400": the area's size. Not clickable.
    Size,
    FpsChip,
    Record,
    Close,
    /// "Quit Inb?". Not clickable.
    QuitQuestion,
    CancelQuit,
    ConfirmQuit,
    /// The time while recording. Not clickable.
    Time,
    Stop,
    /// "Encoding" and its percentage, or "Copied" and the file size, or the
    /// failure message. Not clickable.
    Status,
    Save,
    Dismiss,
}

impl Control {
    pub fn clickable(self) -> bool {
        !matches!(self, Control::Size | Control::QuitQuestion | Control::Time | Control::Status)
    }
}

const CHIP_W: f64 = 68.0;
const CANCEL_W: f64 = 64.0;
const QUIT_W: f64 = 52.0;
const STOP_W: f64 = 76.0;
const SAVE_W: f64 = 52.0;

/// The bar's controls for the session's current state, left to right.
pub fn controls(session: &Session) -> Vec<(Control, Rect)> {
    let right: &[(Control, f64)] = match &session.mode {
        Mode::Idle if session.confirm_quit => &[(Control::CancelQuit, CANCEL_W), (Control::ConfirmQuit, QUIT_W)],
        Mode::Idle | Mode::Arming | Mode::Growing => {
            &[(Control::FpsChip, CHIP_W), (Control::Record, BUTTON), (Control::Close, BUTTON)]
        }
        Mode::Recording => &[(Control::Stop, STOP_W)],
        Mode::Encoding | Mode::Failed(_) => &[(Control::Dismiss, BUTTON)],
        Mode::Done => &[(Control::Save, SAVE_W), (Control::Dismiss, BUTTON)],
    };
    let label = match &session.mode {
        Mode::Idle if session.confirm_quit => Control::QuitQuestion,
        Mode::Idle | Mode::Arming | Mode::Growing => Control::Size,
        Mode::Recording => Control::Time,
        _ => Control::Status,
    };
    let top = ((BAR_H - BUTTON) / 2.0).round();
    let mut x = BAR_W - BAR_PAD_RIGHT;
    let mut placed: Vec<(Control, Rect)> = right
        .iter()
        .rev()
        .map(|&(control, w)| {
            x -= w;
            let r = Rect::new(x, top, w, BUTTON);
            x -= BAR_SPACING;
            (control, r)
        })
        .collect();
    placed.push((label, Rect::new(BAR_PAD_LEFT, top, x - BAR_PAD_LEFT, BUTTON)));
    placed.reverse();
    placed
}

/// The clickable control under `(x, y)`, in bar coordinates.
pub fn hit(session: &Session, x: f64, y: f64) -> Option<Control> {
    controls(session).into_iter().find(|(c, r)| c.clickable() && r.contains(x, y)).map(|(c, _)| c)
}

/// One row per fps option, in menu coordinates, for a menu `width` wide.
pub fn menu_items(width: f64) -> Vec<(u32, Rect)> {
    FPS_OPTIONS
        .iter()
        .enumerate()
        .map(|(i, &fps)| {
            let y = MENU_PAD + MENU_ITEM_H * i as f64;
            (fps, Rect::new(MENU_PAD, y, width - 2.0 * MENU_PAD, MENU_ITEM_H))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Event;
    use std::time::Duration;

    #[test]
    fn idle_bar_fits_and_hits_its_buttons() {
        let s = Session::default();
        let cs = controls(&s);
        assert_eq!(
            cs.iter().map(|c| c.0).collect::<Vec<_>>(),
            [Control::Size, Control::FpsChip, Control::Record, Control::Close]
        );
        assert!(cs.iter().all(|(_, r)| r.x >= 0.0 && r.right() <= BAR_W));
        assert_eq!(hit(&s, 206.0, 18.0), Some(Control::Close));
        assert_eq!(hit(&s, 20.0, 18.0), None);
    }

    #[test]
    fn confirm_replaces_the_idle_controls() {
        let mut s = Session::default();
        s.handle(Duration::ZERO, Event::Close);
        assert_eq!(hit(&s, 200.0, 18.0), Some(Control::ConfirmQuit));
    }

    #[test]
    fn menu_has_a_row_per_option() {
        let items = menu_items(84.0);
        assert_eq!(items.len(), FPS_OPTIONS.len());
        assert_eq!(items[1].0, 15);
    }
}
