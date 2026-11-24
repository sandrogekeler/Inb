//! Tries the clipboard, the save folder and the UI font on this system.
//!
//! cargo run --example clipboard -- <file.gif>   copies the GIF
//! cargo run --example clipboard -- --save-dir   prints the save folder
//! cargo run --example clipboard -- --font       prints the UI font's glyph count

#[cfg(any(windows, target_os = "macos"))]
fn main() {
    use ab_glyph::Font;
    use inb::platform::sys::{clipboard, files};

    let arg = std::env::args().nth(1).unwrap_or_default();
    let result = match arg.as_str() {
        "" => Err("usage: clipboard <file.gif> | --save-dir | --font".to_string()),
        "--save-dir" => files::save_dir().map(|d| println!("{}", d.display())).map_err(|e| e.0),
        "--font" => match files::ui_font().map(ab_glyph::FontVec::try_from_vec) {
            Some(Ok(font)) => {
                println!("{} glyphs", font.glyph_count());
                Ok(())
            }
            Some(Err(e)) => Err(format!("the UI font does not parse: {e}")),
            None => Err("no UI font found".to_string()),
        },
        path => clipboard::copy_gif(std::path::Path::new(path)).map(|()| println!("copied {path}")).map_err(|e| e.0),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn main() {
    eprintln!("clipboard runs on macOS and Windows.");
}
