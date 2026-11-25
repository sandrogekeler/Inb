# Trying Inb on Windows and macOS

Every CI run, on main and on each pull request, builds Inb on both systems.
Two ways to try a build.

## Without Rust: the CI build

1. Open the newest **CI** run under the repository's **Actions** tab (or a
   pull request's **Checks**), then **Summary**. Under **Artifacts**,
   download `inb-Windows` or `inb-macOS`.
2. Windows: unzip and start `inb.exe`. SmartScreen warns because the build is
   not signed: **More info**, then **Run anyway**.
3. macOS: unzip, then in Terminal in that folder:
   `xattr -d com.apple.quarantine inb && chmod +x inb && ./inb`.
   The first recording asks for Screen Recording permission for Terminal.
   Allow it in System Settings, quit Terminal and start again.

## With Rust: from the branch

Install Rust from <https://rustup.rs> once. Windows also needs the Visual
Studio C++ Build Tools, which the installer offers. Then:

```sh
git fetch origin
git switch <branch from the pull request>
cargo run --release
```

Each pull request lists its own steps under "How it was verified": what to
click and what should happen. Examples try one part on its own:

```sh
cargo run --release --example capture -- 100 100 640 400 5 15   # writes target/capture/*.png
cargo run --example clipboard -- target/encode.gif              # then paste somewhere
```
