# Installing Busyflag

Download from the [Releases page](https://github.com/obilabs/busyflag/releases).
Current builds are unsigned, so macOS and Windows warn once on first launch.

## macOS (Intel and Apple Silicon, one universal build)
1. Open `Busyflag_x.y.z_universal.dmg` and drag Busyflag to Applications.
2. First launch: right-click Busyflag in Applications → Open → Open. If macOS
   still refuses, System Settings → Privacy & Security → scroll down →
   "Open Anyway".
3. The green dot appears in the menu bar. Busyflag adds itself to login items.
For fleet deployment use the `.pkg`: `sudo installer -pkg Busyflag_x.y.z_universal.pkg -target /`.

## Windows 10 (1903 or later) and 11
1. Run `Busyflag_x.y.z_x64_en-US.msi` (or the `-setup.exe`).
2. SmartScreen: "More info" → "Run anyway".
3. The green dot appears in the tray, possibly under the overflow chevron; drag
   it out to keep it visible. Busyflag adds itself to start at login.
Silent install: `msiexec /i Busyflag.msi /qn`. No driver is needed for the flag.
Detection relies on Settings → Privacy & security → Microphone → "Microphone
access" being on.

## Linux
### Debian, Ubuntu, Raspberry Pi OS
```
sudo apt install ./busyflag_x.y.z_amd64.deb      # or _arm64.deb on the Pi
```
Then **unplug and replug the flag** so the udev rule installed by the package
takes effect (it grants your user access to the device without root).
Start Busyflag from the app menu; it adds itself to autostart.

### Fedora, RHEL
```
sudo dnf install ./busyflag-x.y.z.x86_64.rpm
```
Replug the flag as above.

### AppImage (any distribution)
Mark it executable and run it. The udev rule is not installed for you: copy
`linux/70-busyflag.rules` from the repository to `/etc/udev/rules.d/`, run
`sudo udevadm control --reload`, and replug the flag.

### Notes
- If the tray status shows a sound-card name such as "ALSA card0/pcm0c"
  instead of the app using the microphone, the PulseAudio/PipeWire command-line
  tools are missing: `sudo apt install pulseaudio-utils` (Fedora:
  `pulseaudio-utils` or `pipewire-utils`). Detection works either way.
- GNOME needs the AppIndicator extension for tray icons; KDE, XFCE, Cinnamon
  and Raspberry Pi OS show them out of the box.
- Something not right? `sh linux/collect-diagnostics.sh 60` (in the
  repository) writes a redacted diagnostics file to attach to an issue.

## Uninstall leftovers
Removing the app leaves the per-user config, logs and login item; paths are
listed in `ENTERPRISE.md`.
