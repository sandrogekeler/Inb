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

No code yet. Chosen for the first build:

| Crate | Licence | Why |
|---|---|---|
| `winit` | Apache-2.0 | Native windows and input for the frame and the buttons |
| `softbuffer`, `tiny-skia` | to check | Draw the border and the buttons without a GPU stack |
| `screencapturekit` | MIT or Apache-2.0 | Screen capture on macOS |
| `windows-capture` | MIT | Screen capture on Windows through DXGI Desktop Duplication |
| `gif` | MIT or Apache-2.0 | GIF encoding, with its own colour quantizer |

## Rejected

| Candidate | Why |
|---|---|
| `gifski` | Better GIF quality, AGPL-3.0-or-later. Allowed under Inb's GPL-3.0 (GPL section 13); revisit if `gif` output looks poor |
| `scap` | Wraps both capture APIs, last release 0.0.7 in November 2024 |
| Tauri 2 | A web view per window; open reports of lost transparency on macOS and failing click-through |
| Electron | Ships a whole browser |

## Removed

| Dependency | Removed in | Why |
|---|---|---|
