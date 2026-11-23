//! Spooling captured frames to disk while recording and turning them into a
//! GIF: delays from each frame's capture time, identical frames merged, and
//! only the changed area written after the first frame.

mod spool;

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub use spool::Spool;

use crate::frame::Frame;

/// NeuQuant sampling step of the `gif` crate: 1 looks at every pixel, 30 at
/// every 30th. 5 finishes the 30 s example in 0.22 s, as fast as 10 to 30 and
/// faster than 1 (0.38 s), at the same file size within 1% (measured
/// 2026-11-23 with `examples/encode.rs`). A busy full-screen recording
/// quantizes more pixels per frame, so revisit if encoding feels slow.
const QUANT_SPEED: i32 = 5;

/// Browsers show a delay under 2 hundredths of a second as 10.
const MIN_DELAY: i64 = 2;

#[derive(Debug)]
pub enum EncodeError {
    Io(std::io::Error),
    Gif(gif::EncodingError),
    NoFrames,
    BadFrame,
    Cancelled,
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::Io(e) => write!(f, "could not write the GIF: {e}"),
            EncodeError::Gif(e) => write!(f, "could not encode the GIF: {e}"),
            EncodeError::NoFrames => f.write_str("nothing was recorded"),
            EncodeError::BadFrame => f.write_str("a captured frame had the wrong size"),
            EncodeError::Cancelled => f.write_str("encoding was cancelled"),
        }
    }
}

impl From<std::io::Error> for EncodeError {
    fn from(e: std::io::Error) -> Self {
        EncodeError::Io(e)
    }
}

impl From<gif::EncodingError> for EncodeError {
    fn from(e: gif::EncodingError) -> Self {
        EncodeError::Gif(e)
    }
}

