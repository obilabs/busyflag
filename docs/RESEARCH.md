# Luxafor free/busy light: research and product decision

Date: 2026-09-02. Dev machine: MacBook Pro, macOS 26.5.2, system Python 3.9.
Targets: macOS, Windows 10/11, Linux (Raspberry Pi OS Bookworm/Trixie).

## What is verified on the Mac today

| Piece | File | Status |
|---|---|---|
| Luxafor Flag driver (colour, front/back split, fade, hardware strobe, patterns) | `luxafor.py` | Working via `hidapi` pip package |
| Mic-in-use detection (any app) | `micmon_macos.py` | Working: CoreAudio `kAudioDevicePropertyDeviceIsRunningSomewhere` via ctypes, no extra deps, no mic permission needed |
| Camera-in-use detection (any app) | `cammon_macos.py` | Working: CoreMediaIO `kCMIODevicePropertyDeviceIsRunningSomewhere` via ctypes |
| Free/busy loop (mic busy = red, idle = green) | `busylight.py` | Working, tested with QuickTime audio recording |
| Per-process attribution (which app has the mic) | probe only | macOS 14+ process-object API works on 26.5; needs a filter for system listeners such as Siri's `com.apple.CoreSpeech`, which reports input running while idle |

The existing `set_luxafor.py` uses pyusb. That cannot claim a HID device on macOS because the kernel HID driver owns it, so it is superseded by `luxafor.py`.

## Mic / camera detection per platform

### macOS (done)
- Device-level "running somewhere" is the approach used by Objective-See's OverSight, so it is battle tested. Polling every 0.5 s is cheap; a listener callback (`AudioObjectAddPropertyListener`) is possible later to avoid polling.
- Known gotcha: Bluetooth headset mics may never report "running somewhere" (Apple developer forum thread 741026, unresolved as of late 2024). Mitigation: also consult the per-process API (`kAudioHardwarePropertyProcessObjectList` + `kAudioProcessPropertyIsRunningInput`) and ignore an allowlist of system bundle IDs. Needs testing with a Bluetooth headset.
- Aggregate devices and macOS 26 specifics: untested by anyone we found. Our test on 26.5 works for the built-in mic.
- Read-only property queries do not trigger the mic permission prompt. Do not add `NSMicrophoneUsageDescription` to the app; it is not needed.

### Windows 10 (1903+) / 11
- Primary: registry `HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\*` and `...\microphone\NonPackaged\*`. Any subkey with `LastUsedTimeStop == 0` means that app is capturing right now. Camera is the same under `...\ConsentStore\webcam`. Used by LuxaforOnAir, hass-workstation-service, busylight-evaristorivi.
- Events instead of polling: `RegNotifyChangeKeyValue` via pywin32. Stdlib `winreg` cannot do this; 1 s polling is fine to start.
- Caveat: only populated when Settings > Privacy > Microphone access is on. A WASAPI session check (pycaw) can be an optional cross-check, but one project reports it misses some Windows 11 apps.
- To verify on your Windows box: latency of the registry update, and behaviour on 24H2/25H2 (no breakage reports found, but not independently verified).

### Linux / Raspberry Pi
- Pi OS Bookworm and later use PipeWire with the PulseAudio compatibility layer, so `pactl list source-outputs` lists active capture streams. Exclude streams whose source name ends in `.monitor` and streams that are corked. Pi OS Lite does not install PipeWire; the ALSA fallback is `grep RUNNING /proc/asound/card*/pcm*c/sub*/status`.
- Python: `pulsectl` gives event callbacks on `source_output` new/remove, so no polling. Camera: inotify or `fuser` on `/dev/video*`, debounced.
- Luxafor needs a udev rule for non-root hidraw access, in `/etc/udev/rules.d/70-luxafor.rules`:
  `KERNEL=="hidraw*", ATTRS{idVendor}=="04d8", ATTRS{idProduct}=="f372", TAG+="uaccess"`
  (use `GROUP="plugdev", MODE="0660"` instead of uaccess if the app runs headless over SSH). 64-bit Pi OS gets a prebuilt aarch64 `hidapi` wheel; 32-bit builds from source and needs `python3-dev libusb-1.0-0-dev libudev-dev`.

## Decision (2026-09-03)

The user wants a lightweight, polished, open-source product to distribute, so the final choice is **Tauri v2 + Rust** (see `busyflag/`). The Python prototype below stays as the reference implementation in `reference/python/`. The section that follows was the pre-decision comparison.

## Stack recommendation (superseded)

Recommendation: **Python, one codebase, `hidapi` for the light, per-OS detector modules, PySide6 `QSystemTrayIcon` for the tray.**

Why Python: the hard parts (HID protocol, three OS-specific detectors) are already proven or well documented in Python, iteration is fastest, and the Pi is a first-class target. Node and Rust are not installed here, and a Tauri/Electron app would still need the same native detection code per OS.

Tray options considered:
- PySide6 `QSystemTrayIcon`: actively maintained, works on current macOS, Windows, and Linux. Cost: large dependency (roughly 150 MB) and on the Pi it must come from the Debian package (`python3-pyside6.qtwidgets` on Trixie) because PyPI has no aarch64 Linux wheels. Recommended for macOS and Windows.
- pystray: tiny, fine on Windows and Linux (GTK/AppIndicator), but unmaintained since 2023 with open macOS bugs (missing icons on 14.x, detached mode broken on Apple Silicon). Reasonable Pi-only fallback if PySide6 is too heavy there.
- rumps: macOS only, so it would fragment the UI code.

Keep the tray layer behind a small interface so the Pi can use pystray if PySide6 proves too heavy there.

Packaging: PyInstaller (or Briefcase) on macOS and Windows. macOS Sequoia and later require Developer ID signing plus notarisation for a smooth install; unsigned builds need the "Open Anyway" dance in System Settings. On the Pi: a venv plus a `systemd --user` service for autostart. Install a newer Python via Homebrew for building the Mac app rather than shipping against the system 3.9.

## Configurable manager (goal 3) design notes
- Config file (TOML or JSON) in the user config dir: busy colour, free colour, off-hours/idle colour, poll interval, whether camera counts as busy, per-device ignore list (e.g. ignore a specific input device), optional app allow/deny list on platforms that can attribute usage (macOS process API, Windows registry key names).
- Tray menu: current state, pause/resume, force busy, colour test, open config, quit.
- State machine: free / busy / paused / disconnected (light unplugged: retry on hotplug).

## Projects to borrow from
- LuxaforOnAir (Windows, C#): registry walk reference implementation.
- OverSight (macOS, ObjC): CoreAudio/CMIO listener reference.
- JnyJny/busylight (Python): mature multi-vendor light driver and CLI; useful if we later want to support lights other than Luxafor.
- busylight-evaristorivi (Python, cross-platform): Windows registry approach; its macOS approach scrapes Control Center and should be avoided.

## Open items to verify on the other machines
1. Windows: registry latency, 24H2/25H2 behaviour, packaged (Store) apps such as Teams appearing under package keys.
2. Pi: PipeWire vs ALSA path depending on OS image, udev rule, PySide6 availability and memory footprint.
3. macOS: Bluetooth headset mic, aggregate devices, Teams and Chrome meetings.
