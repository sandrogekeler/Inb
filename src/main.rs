#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(any(windows, target_os = "macos"))]
mod app;

#[cfg(any(windows, target_os = "macos"))]
fn main() {
    if let Err(e) = app::run() {
        eprintln!("Inb could not start: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn main() {
    eprintln!("Inb runs on macOS and Windows.");
    std::process::exit(1);
}
