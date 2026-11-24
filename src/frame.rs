//! One captured picture, as capture hands it to the encoder.

use std::time::Duration;

/// Straight RGBA, `width * height * 4` bytes, rows without padding, already
/// cropped to the area and scaled to its size in logical points.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Time since the capture started.
    pub at: Duration,
}