/// Writes the spooled frames to `out` as a looping GIF. `fps` sets how long
/// the last frame shows. Calls `progress` with 0 to 1 and stops early when
/// `cancel` is set. A failed or cancelled encode deletes `out`. Returns the
/// file's size in bytes.
pub fn encode(
    mut spool: Spool,
    out: &Path,
    fps: u32,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> Result<u64, EncodeError> {
    if spool.is_empty() {
        return Err(EncodeError::NoFrames);
    }
    let result = write_gif(&mut spool, out, fps, progress, cancel);
    if result.is_err() {
        let _ = std::fs::remove_file(out);
    }
    result
}

fn write_gif(
    spool: &mut Spool,
    out: &Path,
    fps: u32,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> Result<u64, EncodeError> {
    let total = spool.len();
    let mut reader = spool.reader()?;
    let first = reader.next_frame()?.ok_or(EncodeError::NoFrames)?;
    let mut encoder =
        gif::Encoder::new(BufWriter::new(File::create(out)?), first.width as u16, first.height as u16, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let mut clock = Clock { origin: first.at.as_micros() as u64, written: 0 };
    let interval = 1_000_000 / u64::from(fps.max(1));
    let mut last_at = clock.origin;
    let mut shown: Option<Frame> = None;
    let mut pending = first;
    progress(1.0 / total as f32);
    for i in 1..total {
        if cancel.load(Ordering::Relaxed) {
            return Err(EncodeError::Cancelled);
        }
        let frame = reader.next_frame()?.ok_or(EncodeError::NoFrames)?;
        last_at = frame.at.as_micros() as u64;
        if frame.rgba != pending.rgba {
            let delay = clock.delay(last_at);
            write_frame(&mut encoder, shown.as_ref(), &pending, delay)?;
            shown = Some(std::mem::replace(&mut pending, frame));
        }
        progress((i + 1) as f32 / total as f32);
    }
    let delay = clock.delay(last_at + interval);
    write_frame(&mut encoder, shown.as_ref(), &pending, delay)?;
    drop(encoder);
    Ok(std::fs::metadata(out)?.len())
}

/// Turns capture times into GIF delays. Each delay runs to the next boundary
/// in hundredths of a second, counted from the start, so rounding never adds
/// up: the delays sum to the recording's length.
struct Clock {
    origin: u64,
    written: i64,
}

impl Clock {
    fn delay(&mut self, boundary_micros: u64) -> u16 {
        let boundary = ((boundary_micros - self.origin + 5_000) / 10_000) as i64;
        // shortcut: a still picture over 655 s is cut short, revisit if
        // recordings that long are allowed.
        let delay = (boundary - self.written).clamp(MIN_DELAY, i64::from(u16::MAX));
        self.written += delay;
        delay as u16
    }
}

/// Writes `frame` showing for `delay`, as just the area that changed from
/// `shown` (the whole picture for the first frame).
fn write_frame(
    encoder: &mut gif::Encoder<BufWriter<File>>,
    shown: Option<&Frame>,
    frame: &Frame,
    delay: u16,
) -> Result<(), EncodeError> {
    let (left, top, width, height) = match shown {
        Some(shown) => changed_area(shown, frame),
        None => (0, 0, frame.width, frame.height),
    };
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in top..top + height {
        let start = (y as usize * frame.width as usize + left as usize) * 4;
        rgba.extend_from_slice(&frame.rgba[start..start + width as usize * 4]);
    }
    let mut gif_frame = gif::Frame::from_rgba_speed(width as u16, height as u16, &mut rgba, QUANT_SPEED);
    gif_frame.left = left as u16;
    gif_frame.top = top as u16;
    gif_frame.delay = delay;
    gif_frame.dispose = gif::DisposalMethod::Keep;
    encoder.write_frame(&gif_frame)?;
    Ok(())
}

/// The smallest box `(left, top, width, height)` holding every pixel that
/// differs. The frames differ, so the box is never empty.
fn changed_area(old: &Frame, new: &Frame) -> (u32, u32, u32, u32) {
    let w = new.width as usize;
    fn row(f: &Frame, y: usize) -> &[u8] {
        let n = f.width as usize * 4;
        &f.rgba[y * n..(y + 1) * n]
    }
    let (mut top, mut bottom, mut left, mut right) = (usize::MAX, 0, w, 0);
    for y in 0..new.height as usize {
        let (a, b) = (row(old, y), row(new, y));
        if a == b {
            continue;
        }
        top = top.min(y);
        bottom = y;
        let differs = |x: usize| a[x * 4..x * 4 + 4] != b[x * 4..x * 4 + 4];
        left = left.min((0..w).find(|&x| differs(x)).unwrap_or(w));
        right = right.max((0..w).rev().find(|&x| differs(x)).unwrap_or(0));
    }
    (left as u32, top as u32, (right - left + 1) as u32, (bottom - top + 1) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    type Seen = Vec<[u16; 5]>;

    fn temp_gif() -> std::path::PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("inb-encode-test-{}-{n}.gif", std::process::id()))
    }

    fn frame(rgba: Vec<u8>, at_micros: u64) -> Frame {
        Frame { width: 8, height: 6, rgba, at: Duration::from_micros(at_micros) }
    }

    fn solid(v: u8) -> Vec<u8> {
        [v, 0, 255 - v, 255].repeat(8 * 6)
    }

    /// Encodes `frames` and returns the file with each frame's
    /// (left, top, width, height, delay).
    fn run(frames: Vec<Frame>, fps: u32) -> (std::path::PathBuf, Seen) {
        let mut spool = Spool::default();
        for f in frames {
            spool.push(f).unwrap();
        }
        let out = temp_gif();
        encode(spool, &out, fps, &mut |_| {}, &AtomicBool::new(false)).unwrap();
        let mut decoder = gif::DecodeOptions::new().read_info(File::open(&out).unwrap()).unwrap();
        let mut seen = Vec::new();
        while let Some(f) = decoder.read_next_frame().unwrap() {
            seen.push([f.left, f.top, f.width, f.height, f.delay]);
        }
        (out, seen)
    }

    #[test]
    fn delays_sum_to_the_recording_length() {
        // 31 frames at 15 fps: 30 gaps of 66.666 ms plus one interval.
        let frames = (0..31).map(|i| frame(solid(i as u8 * 7), i * 66_666)).collect();
        let (out, seen) = run(frames, 15);
        let sum: u32 = seen.iter().map(|f| u32::from(f[4])).sum();
        assert_eq!(seen.len(), 31);
        assert!(sum.abs_diff(207) <= 1, "sum was {sum}");
        std::fs::remove_file(out).unwrap();
    }

    #[test]
    fn identical_frames_merge_into_one_longer_frame() {
        let frames =
            vec![frame(solid(10), 0), frame(solid(10), 100_000), frame(solid(10), 200_000), frame(solid(200), 300_000)];
        let (out, seen) = run(frames, 10);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0][4], 30);
        assert_eq!(seen[1][4], 10);
        std::fs::remove_file(out).unwrap();
    }

    #[test]
    fn a_changed_corner_writes_a_sub_frame() {
        let mut corner = solid(10);
        for (x, y) in [(6, 4), (7, 4), (6, 5), (7, 5)] {
            let i = (y * 8 + x) * 4;
            corner[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
        let (out, seen) = run(vec![frame(solid(10), 0), frame(corner, 100_000)], 10);
        assert_eq!(seen[0][..4], [0, 0, 8, 6]);
        assert_eq!(seen[1][..4], [6, 4, 2, 2]);
        std::fs::remove_file(out).unwrap();
    }

    #[test]
    fn cancel_removes_the_file() {
        let mut spool = Spool::default();
        for i in 0..3 {
            spool.push(frame(solid(i as u8 * 50), i * 66_000)).unwrap();
        }
        let out = temp_gif();
        let r = encode(spool, &out, 15, &mut |_| {}, &AtomicBool::new(true));
        assert!(matches!(r, Err(EncodeError::Cancelled)));
        assert!(!out.exists());
    }

    #[test]
    fn spool_file_is_gone_after_encode() {
        let mut spool = Spool::default();
        spool.push(frame(solid(1), 0)).unwrap();
        let path = spool.path.clone().unwrap();
        assert!(path.exists());
        let out = temp_gif();
        encode(spool, &out, 15, &mut |_| {}, &AtomicBool::new(false)).unwrap();
        assert!(!path.exists());
        std::fs::remove_file(out).unwrap();
    }

    #[test]
    fn push_refuses_a_frame_of_another_size() {
        let mut spool = Spool::default();
        spool.push(frame(solid(1), 0)).unwrap();
        let small = Frame { width: 2, height: 2, rgba: vec![0; 16], at: Duration::ZERO };
        assert!(matches!(spool.push(small), Err(EncodeError::BadFrame)));
    }

    #[test]
    fn empty_spool_is_an_error() {
        let r = encode(Spool::default(), Path::new("unused.gif"), 15, &mut |_| {}, &AtomicBool::new(false));
        assert!(matches!(r, Err(EncodeError::NoFrames)));
    }
}
