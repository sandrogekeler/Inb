//! A pixmap drawn on in points: shapes and text, scaled to physical pixels.

use ab_glyph::{Font, FontVec, GlyphId, PxScale, ScaleFont, point};
use tiny_skia::{FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

use crate::draw::color;
use crate::geometry::Rect;
use crate::spec::{BOLD_SMEAR, FONT_SIZE, Rgba};

/// Control point distance that makes a cubic curve a quarter circle.
const CIRCLE_K: f32 = 0.552_284_8;

/// How a run of text looks.
#[derive(Clone, Copy)]
pub struct Ink {
    pub color: Rgba,
    pub bold: bool,
    /// Every digit takes the width of the widest one, so a clock does not jitter.
    pub tabular: bool,
}

impl Ink {
    pub fn new(color: Rgba) -> Self {
        Self { color, bold: false, tabular: false }
    }

    pub fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    pub fn tabular(self) -> Self {
        Self { tabular: true, ..self }
    }
}

pub struct Canvas<'a> {
    pub pm: Pixmap,
    scale: f64,
    font: Option<&'a FontVec>,
}

/// A rounded rectangle with one radius per corner: top-left, top-right,
/// bottom-right, bottom-left.
pub fn rounded(r: Rect, radii: [f64; 4]) -> Path {
    let (x, y, w, h) = (r.x as f32, r.y as f32, r.w as f32, r.h as f32);
    let [tl, tr, br, bl] = radii.map(|v| v as f32);
    let k = CIRCLE_K;
    let mut p = PathBuilder::new();
    p.move_to(x + tl, y);
    p.line_to(x + w - tr, y);
    p.cubic_to(x + w - tr + k * tr, y, x + w, y + tr - k * tr, x + w, y + tr);
    p.line_to(x + w, y + h - br);
    p.cubic_to(x + w, y + h - br + k * br, x + w - br + k * br, y + h, x + w - br, y + h);
    p.line_to(x + bl, y + h);
    p.cubic_to(x + bl - k * bl, y + h, x, y + h - bl + k * bl, x, y + h - bl);
    p.line_to(x, y + tl);
    p.cubic_to(x, y + tl - k * tl, x + tl - k * tl, y, x + tl, y);
    p.close();
    p.finish().expect("a rounded rectangle has points")
}

pub fn circle(cx: f64, cy: f64, r: f64) -> Path {
    rounded(Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r), [r; 4])
}

impl<'a> Canvas<'a> {
    pub fn new(size: (u32, u32), scale: f64, font: Option<&'a FontVec>) -> Self {
        Self { pm: Pixmap::new(size.0, size.1).expect("window sizes are never zero"), scale, font }
    }

    fn transform(&self) -> Transform {
        Transform::from_scale(self.scale as f32, self.scale as f32)
    }

