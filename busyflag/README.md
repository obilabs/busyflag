# Busyflag

A tiny tray app that turns a [Luxafor Flag](https://luxafor.com) **red while any
application is using your microphone** (optionally the camera) and **green when
you're free**. No per-app integrations: it asks the operating system directly.

Works on macOS, Windows 10/11 and Linux (including Raspberry Pi). Built with
[Tauri v2](https://tauri.app) and Rust; installers are a few megabytes.
Made by [Obilabs](https://github.com/obilabs). Open source under Apache 2.0.

## Features

- Free / busy colours, brightness, fade speed, all configurable
- Camera use optionally counts as busy
- Amber "away" colour while the screen is locked (busy still wins), or off if you prefer
- Pause (light off) and Force busy from the tray: one click forces for 30 min (configurable), or pick 5 min to 2 h or until cleared
- Hold time so a brief mic release doesn't flicker the light
- Ignore lists for apps and input devices
- Starts at login (toggle in the tray or Settings)
- Survives unplugging: reconnects when the flag comes back
- Admin-deployable defaults file for fleets, see `../docs/ENTERPRISE.md`
- Settings window with live status; config is a plain JSON file
- Tray icon uses shape as well as colour (dot, bar, moon, pause) so states are clear without colour vision
- Activity log: which app or device used the mic or camera, when, and for how long. Local only, with retention and Clear
- No network access, no telemetry

## Tray states

![Tray icon states](docs/tray-states.png)

Left to right: free, busy, away (screen locked), paused, and the hollow rings
shown when no Luxafor is connected. Each state has its own shape, so the light
and the icon are readable without colour vision.

## How detection works

| OS | Microphone | Camera |
|---|---|---|
| macOS | CoreAudio `kAudioDevicePropertyDeviceIsRunningSomewhere` on every input device (plus optional per-process attribution on macOS 14+) | CoreMediaIO `kCMIODevicePropertyDeviceIsRunningSomewhere` |
| Windows | `HKCU\...\CapabilityAccessManager\ConsentStore\microphone`, any app with `LastUsedTimeStop == 0` | same store, `webcam` |
| Linux | PipeWire/PulseAudio capture streams via `pactl` (monitor and corked streams excluded); ALSA `/proc/asound` fallback | any process holding `/dev/video*` open |

Screen lock: macOS `CGSessionCopyCurrentDictionary` (`CGSSessionScreenIsLocked`),
Windows `WTSQuerySessionInformation` session flags, Linux logind `LockedHint`
with an `org.freedesktop.ScreenSaver` D-Bus fallback.

None of these need microphone or camera permission: the app only reads state,
it never records.

Known limits: on older macOS versions some Bluetooth headsets were reported
never to show device-level activity (Apple developer forums, 2024). Verified
working with a Sony WH-CH710N on macOS 26.5; if a headset stays green for you,
enable "trust per-app audio state" in Settings, which uses the per-process
signal instead.
On Windows the consent store is only maintained while Settings > Privacy >
Microphone access is on.

## Building

Prerequisites: [Rust](https://rustup.rs) and the Tauri CLI:

```bash
cargo install tauri-cli --version "^2" --locked
```

Then, from `busyflag/`:

```bash
cargo tauri dev      # run with logs
cargo tauri build    # installers in src-tauri/target/release/bundle/
```

Set `RUST_LOG=debug` for verbose logs. The app also writes `busyflag.log` to the
platform log directory (path shown at the bottom of Settings).

### macOS
Xcode Command Line Tools. Distribution builds need a Developer ID certificate
and notarisation (Tauri's bundler handles both via `APPLE_*` env vars).

### Windows
Visual Studio Build Tools (C++ workload) and WebView2 (preinstalled on
Windows 11). The Luxafor is a generic HID device; no driver needed.

### Linux / Raspberry Pi
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev
```
Copy `linux/70-busyflag.rules` to `/etc/udev/rules.d/` and replug the flag
(the .deb does this for you). Microphone detection uses `pactl`, present on
Raspberry Pi OS Desktop; on Pi OS Lite install `pipewire-pulse` or rely on the
ALSA fallback. Autostart: add the app to `~/.config/autostart/` or run it from a
`systemd --user` service.

## Configuration

Stored as JSON in the platform config directory (shown in Settings):
macOS `~/Library/Application Support/com.busyflag.desktop/config.json`,
Windows `%APPDATA%\com.busyflag.desktop\config.json`,
Linux `~/.config/com.busyflag.desktop/config.json`.

```json
{
  "busy_colour": [255, 0, 0],
  "free_colour": [0, 255, 0],
  "paused_colour": [0, 0, 0],
  "locked_colour": [255, 170, 0],
  "lock_detection": true,
  "force_busy_default_minutes": 30,
  "test_duration_s": 3,
  "use_camera": false,
  "brightness": 100,
  "poll_interval_ms": 500,
  "busy_hold_ms": 2000,
  "fade_speed": 10,
  "ignore_devices": [],
  "ignore_apps": ["com.apple.CoreSpeech", "com.apple.assistantd", "com.apple.Siri", "com.apple.SiriNCService"],
  "process_level_detection": false
}
```

## Project layout

```
src/                  settings window (plain HTML/CSS/JS, no bundler)
src-tauri/src/
  lib.rs              Tauri setup and commands
  tray.rs             tray icon, menu, dynamic status dot
  manager.rs          polling loop and free/busy state machine
  light.rs            Luxafor Flag HID driver
  config.rs           config model, load/save
  detect/             per-OS microphone and camera detectors
linux/                udev rule
```

The Python prototype that validated the protocol and the macOS calls lives in
`../reference/python/`.

## Luxafor protocol

9-byte HID output report (byte 0 = report id 0): `01 LED R G B` static colour,
`02 LED R G B 00 SPEED` fade, `03 LED R G B SPEED 00 REPEAT` strobe,
`06 PATTERN REPEAT` built-in pattern. LED `0xFF` all, `0x41` front, `0x42` back,
`1..6` single LEDs. USB VID `04D8`, PID `F372`.

## License

Apache License 2.0. See `LICENSE` and `NOTICE`. Use it, change it, ship it;
just keep the notices. The Obilabs name and logo are not licensed.

Luxafor is a trademark of Greynut Ltd; this project is not affiliated.
