# Inb: dependency policy and inventory

A decision record, not a lockfile mirror. Every direct dependency gets one line
saying why it is here. Removed or rejected candidates go under "Removed" with
the reason, so the same discussion does not happen twice.

## Policy

- Prefer the standard library, then what is already in the tree.
- A new dependency needs a reason a reviewer can check: what it does that the
  tree cannot reasonably do itself, and what it gets access to.
- Dependabot opens update PRs weekly (`.github/dependabot.yml`).

## Current inventory

| Crate | Licence | Why |
|---|---|---|
| `winit` 0.30 | Apache-2.0 | Native windows, input and displays on both systems |
| `tiny-skia` | BSD-3-Clause | Draws every window into a pixmap without a GPU stack |
| `ab_glyph` | Apache-2.0 | Rasterizes the bar's text from the system UI font |
| `gif` | MIT or Apache-2.0 | GIF encoding, with its own colour quantizer |
| `windows-capture` | MIT | Windows: DXGI Desktop Duplication |
| `windows` 0.62 | MIT or Apache-2.0 | Windows: layered windows, clipboard, known folders. Same version `windows-capture` uses |
| `objc2`, `objc2-*` 0.3, `block2`, `dispatch2` | MIT, Apache-2.0 or Zlib | macOS: AppKit windows, ScreenCaptureKit, the pasteboard. Builds without Xcode, so the macOS code is checked from any system |

winit 0.30 still uses objc2 0.5, so two objc2 versions are in the tree until
winit 0.31 is released. Inb passes only raw pointers between them.

## Rejected

| Candidate | Why |
|---|---|
| `gifski` | Better GIF quality, AGPL-3.0-or-later. Allowed under Inb's GPL-3.0 (GPL section 13); revisit if `gif` output looks poor |
| `softbuffer` | No per-pixel transparency on macOS or Windows; the frame's edges and the bar's corners need it, so each system's window module presents the pixmap itself |
| `screencapturekit` | Its build script compiles a Swift bridge, which needs Xcode; `objc2-screen-capture-kit` does not |
| `arboard`, `rfd` | Clipboard images paste only the first frame, and Save needs no dialog; a few lines per system do both |
| `scap` | Wraps both capture APIs, last release 0.0.7 in November 2024 |
| Tauri 2 | A web view per window; open reports of lost transparency on macOS and failing click-through |
| Electron | Ships a whole browser |

## Removed

| Dependency | Removed in | Why |
|---|---|---|
