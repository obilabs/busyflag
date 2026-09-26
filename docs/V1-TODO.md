# Road to Busyflag 1.0.0

**Feature-frozen 2026-09-26.** Busyflag is finished as a product; remaining
work is signing, packaging and bug fixes only. 0.5.0 is the shipped pre-release.

0.5.0 is out as an unsigned pre-release. 1.0.0 is the signed, stable release.

## Must

- [ ] Obilabs mailbox on obilabs.dev; Apple ID on it; Apple Developer Program enrolment (organisation if incorporated, needs D-U-N-S)
- [ ] Developer ID Application + Installer certificates; seven `APPLE_*` repository secrets (see RELEASING.md)
- [ ] Confirm a CI build signs and notarises (Gatekeeper opens the dmg with no warning)
- [ ] Windows signing: SignPath Foundation application (free, OSS) or Azure Trusted Signing
- [ ] Enable "Immutable releases" in the repository settings
- [ ] Run on a Raspberry Pi (deb, udev rule, PipeWire detection, tray on the Pi desktop)
- [x] Run on an x86_64 Linux desktop (deb on Ubuntu 24.04; AppImage untested)
- [ ] Fix whatever 0.5.0 users report

## Should
- [ ] Linux: verify screen lock on GNOME/Wayland (pactl note is in INSTALL.md)

- [ ] Screenshots of tray menu and Settings in the README and on the website
- [ ] "About Busyflag" tray item with version and Obilabs link
- [ ] Test a USB microphone and an aggregate device on macOS
- [ ] Windows: verify product names (Opera, Microsoft Edge) and local-time log in the new build
- [ ] Multi-user machine: two accounts logged in, one flag

## Could

- [ ] Homebrew cask and winget manifest after signing
- [ ] Luxafor Orb / Bluetooth support if the protocol matches (unverified)
- [ ] Idle detection (no input for N minutes) as a second "away" trigger
- [ ] Localisation
