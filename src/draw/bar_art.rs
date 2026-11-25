//! The bar's picture for each session state.

use std::time::Duration;

use ab_glyph::FontVec;
use tiny_skia::Pixmap;

use super::canvas::{Canvas, Ink, circle, rounded};
use crate::bar::{Control, controls};
use crate::geometry::Rect;
use crate::session::{Mode, Session};
use crate::spec::*;

/// The bar for the session's state. `area` is the recorded area in points.
pub fn render(session: &Session, now: Duration, area: Rect, font: Option<&FontVec>, scale: f64) -> Pixmap {
    let size = super::pixel_size(Rect::new(0.0, 0.0, BAR_W, BAR_H), scale);
    let mut c = Canvas::new(size, scale, font);
    let full = Rect::new(0.0, 0.0, BAR_W, BAR_H);
    c.fill_rect(full, RADIUS, BG);
    // While growing the circle covers the idle controls; afterwards it is the
    // background the recording controls sit on.
    let growing = session.mode == Mode::Growing;
    if !growing {
        red_circle(&mut c, session.circle(now));
    }
    for (control, r) in controls(session) {
        draw_control(&mut c, session, now, area, control, r);
    }
    if growing {
        red_circle(&mut c, session.circle(now));
    }
    if session.mode == Mode::Encoding {
        let line =
            rounded(Rect::new(0.0, BAR_H - PROGRESS_H, BAR_W * f64::from(session.progress), PROGRESS_H), [0.0; 4]);
        c.fill(&line, FG, Some(&rounded(full, [RADIUS; 4])));
    }
    c.outline(full, RADIUS, OUTLINE, EDGE);
    c.pm
}

