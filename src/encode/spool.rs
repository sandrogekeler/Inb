//! The frames of one recording, kept in a temporary file while it runs.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use super::EncodeError;
use crate::frame::Frame;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// Per frame: width and height as `u32`, `at` in microseconds as `u64`, all
/// little endian, then the raw RGBA bytes.
const HEADER: usize = 16;

/// The frames of one recording. They live in a file under
/// `temp_dir()/Inb/`, created by the first `push` and deleted on drop, so
/// memory stays flat however long the recording runs.
#[derive(Default)]
pub struct Spool {
    pub(super) path: Option<PathBuf>,
    writer: Option<BufWriter<File>>,
    count: usize,
    size: (u32, u32),
}

impl Spool {
    pub fn push(&mut self, frame: Frame) -> Result<(), EncodeError> {
        if frame.rgba.len() as u64 != u64::from(frame.width) * u64::from(frame.height) * 4 {
            return Err(EncodeError::BadFrame);
        }
        if self.count == 0 {
            self.size = (frame.width, frame.height);
        } else if self.size != (frame.width, frame.height) {
            return Err(EncodeError::BadFrame);
        }
        if self.writer.is_none() {
            let dir = std::env::temp_dir().join("Inb");
            std::fs::create_dir_all(&dir)?;
            let name = format!("spool-{}-{}.rgba", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
            let path = dir.join(name);
            self.writer = Some(BufWriter::new(File::create(&path)?));
            self.path = Some(path);
        }
        let writer = self.writer.as_mut().expect("created above");
        let mut header = [0u8; HEADER];
        header[..4].copy_from_slice(&frame.width.to_le_bytes());
        header[4..8].copy_from_slice(&frame.height.to_le_bytes());
        header[8..].copy_from_slice(&(frame.at.as_micros() as u64).to_le_bytes());
        writer.write_all(&header)?;
        writer.write_all(&frame.rgba)?;
        self.count += 1;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Ends the writing and reads the frames back in order. The file stays
    /// until the spool is dropped.
    pub(super) fn reader(&mut self) -> Result<SpoolReader, EncodeError> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush()?;
        }
        let path = self.path.as_ref().ok_or(EncodeError::NoFrames)?;
        Ok(SpoolReader { file: BufReader::new(File::open(path)?), left: self.count })
    }
}

impl Drop for Spool {
    fn drop(&mut self) {
        self.writer = None;
        if let Some(path) = &self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub(super) struct SpoolReader {
    file: BufReader<File>,
    left: usize,
}

impl SpoolReader {
    pub(super) fn next_frame(&mut self) -> Result<Option<Frame>, EncodeError> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        let mut header = [0u8; HEADER];
        self.file.read_exact(&mut header)?;
        let width = u32::from_le_bytes(header[..4].try_into().expect("4 bytes"));
        let height = u32::from_le_bytes(header[4..8].try_into().expect("4 bytes"));
        let at = u64::from_le_bytes(header[8..].try_into().expect("8 bytes"));
        let mut rgba = vec![0; width as usize * height as usize * 4];
        self.file.read_exact(&mut rgba)?;
        Ok(Some(Frame { width, height, rgba, at: Duration::from_micros(at) }))
    }
}
