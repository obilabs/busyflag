# Deploying Busyflag in an organisation

Busyflag is a per-user tray app. It needs a logged-in desktop session (that is
where the tray and, on Windows, the per-user microphone consent store live), so
it starts at login, not at boot.

## What it does and does not do

- Reads OS state only: "is any app capturing from the microphone / camera",
  "is the session locked". It never opens the microphone or camera itself and
  needs no microphone or camera permission.
- Talks to the Luxafor Flag over USB HID. No drivers on macOS or Windows; a
  udev rule on Linux (installed by the .deb).
- Makes no network connections. There is no telemetry, no update check, no
  crash reporting. Updates are whatever your software distribution does.
- Writes three things in the user's profile: a JSON config, a small log file,
  and an activity log (which app or device used the mic or camera, when, for how
  long). The activity log is on by default, has a retention setting (30 days),
  can be cleared by the user, and can be disabled fleet-wide with
  `"activity_log": false` in the managed defaults file.

## Silent installation

| OS | Package | Silent install |
|---|---|---|
| Windows | `Busyflag_x.y.z_x64_en-US.msi` | `msiexec /i Busyflag.msi /qn` |
| Windows | `Busyflag_x.y.z_x64-setup.exe` (NSIS) | `Busyflag-setup.exe /S` |
| macOS | `Busyflag_x.y.z_universal.pkg` | `sudo installer -pkg Busyflag.pkg -target /`, or push through Jamf / Intune / Kandji |
| macOS | `Busyflag_x.y.z_universal.dmg` | for people installing by hand |
| Debian / Raspberry Pi OS | `busyflag_x.y.z_amd64.deb` / `_arm64.deb` | `apt install ./busyflag_*.deb` |
| Fedora / RHEL | `busyflag-x.y.z.x86_64.rpm` | `dnf install ./busyflag-*.rpm` |
| Any Linux | `busyflag_x.y.z_amd64.AppImage` | copy anywhere, mark executable, add the udev rule from `linux/` |

## Managed defaults

Drop a JSON file at the path below and every user on the machine starts from
it. Keys are the same as the user config; you only need to include the keys
you want to change. Users can still override them in their own config, so
this is "defaults", not "policy".

| OS | Path |
|---|---|
| macOS | `/Library/Application Support/Busyflag/defaults.json` |
| Windows | `%ProgramData%\Busyflag\defaults.json` |
| Linux | `/etc/busyflag/defaults.json` |

Example, a company that wants camera use to count, a softer busy colour and a
five-minute force-busy default:

```json
{
  "use_camera": true,
  "busy_colour": [220, 40, 40],
  "force_busy_default_minutes": 5
}
```

Precedence, lowest to highest: built-in defaults, this file, the user's config.

## Start at login

Enabled automatically on the first run for each user and toggled from the tray
menu or Settings. Mechanism per OS: a LaunchAgent in `~/Library/LaunchAgents`
on macOS, `HKCU\...\Run` on Windows, `~/.config/autostart/*.desktop` on Linux.
To pre-seed it fleet-wide, deploy the same entry with your management tool; the
app detects an existing entry and shows the toggle as on.

## Files written per user

| | Config | Log |
|---|---|---|
| macOS | `~/Library/Application Support/com.busyflag.desktop/config.json` | `~/Library/Logs/com.busyflag.desktop/busyflag.log` |
| Windows | `%APPDATA%\com.busyflag.desktop\config.json` | `%LOCALAPPDATA%\com.busyflag.desktop\logs\busyflag.log` |
| Linux | `~/.config/com.busyflag.desktop/config.json` | `~/.local/share/com.busyflag.desktop/logs/busyflag.log` |

The log rotates itself at 2 MB and contains state changes with the names of
the apps that held the microphone. The activity log (`activity.csv`, next to
the config) holds the same information as a per-app history. Treat both as
mildly sensitive personal data under your retention policy.

## Windows privacy setting

Detection relies on Windows maintaining the microphone consent store, which it
only does while Settings > Privacy & security > Microphone > "Microphone
access" is on. Group Policy `LetAppsAccessMicrophone` set to "force deny" will
also stop detection.

## Before a wide rollout

Things that make a mass deployment noisy, and where Busyflag stands:

| Concern | Status |
|---|---|
| App silently disappears on an internal error | The detector loop restarts itself after a crash and every panic is written to the log. |
| A corrupted config wipes the user's settings | An unparseable config is moved aside as `config.broken-<time>.json`, never overwritten. |
| CPU or process churn on low-end hardware | macOS and Windows use in-process API calls. Linux runs one `pactl` per poll with a 2 s time limit and caches the monitor list for 30 s. |
| Wrong state after sleep or USB hiccups | Presence is checked every 2 s, so an unplugged flag is noticed within 2 s even with no colour change; "not connected" is shown after 3 s missing; reconnect is automatic within 2 s of replug. |
| Support tickets without context | Version is shown in Settings and logged at start; log path is one click away. |
| Colour-blind users | Each tray state has a distinct shape as well as a colour. |
| Gatekeeper / SmartScreen warnings | Sign and notarise macOS builds (Developer ID) and sign Windows installers before deploying. The CI workflow signs macOS builds when the `APPLE_*` secrets are set. |
| Leftovers after uninstall | Removing the app leaves the per-user config, log and login item. Delete the paths above and `~/Library/LaunchAgents/Busyflag.plist` (macOS) or the `Run` key entry (Windows) in your uninstall script. |
| Localisation | English only for now. |

Verified on macOS 26.5: built-in mic, Bluetooth headset mic (Sony WH-CH710N),
Bluetooth speakerphone mic (Mifa A10), camera, screen lock and unlock,
unplug/replug, second launch, start at login.
Verified on Windows 11 (2026-09-03): install
from the msi, flag connected, start at login, microphone via Opera, camera via
Edge, two apps overlapping, lock and unlock, unplug and replug with automatic
reconnect, activity CSV export.
Not yet verified: Linux at runtime, USB microphones, aggregate devices,
multi-user machines.