/// Where the record button sits, which the growing circle starts from.
fn record_centre() -> (f64, f64) {
    let r = controls(&Session::default()).into_iter().find(|(k, _)| *k == Control::Record).map(|(_, r)| r);
    r.map_or((0.0, 0.0), |r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
}

/// The `REC_BG` circle at `t` (0 to 1) of its growth, clipped to the bar.
fn red_circle(c: &mut Canvas, t: f32) {
    if t <= 0.0 {
        return;
    }
    let (cx, cy) = record_centre();
    let reach = cx.max(BAR_W - cx).hypot(cy.max(BAR_H - cy));
    let r = DOT_SIZE / 2.0 + (reach - DOT_SIZE / 2.0) * f64::from(t);
    c.fill(&circle(cx, cy, r), REC_BG, Some(&rounded(Rect::new(0.0, 0.0, BAR_W, BAR_H), [RADIUS; 4])));
}

fn draw_control(c: &mut Canvas, s: &Session, now: Duration, area: Rect, control: Control, r: Rect) {
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let fg = Ink::new(FG);
    match control {
        Control::Size => {
            let text = format!("{} × {}", area.w.round(), area.h.round());
            let text = c.fit(&text, r.w, Ink::new(MUTED));
            c.text(&text, r.x, cy, Ink::new(MUTED));
        }
        Control::FpsChip => fps_chip(c, s.fps, r),
        Control::Record => record_button(c, s.arm_progress(now), cx, cy),
        Control::Close | Control::Dismiss => cross(c, cx, cy),
        Control::QuitQuestion => {
            c.text("Quit Inb?", r.x, cy, fg);
        }
        Control::CancelQuit => {
            c.fill_rect(r, RADIUS, CHIP);
            c.text_centered("Cancel", cx, cy, fg);
        }
        Control::ConfirmQuit => {
            c.fill_rect(r, RADIUS, REC_BG);
            c.text_centered("Quit", cx, cy, Ink::new(REC_FG));
        }
        Control::Time => {
            c.fill(&circle(r.x + REC_DOT_SIZE / 2.0, cy, REC_DOT_SIZE / 2.0), REC_FG, None);
            c.text(&s.elapsed_label(now), r.x + REC_DOT_SIZE + ICON_GAP, cy, Ink::new(REC_FG).tabular());
        }
        Control::Stop => stop_button(c, r),
        Control::Status => status(c, s, r),
        Control::Save => {
            let (fill, ink, label) = if s.saved { (CHIP, fg, "Saved") } else { (FG, Ink::new(BG), "Save") };
            c.fill_rect(r, RADIUS, fill);
            c.text_centered(label, cx, cy, ink);
        }
    }
}

fn fps_chip(c: &mut Canvas, fps: u32, r: Rect) {
    let cy = r.y + r.h / 2.0;
    c.fill_rect(r, RADIUS, CHIP);
    c.text(&format!("{fps} fps"), r.x + CHIP_TEXT_PAD, cy, Ink::new(FG));
    let mid = r.right() - CHEVRON_INSET - CHEVRON_W / 2.0;
    let (hw, hh) = (CHEVRON_W / 2.0, CHEVRON_H / 2.0);
    c.line(&[(mid - hw, cy - hh), (mid, cy + hh), (mid + hw, cy - hh)], 1.4, MUTED);
}

/// The record dot in its ring. While arming, the dot is wiped clockwise from
/// 12 o'clock: `wiped` (0 to 1) of it is gone.
fn record_button(c: &mut Canvas, wiped: f32, cx: f64, cy: f64) {
    let radius = DOT_SIZE / 2.0;
    let ring = circle(cx, cy, radius + DOT_RING);
    c.fill_ring(&ring, &circle(cx, cy, radius), CHIP);
    if wiped <= 0.0 {
        c.fill(&circle(cx, cy, radius), DOT, None);
        return;
    }
    // A fan of short edges, a little past the dot's radius so the edges stay outside it.
    const STEPS: usize = 96;
    let (start, left) =
        (-std::f64::consts::FRAC_PI_2 + f64::from(wiped) * std::f64::consts::TAU, 1.0 - f64::from(wiped));
    let n = ((STEPS as f64 * left).ceil() as usize).max(1);
    let mut pts = vec![(cx, cy)];
    for i in 0..=n {
        let a = start + left * std::f64::consts::TAU * i as f64 / n as f64;
        pts.push((cx + radius * 1.02 * a.cos(), cy + radius * 1.02 * a.sin()));
    }
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(pts[0].0 as f32, pts[0].1 as f32);
    for p in &pts[1..] {
        pb.line_to(p.0 as f32, p.1 as f32);
    }
    pb.close();
    if let Some(fan) = pb.finish() {
        c.fill(&fan, DOT, Some(&circle(cx, cy, radius)));
    }
}

fn cross(c: &mut Canvas, cx: f64, cy: f64) {
    let h = CROSS_SIZE / 2.0;
    c.line(&[(cx - h, cy - h), (cx + h, cy + h)], CROSS_STROKE, MUTED);
    c.line(&[(cx - h, cy + h), (cx + h, cy - h)], CROSS_STROKE, MUTED);
}

pub fn check(c: &mut Canvas, x: f64, cy: f64, color: Rgba) {
    let (w, h) = (CHECK_W, CHECK_H);
    let top = cy - h / 2.0;
    c.line(&[(x, top + h * 0.55), (x + w * 0.38, top + h), (x + w, top)], 1.6, color);
}

fn stop_button(c: &mut Canvas, r: Rect) {
    let cy = r.y + r.h / 2.0;
    let ink = Ink::new(REC_BG).bold();
    c.fill_rect(r, RADIUS, REC_FG);
    let group = STOP_ICON + ICON_GAP + c.width("Stop", ink);
    let x = r.x + (r.w - group) / 2.0;
    c.fill_rect(Rect::new(x, cy - STOP_ICON / 2.0, STOP_ICON, STOP_ICON), 1.5, REC_BG);
    c.text("Stop", x + STOP_ICON + ICON_GAP, cy, ink);
}

fn status(c: &mut Canvas, s: &Session, r: Rect) {
    let cy = r.y + r.h / 2.0;
    let (fg, muted) = (Ink::new(FG), Ink::new(MUTED));
    match &s.mode {
        Mode::Encoding => {
            let w = c.text("Encoding", r.x, cy, fg);
            c.text(&format!("{}%", (s.progress * 100.0).round()), r.x + w + ICON_GAP, cy, muted);
        }
        Mode::Done => {
            check(c, r.x, cy, FG);
            let x = r.x + CHECK_W + ICON_GAP;
            let w = c.text("Copied", x, cy, fg);
            c.text(&size_label(s.gif_bytes), x + w + ICON_GAP, cy, muted);
        }
        Mode::Failed(message) => {
            let text = c.fit(message, r.w, fg);
            c.text(&text, r.x, cy, fg);
        }
        _ => {}
    }
}

/// A file size as `1.2 MB` or `840 KB`.
pub fn size_label(bytes: u64) -> String {
    const KB: u64 = 1024;
    if bytes >= KB * KB {
        format!("{:.1} MB", bytes as f64 / (KB * KB) as f64)
    } else {
        format!("{} KB", bytes.div_ceil(KB))
    }
}

#[cfg(test)]
mod tests {
    use super::size_label;

    #[test]
    fn sizes_use_mb_from_a_megabyte_up() {
        assert_eq!(size_label(1_258_291), "1.2 MB");
        assert_eq!(size_label(860_160), "840 KB");
        assert_eq!(size_label(10), "1 KB");
    }
}
