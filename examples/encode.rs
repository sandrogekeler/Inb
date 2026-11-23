//! Encodes a synthetic recording: 30 s, 640 x 400, 15 fps, a square moving
//! over a gradient. Prints the file size, the encode time and the peak memory
//! where the system reports it, and writes `target/encode.gif`.
//!
//! cargo run --release --example encode

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use inb::encode::{Spool, encode};
use inb::frame::Frame;

const W: u32 = 640;
const H: u32 = 400;
const FPS: u32 = 15;
const SECONDS: u32 = 30;
const SQUARE: u32 = 60;

fn picture(i: u32, total: u32) -> Vec<u8> {
    let travel = (W - SQUARE) as f32;
    let phase = i as f32 / total as f32 * 2.0;
    let x0 = (if phase <= 1.0 { phase } else { 2.0 - phase } * travel) as u32;
    let y0 = (H - SQUARE) / 2;
    let mut rgba = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            if (x0..x0 + SQUARE).contains(&x) && (y0..y0 + SQUARE).contains(&y) {
                rgba.extend_from_slice(&[255, 255, 255, 255]);
            } else {
                let r = (x * 255 / W) as u8;
                let g = (y * 255 / H) as u8;
                rgba.extend_from_slice(&[r, g, 160, 255]);
            }
        }
    }
    rgba
}

/// Linux only: the process's peak resident memory.
fn peak_memory() -> Option<String> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|l| l.strip_prefix("VmHWM:")).map(|v| v.trim().to_string())
}

fn main() {
    let total = FPS * SECONDS;
    let mut spool = Spool::default();
    for i in 0..total {
        let at = Duration::from_micros(u64::from(i) * 1_000_000 / u64::from(FPS));
        spool.push(Frame { width: W, height: H, rgba: picture(i, total), at }).expect("spool a frame");
    }
    let out = std::path::Path::new("target/encode.gif");
    let started = Instant::now();
    let bytes = encode(spool, out, FPS, &mut |_| {}, &AtomicBool::new(false)).expect("encode");
    println!("{total} frames, {W} x {H}, {FPS} fps, {SECONDS} s");
    println!("file size: {:.2} MB ({bytes} bytes)", bytes as f64 / 1e6);
    println!("encode time: {:.2} s", started.elapsed().as_secs_f64());
    println!("peak memory: {}", peak_memory().unwrap_or_else(|| "not available on this system".into()));
    println!("written to {}", out.display());
}
