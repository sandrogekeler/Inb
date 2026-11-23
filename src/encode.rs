//! Spooling captured frames while recording and turning them into a GIF.
//!
//! shortcut: frames are kept in memory and every frame is quantized at one
//! speed, revisit in #13 (disk spool, duplicate frames, timing, quality).

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::frame::Frame;

#[derive(Debug)]
pub enum EncodeError {
    Io(std::io::Error),
    Gif(gif::EncodingError),
    NoFrames,
    Cancelled,
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::Io(e) => write!(f, "could not write the GIF: {e}"),
            EncodeError::Gif(e) => write!(f, "could not encode the GIF: {e}"),
            EncodeError::NoFrames => f.write_str("nothing was recorded"),
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

/// The frames of one recording, collected while it runs.
#[derive(Default)]
pub struct Spool {
    frames: Vec<Frame>,
}

impl Spool {
    pub fn push(&mut self, frame: Frame) -> Result<(), EncodeError> {
        self.frames.push(frame);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// Writes the spooled frames to `out` as a looping GIF at `fps`. Calls
/// `progress` with 0 to 1 and stops early when `cancel` is set. Returns the
/// file's size in bytes.
pub fn encode(
    spool: Spool,
    out: &Path,
    fps: u32,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> Result<u64, EncodeError> {
    let first = spool.frames.first().ok_or(EncodeError::NoFrames)?;
    let (w, h) = (first.width as u16, first.height as u16);
    let delay = (100 / fps.max(1)) as u16;
    let mut encoder = gif::Encoder::new(BufWriter::new(File::create(out)?), w, h, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let total = spool.frames.len();
    for (i, mut f) in spool.frames.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(EncodeError::Cancelled);
        }
        let mut gif_frame = gif::Frame::from_rgba_speed(f.width as u16, f.height as u16, &mut f.rgba, 10);
        gif_frame.delay = delay;
        encoder.write_frame(&gif_frame)?;
        progress((i + 1) as f32 / total as f32);
    }
    drop(encoder);
    Ok(std::fs::metadata(out)?.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn writes_a_readable_gif() {
        let mut spool = Spool::default();
        for i in 0..3u8 {
            let rgba = [i * 80, 0, 255 - i * 80, 255].repeat(4 * 3);
            spool.push(Frame { width: 4, height: 3, rgba, at: Duration::from_millis(i as u64 * 66) }).unwrap();
        }
        let out = std::env::temp_dir().join(format!("inb-encode-test-{}.gif", std::process::id()));
        let mut seen = Vec::new();
        let bytes = encode(spool, &out, 15, &mut |p| seen.push(p), &AtomicBool::new(false)).unwrap();
        assert!(bytes > 0);
        assert_eq!(seen.last(), Some(&1.0));
        let mut decoder = gif::DecodeOptions::new().read_info(File::open(&out).unwrap()).unwrap();
        let mut count = 0;
        while decoder.read_next_frame().unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 3);
        std::fs::remove_file(out).unwrap();
    }

    #[test]
    fn empty_spool_is_an_error() {
        let r = encode(Spool::default(), Path::new("unused.gif"), 15, &mut |_| {}, &AtomicBool::new(false));
        assert!(matches!(r, Err(EncodeError::NoFrames)));
    }
}
