//! Inb's library: the frame's geometry, the recording session, drawing and
//! GIF encoding, none of which touch an operating system, and `platform`, the
//! only module that names one. The binary in `main.rs` and `app/` wires them up.

pub mod bar;
pub mod draw;
pub mod encode;
pub mod frame;
pub mod geometry;
pub mod image;
#[cfg(any(windows, target_os = "macos"))]
pub mod platform;
pub mod session;
pub mod spec;
