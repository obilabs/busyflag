# Busyflag: notes for the agent working on this repo

Tray app that turns a Luxafor Flag red when any app uses the microphone or
camera, amber when the screen is locked, green when free. Tauri v2 + Rust, one
codebase for macOS, Windows and Linux. Made by Obilabs, Apache-2.0.

Read first: README.md (front page), busyflag/README.md (features, detection,
building), docs/V1-TODO.md (what is left before 1.0), docs/RELEASING.md.

## Layout
- `busyflag/src-tauri/src/`: `lib.rs` setup and commands, `tray.rs` menu and
  glyphs, `manager.rs` state machine and activity log, `light.rs` HID driver,
  `config.rs`, `detect/{macos,windows,linux}.rs` per-OS detectors.
- `busyflag/src/`: plain HTML/CSS/JS settings page, no bundler.
- `.github/workflows/build.yml`: builds dmg+pkg, msi+exe, deb/rpm/AppImage,
  arm64 deb/rpm on every push; a `v*` tag creates a draft release
  (0.x = pre-release). Immutable releases are on: published tags can't change.
- `reference/python/`: the original prototype; the source of truth for the HID
  protocol and the macOS CoreAudio calls.

## Rules
- No network access, no telemetry, no mic/camera permission. Keep it that way;
  it is the product's trust story.
- Every OS detector must work with zero configuration and degrade gracefully.
- States need a shape as well as a colour (accessibility).
- Anything a mass deployment could trip on (crash, corrupted config, missing
  device, slow poll) gets a defensive fix and a line in docs/ENTERPRISE.md.
- Commits: author is the GitHub no-reply identity (repo git config is set);
  end messages with the Co-Authored-By trailer the harness provides.
- Never commit test logs or anything with usernames/paths; those live outside
  the repo in the owner's private folder.

## Verified so far
macOS 26.5 (built-in, Bluetooth headset and speakerphone mics, camera, lock,
unplug/replug, start at login) and Windows 11 (mic via Opera, camera via Edge,
lock, unplug/replug). Linux and Raspberry Pi: built, never run.

## Toolchain
Rust via rustup, `cargo install tauri-cli --version "^2" --locked`, then
`cargo tauri dev` / `cargo tauri build` from `busyflag/`. On the owner's Mac,
cargo lives in /opt/homebrew/opt/rustup/bin (not on the default PATH of tool
shells). Windows needs VS Build Tools (C++); Linux needs the apt list in the
app README.

## Status ritual
Update docs/V1-TODO.md checkboxes and push at the end of a session; that file
is the handoff between machines.
