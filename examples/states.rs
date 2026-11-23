//! Draws every state of the frame, the bar and the fps menu to PNG files.
//!
//! cargo run --example states -- <font file>
//!
//! Writes target/states/<name>@1x.png and @2x.png on a grey backdrop, so the
//! translucent pixels show. `frame-*` shows the four edge windows together.

use std::path::Path;
use std::time::Duration;

use ab_glyph::FontVec;
use inb::draw;
use inb::geometry::{Rect, SIDES, layout};
use inb::session::{Event, Session};
use inb::spec::HIT;
use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// The session after `events`, each at its time in milliseconds.
fn session(events: &[(u64, Event)]) -> Session {
    let mut s = Session::default();
    for (t, e) in events {
        s.handle(ms(*t), e.clone());
    }
    s
}

fn recording() -> Vec<(u64, Event)> {
    vec![(0, Event::Record), (1000, Event::Tick), (1600, Event::Tick)]
}

fn stopped() -> Vec<(u64, Event)> {
    let mut e = recording();
    e.push((5000, Event::Stop));
    e
}

fn save(p: &Pixmap, dir: &Path, name: &str, scale: u32) {
    let mut out = Pixmap::new(p.width(), p.height()).expect("size");
    out.fill(Color::from_rgba8(0x80, 0x80, 0x80, 255));
    out.draw_pixmap(0, 0, p.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    out.save_png(dir.join(format!("{name}@{scale}x.png"))).expect("write png");
}

fn frame(area: Rect, red: f32, handles: bool, scale: f64) -> Pixmap {
    let outer = area.outset(HIT);
    let screen = outer.outset(60.0);
    let l = layout(area, screen);
    let size = ((outer.w * scale) as u32, (outer.h * scale) as u32);
    let mut canvas = Pixmap::new(size.0, size.1).expect("size");
    for (side, rect) in SIDES.iter().zip(l.edges) {
        let p = draw::edge(*side, rect, red, handles, scale);
        let (x, y) = (((rect.x - outer.x) * scale) as i32, ((rect.y - outer.y) * scale) as i32);
        canvas.draw_pixmap(x, y, p.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }
    canvas
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: states <font file>");
        std::process::exit(2);
    });
    let font =
        FontVec::try_from_vec(std::fs::read(&path).expect("read the font file")).expect("a TrueType or OpenType font");
    let dir = Path::new("target/states");
    std::fs::create_dir_all(dir).expect("create target/states");
    let area = Rect::new(0.0, 0.0, 640.0, 400.0);
    let mut done = recording();
    done.extend([(5000, Event::Stop), (5100, Event::EncodeProgress(0.4)), (6000, Event::Encoded { bytes: 1_258_291 })]);
    let mut saved = done.clone();
    saved.extend([(6100, Event::Save), (6200, Event::Saved)]);
    let mut enc = stopped();
    enc.push((5100, Event::EncodeProgress(0.4)));
    let bars = [
        ("bar-idle", session(&[]), 0),
        ("bar-arming", session(&[(0, Event::Record)]), 500),
        ("bar-growing", session(&recording()[..2]), 1300),
        ("bar-recording", session(&recording()), 1600 + 65_000),
        ("bar-confirm", session(&[(0, Event::Close)]), 0),
        ("bar-encoding", session(&enc), 6500),
        ("bar-done", session(&done), 7000),
        ("bar-done-saved", session(&saved), 7000),
        ("bar-failed", session(&[(0, Event::Record), (100, Event::Failed("Screen recording is off".into()))]), 200),
    ];
    let menu = Rect::new(0.0, 0.0, 84.0, 8.0 + 28.0 * 6.0);
    for scale in [1u32, 2] {
        let s = f64::from(scale);
        for (name, session, now) in &bars {
            save(&draw::bar(session, ms(*now), area, Some(&font), s), dir, name, scale);
        }
        save(&draw::menu(menu, 15, Some(25), Some(&font), s), dir, "menu-hover", scale);
        let edge = Rect::new(0.0, 0.0, 200.0, HIT);
        for (red, tag) in [(0.0, "red0"), (1.0, "red1")] {
            for side in SIDES {
                let rect = if matches!(side, inb::geometry::Side::Top | inb::geometry::Side::Bottom) {
                    edge
                } else {
                    Rect::new(0.0, 0.0, HIT, 120.0)
                };
                save(
                    &draw::edge(side, rect, red, red == 0.0, s),
                    dir,
                    &format!("edge-{side:?}-{tag}").to_lowercase(),
                    scale,
                );
            }
            save(&frame(Rect::new(0.0, 0.0, 200.0, 120.0), red, red == 0.0, s), dir, &format!("frame-{tag}"), scale);
        }
    }
    println!("wrote target/states");
}
