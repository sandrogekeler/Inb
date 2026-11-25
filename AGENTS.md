# Inb

A screen recorder for macOS and Windows that saves GIFs

<!-- Project-owned. Seeded once from sandrogekeler/AllesSandro; rewrite freely.
The one agent file: Codex reads it directly, CLAUDE.md and GEMINI.md import it.
Keep it under ~200 lines, since it loads into every session. Put rules that
only matter for some files in .claude/rules/ with `paths:` frontmatter. -->

Rules shared with other projects are in `.claude/rules/`. An agent that does
not load that directory itself reads `working.md`, `writing.md` and
`simplicity.md` there before starting.

## Stack
- Rust, one codebase for macOS 13+ and Windows 10 version 2004+. No web view.
- Crates and why: `agent_docs/DEPENDENCIES.md`. Architecture and the options
  not taken: https://claude.ai/artifact/59JG4rGJEhVBsCVjkDXYmy

## Project structure

- `src/lib.rs`: everything below. Pure modules build and test on Linux too.
  - `spec.rs`: the settled design's colours, sizes and timings. Every value
    the drawing uses lives here.
  - `geometry.rs`: where each window goes, frame drags. `bar.rs`: where each
    control sits on the bar and the fps menu, for drawing and hit tests alike.
  - `session.rs`: the recording state machine. Events in, effects out.
  - `draw.rs`: a pixmap per window. `image.rs`: crop and scale a capture.
    `encode.rs`: spool frames to disk, write the GIF. `frame.rs`: one capture.
  - `platform/`: `mac/` and `win/`, each with `window`, `capture`,
    `clipboard` and `files` exposing the same functions.
- `src/main.rs`, `src/app.rs`, `src/app/recording.rs`: the binary, the event
  loop and the effects (capture, encode, copy, save threads).
- `examples/`: `capture` and `clipboard` try one platform part on its own;
  `encode` encodes a synthetic 30 s recording.
- `docs/testing.md`: how to try a pull request on Windows and macOS.
- `site/`: the download page, plain HTML with no build step. Its demo reads
  sizes, colours and timings from `spec.rs` and `bar.rs` by hand: change both together.
- `agent_docs/`: the roadmap, the health and security checklists and the
  dependency inventory.

## Architecture rules

- One codebase runs on macOS and Windows.
- Inb only records what the user selects, and only while recording.
- No Inb window is ever inside the recorded area. The frame is four edge
  windows around it, and the buttons are attached outside its bottom-right
  corner. Recording crops the display to the inside of the frame.
- Capture uses each system's own API: ScreenCaptureKit on macOS, DXGI Desktop
  Duplication on Windows (Windows.Graphics.Capture draws a yellow border).
- Code for one system lives in `src/platform/<system>/`. The rest of Inb
  calls it through `platform::sys` and never names a system, so Linux can be
  added later. Only `platform/` and `app` use `cfg(target_os)`.
- Coordinates are logical points, desktop-relative, until `draw` and
  `platform` turn them into physical pixels. The GIF is made at point size.
- Every `unsafe` block carries a `// SAFETY:` comment.

## Build & dev commands

```sh
cargo run                        # Inb itself, on macOS or Windows
cargo test                       # unit tests, also on Linux
cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cargo clippy --all-targets --target aarch64-apple-darwin -- -D warnings
python3 .claude/suite-check.py   # every health check in .claude/suite.json
```

The two cross-target clippy runs check platform code from any system (add the
targets with `rustup target add`); they compile without linking. CI
(`.github/workflows/ci.yml`) builds, lints and tests on `windows-latest` and
`macos-latest` and uploads both builds. Run the last command before calling a
task done. A check it reports as skipped did not pass.

## Task tracking
Work is tracked in GitHub Issues. Every issue carries a `type:`, an `area:` and
exactly one priority (`p0`-`p3`) label. The issue forms ask for it and the
`issue-priority` workflow applies it; an issue opened through the API (by an
agent) sets the label in the same call, `p2` when unclear. Close issues from
the pull request with `Fixes #N`.

## Pull requests

Follow `CONTRIBUTING.md`: one pull request per piece of work, a `## Changelog`
line per change, an `area:` label.

## Files that come from the template

Some files are owned by sandrogekeler/AllesSandro and must not be edited here:
`.claude/rules/template-files.md` lists them.

## Docs

Describe what is on disk, in the present tense. Intent goes in
`agent_docs/ROADMAP.md` as an unchecked item. Unsure whether something landed?
Check before writing it down.

## Do not

- Commit secrets, `.env` files or credentials.
- Edit generated files or lockfiles by hand; regenerate them.
- Add a dependency without reading `agent_docs/DEPENDENCIES.md`.
