# Busyflag — product overview for the Obilabs website

Status: v0.1.0 candidate, 2026-09-03. Written for whoever integrates this into
obilabs' site or marketing; every claim below is verified unless marked.

## One-line

Busyflag turns a Luxafor Flag USB light red while any app is using your
microphone or camera, amber while your screen is locked, and green when you're
free. No setup, no accounts, no network.

## Elevator pitch

People on calls get interrupted because a red light has to be switched on by
hand and nobody does. Busyflag watches the operating system instead of the
apps: the moment Teams, Zoom, Meet, Chrome, QuickTime or anything else opens
the microphone, the flag goes red, and it goes green when the mic is released.
It works with every meeting app because it never has to know about any of them.

## Feature list

- Free / busy / away colours, brightness and fade speed, all configurable
- Camera use optionally counts as busy
- Amber "away" while the screen is locked; busy still wins
- Force busy from the tray for 5 min to 2 h or until turned off (default 30 min)
- Pause (light off)
- Hold time so a brief mic release doesn't flicker the light
- Activity log: which app or device used the mic or camera, when, for how long;
  a plain CSV on the user's disk with retention, Clear and Export
- Tray shows the last five activity entries; Settings shows the full list
- Tray glyphs use shape as well as colour (dot, bar, moon, pause; hollow ring
  when the flag is unplugged) so states are readable without colour vision
- Starts at login; survives unplugging; one instance only
- Settings window with live status; config is a plain JSON file
- Admin-deployable defaults file for fleets; silent-install packages
- No network access, no telemetry, no microphone or camera permission needed

## Platforms and packages

| Platform | Package | Size | Status |
|---|---|---|---|
| macOS 12+ (Intel and Apple Silicon, one universal build) | .dmg and .pkg (for MDM) | ~5 MB | Verified on macOS 26.5 |
| Windows 10 (1903+) / 11 | .msi and setup .exe | ~5 MB | Verified on Windows 11 |
| Linux x86_64 | .deb, .rpm, AppImage | ~6 MB | Built, not yet run |
| Linux ARM64 / Raspberry Pi OS | .deb, .rpm | ~6 MB | Built, not yet run |

Builds come from GitHub Actions on every push; a `v*` tag creates a GitHub
Release with all installers attached.

Not yet signed: macOS shows a Gatekeeper warning and Windows a SmartScreen
warning until code signing is in place (see "Before public release").

## How it works (for the technical page)

- macOS: CoreAudio `kAudioDevicePropertyDeviceIsRunningSomewhere` on every
  input device, CoreMediaIO for cameras, `CGSessionCopyCurrentDictionary` for
  the lock state, plus the macOS 14+ per-process audio API to name the app.
- Windows: the CapabilityAccessManager consent store in the registry, where
  Windows records each app's microphone and webcam use; `WTSQuerySessionInformation` for the lock state.
- Linux: PipeWire / PulseAudio capture streams via `pactl`, ALSA fallback,
  `/dev/video*` open handles, logind `LockedHint`.
- Light: Luxafor Flag over USB HID (VID 04D8, PID F372), 9-byte reports.
- Stack: Tauri v2, Rust backend, plain HTML settings page. ~4 MB binary.

## Privacy statement (suggested wording)

Busyflag reads whether the microphone and camera are in use; it never records
audio or video and needs no microphone or camera permission. It makes no
network connections and sends nothing anywhere. The optional activity log is a
CSV file on your own computer that you can view, export, clear or turn off.

## Enterprise

- Silent install: `msiexec /i Busyflag.msi /qn`, `installer -pkg Busyflag.pkg -target /`, `apt install ./busyflag.deb`
- Managed defaults: a `defaults.json` at a machine-wide path seeds every user's settings
- Start at login uses each OS's native mechanism
- Logs: app log and activity CSV per user, paths shown in Settings
- Full detail: `docs/ENTERPRISE.md` in the repository

## Reporting problems

Tray → "Report a problem…" (or the link at the bottom of Settings) opens a
GitHub issue prefilled with version and OS. Users paste lines from the app log
(Settings → Activity → App log) and, for detection questions, attach the
activity export. Repository: https://github.com/obilabs/busyflag/issues

## Licence and credit

Apache 2.0. Copyright 2026 Obilabs. Luxafor is a trademark of Greynut Ltd; not
affiliated. Repository: https://github.com/obilabs/busyflag

## Before public release (open items)

1. Apple Developer ID signing and notarisation (CI is wired; needs the
   certificate secrets).
2. Windows code signing (Azure Trusted Signing or SignPath for open source).
3. Run on a Raspberry Pi and an x86_64 Linux desktop.
4. Tag `v0.1.0` to publish the first Release.
5. Screenshots of the tray menu and Settings for the site.

## Suggested website copy blocks

- Headline: "Your light knows when you're on a call."
- Sub: "Busyflag watches your mic, not your meeting app. Red when you're live, green when you're free, amber when you've stepped away."
- Three tiles: "Works with every app", "Nothing leaves your machine", "Fleet-ready in minutes".
- CTA: "Download for macOS, Windows or Linux" → GitHub Releases.
