# Windows test plan (first run on the Windows PC)

## 0. Getting a Windows build

There is no way to build the Windows version on the Mac (the HID library needs
the Windows SDK), so pick one:

- **Build on the PC** (section 1 below). About 30 minutes of installs the first time,
  then a couple of minutes per build.
- **Let GitHub build it**: push this folder to a GitHub repository; the workflow in
  `.github/workflows/build.yml` produces the Windows `.msi` and `.exe` installers as
  downloadable artifacts under the Actions tab, plus macOS and Linux builds.
  Download the `busyflag-windows-x86_64` artifact on the PC and run the installer.

## 1. Build it on the PC

Install, in this order:

1. **Visual Studio Build Tools 2022**: choose the "Desktop development with C++" workload
   (https://visualstudio.microsoft.com/visual-cpp-build-tools/).
2. **Rust** via https://rustup.rs (accept defaults; it uses the MSVC toolchain above).
3. WebView2 is already on Windows 11; on Windows 10 install the Evergreen runtime
   (https://developer.microsoft.com/microsoft-edge/webview2/).
4. Tauri CLI, from a fresh terminal:
   ```
   cargo install tauri-cli --version "^2" --locked
   ```
5. Copy the `luxafor` folder to the PC (or clone it once it is on GitHub), then:
   ```
   cd luxafor\busyflag
   cargo tauri dev
   ```
   `dev` keeps a console open so you see the log lines. `cargo tauri build` makes
   the installer under `src-tauri\target\release\bundle\msi\` and `...\nsis\`.

The Luxafor Flag needs no driver on Windows. If the tray dot is a hollow ring the
app can't open the device: check it isn't held by Luxafor's own software.

## 2. What to check

| Check | Expected | Notes |
|---|---|---|
| Tray icon appears, menu opens on left and right click | Green dot, status "Free" | Windows may hide it in the overflow chevron; drag it out |
| Voice Recorder app: record | Red within about a second, app name in the status line | Tests a packaged (Store) app |
| Zoom / Teams / Chrome meet: join with mic on | Red, app name in status | Teams may show as `MSTeams` or `ms-teams.exe` |
| Mute in Teams | Stays red | Muting doesn't release the mic; that is expected |
| Leave the call | Green after the 2 s hold | Note how long the registry takes to update |
| Two apps use the mic, close one | Stays red until both stop | |
| Enable "Camera counts as busy", open Camera app | Red | |
| Lock the PC (Win+L), unlock | Amber while locked, then green | Busy still wins while locked |
| Settings window: change busy colour, Save | Light changes immediately | |
| Unplug the flag, replug | Tray ring while unplugged, dot again within 2 s | |
| Launch a second copy | Just brings up Settings | |
| Settings > Privacy > Microphone access **off** | Detection stops working | Known limitation of the consent store |

## 3. If detection never triggers

Open `regedit` at
`HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone`
while recording. Some subkey (or one under `NonPackaged`) should show
`LastUsedTimeStop = 0`. If nothing there changes, the OS isn't maintaining the
store and we need the WASAPI session fallback. Send the key names you see for
the apps you care about; they feed the ignore list and the friendly names.

## 4. Sending logs back

The app writes `busyflag.log` to
`%LOCALAPPDATA%\com.busyflag.desktop\logs\` (Settings > "Show in folder" next
to Log opens it). Every free/busy change is logged with the app names seen, so
a good test log is: start the app, do the checks above in order, then copy the
file. Copy it into the `luxafor\docs\` folder as `windows-test-1.log` (or
paste it into the chat) and it can be reviewed from the Mac.

For more detail run `set RUST_LOG=debug` before `cargo tauri dev`.

## Result of test 1 (2026-09-03, Windows 11)

All checks passed on the first build. Log and CSV are in `test-results/`.
Mic use by Opera and camera use by Edge were detected within a second, the
lock showed amber, unplugging the flag was noticed on the next write and the
flag reconnected 14 s later on its own. Follow-ups made afterwards: executables
now show product names (Opera, Microsoft Edge) instead of `opera.exe`, and the
app log uses local time like the CSV.
