# Inb: roadmap

Intent, not record. What is done lives in the code and the closed issues; this
file says what is next and why. Unchecked items are not done.

## Now

- [ ] In the pull request that adds the first `Cargo.toml`: run
      `copier update --data 'languages=[rust]'` (CodeQL fails on a tree with
      no Rust code), and add `cargo fmt --all --check`,
      `cargo clippy --all-targets -- -D warnings` and `cargo test` to
      `.claude/suite.json`, a `cargo` entry to `.github/dependabot.yml` and the
      cargo commands to `AGENTS.md`. Those three files are seeded, so the
      template answer does not change them.
- [ ] Open the frame with its buttons, and move and resize it.

## Next

- [ ] Record a part of the screen and save it as a GIF.

## Later

- [ ] Trim a recording before saving it.
- [ ] Linux. X11 works like the other two systems. On Wayland an app cannot
      place its own windows, and GNOME lacks the layer-shell protocol that
      allows it on KDE and Sway, so the frame needs another design there.
      Capture goes through the xdg-desktop-portal screen-share picker.

## Ideas

Not decided yet. An idea that is decided becomes an issue.