    fn paint(c: Rgba) -> Paint<'static> {
        let mut p = Paint::default();
        p.set_color(color(c));
        p.anti_alias = true;
        p
    }

    /// Fills `path`, only where `clip` (a path in points) covers when given.
    pub fn fill(&mut self, path: &Path, c: Rgba, clip: Option<&Path>) {
        let t = self.transform();
        let mask = clip.map(|clip| {
            let mut m = Mask::new(self.pm.width(), self.pm.height()).expect("window sizes are never zero");
            m.fill_path(clip, FillRule::Winding, true, t);
            m
        });
        self.pm.fill_path(path, &Self::paint(c), FillRule::Winding, t, mask.as_ref());
    }

    /// Fills the area between `outer` and `inner`.
    pub fn fill_ring(&mut self, outer: &Path, inner: &Path, c: Rgba) {
        let mut pb = PathBuilder::new();
        pb.push_path(outer);
        pb.push_path(inner);
        let ring = pb.finish().expect("a ring has points");
        self.pm.fill_path(&ring, &Self::paint(c), FillRule::EvenOdd, self.transform(), None);
    }

    pub fn fill_rect(&mut self, r: Rect, radius: f64, c: Rgba) {
        self.fill(&rounded(r, [radius; 4]), c, None);
    }

    /// A line through `points`, round at its ends and corners.
    pub fn line(&mut self, points: &[(f64, f64)], width: f64, c: Rgba) {
        let mut pb = PathBuilder::new();
        pb.move_to(points[0].0 as f32, points[0].1 as f32);
        for p in &points[1..] {
            pb.line_to(p.0 as f32, p.1 as f32);
        }
        let Some(path) = pb.finish() else { return };
        let stroke =
            Stroke { width: width as f32, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
        self.pm.stroke_path(&path, &Self::paint(c), &stroke, self.transform(), None);
    }

    /// A stroke `width` wide that stays inside `r`.
    pub fn outline(&mut self, r: Rect, radius: f64, width: f64, c: Rgba) {
        let half = width / 2.0;
        let path = rounded(Rect::new(r.x + half, r.y + half, r.w - width, r.h - width), [radius - half; 4]);
        let stroke = Stroke { width: width as f32, ..Stroke::default() };
        self.pm.stroke_path(&path, &Self::paint(c), &stroke, self.transform(), None);
    }

    fn px_scale(&self, font: &FontVec) -> PxScale {
        let em = FONT_SIZE * self.scale as f32;
        let ratio = font.units_per_em().map_or(1.0, |u| font.height_unscaled() / u);
        PxScale::from(em * ratio)
    }

    /// Glyphs with their x offsets in pixels, and the run's width in pixels.
    fn run(&self, font: &FontVec, s: &str, ink: Ink) -> (Vec<(GlyphId, f32)>, f32) {
        let scaled = font.as_scaled(self.px_scale(font));
        let digit = ('0'..='9').map(|c| scaled.h_advance(scaled.glyph_id(c))).fold(0.0, f32::max);
        let (mut x, mut prev, mut out) = (0.0, None, Vec::new());
        for ch in s.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(p) = prev {
                x += scaled.kern(p, id);
            }
            let own = scaled.h_advance(id);
            let advance = if ink.tabular && ch.is_ascii_digit() { digit } else { own };
            out.push((id, x + (advance - own) / 2.0));
            x += advance;
            prev = Some(id);
        }
        (out, x)
    }

    /// The width of `s` in points, 0 without a font.
    pub fn width(&self, s: &str, ink: Ink) -> f64 {
        self.font.map_or(0.0, |f| f64::from(self.run(f, s, ink).1) / self.scale)
    }

    /// `s`, shortened with an ellipsis until it fits `max` points.
    pub fn fit(&self, s: &str, max: f64, ink: Ink) -> String {
        if self.width(s, ink) <= max {
            return s.to_string();
        }
        let mut chars: Vec<char> = s.chars().collect();
        while chars.pop().is_some() {
            let cut = chars.iter().collect::<String>() + "…";
            if self.width(&cut, ink) <= max {
                return cut;
            }
        }
        String::new()
    }

    /// Draws `s` from `x`, its capital letters centred on `cy`. Returns its
    /// width in points. Draws nothing without a font.
    pub fn text(&mut self, s: &str, x: f64, cy: f64, ink: Ink) -> f64 {
        let Some(font) = self.font else { return 0.0 };
        let ps = self.px_scale(font);
        let (glyphs, width) = self.run(font, s, ink);
        let cap = font
            .outline_glyph(font.glyph_id('H').with_scale_and_position(ps, point(0.0, 0.0)))
            .map_or(ps.y * 0.7, |g| -g.px_bounds().min.y);
        let scale = self.scale as f32;
        let baseline = cy as f32 * scale + cap / 2.0;
        let offsets: &[f32] = if ink.bold { &[0.0, BOLD_SMEAR as f32 * scale] } else { &[0.0] };
        for dx in offsets {
            for &(id, gx) in &glyphs {
                let origin = point(x as f32 * scale + gx + dx, baseline);
                let Some(g) = font.outline_glyph(id.with_scale_and_position(ps, origin)) else { continue };
                let b = g.px_bounds();
                g.draw(|px, py, cov| {
                    self.blend(b.min.x as i32 + px as i32, b.min.y as i32 + py as i32, ink.color, cov)
                });
            }
        }
        f64::from(width) / scale as f64
    }

    /// `s` centred on `cx`.
    pub fn text_centered(&mut self, s: &str, cx: f64, cy: f64, ink: Ink) {
        let w = self.width(s, ink);
        self.text(s, cx - w / 2.0, cy, ink);
    }

    /// Source-over of `c` at coverage `cov` onto one premultiplied pixel.
    fn blend(&mut self, x: i32, y: i32, c: Rgba, cov: f32) {
        let (w, h) = (self.pm.width() as i32, self.pm.height() as i32);
        if x < 0 || y < 0 || x >= w || y >= h {
            return;
        }
        let a = cov * f32::from(c.3) / 255.0;
        let i = (y * w + x) as usize * 4;
        let px = &mut self.pm.data_mut()[i..i + 4];
        for (d, s) in px.iter_mut().zip([c.0, c.1, c.2, 255]) {
            *d = (f32::from(s) * a + f32::from(*d) * (1.0 - a)).round() as u8;
        }
    }
}
