# Releasing Busyflag

## One-time setup: code signing

### macOS (Developer ID + notarisation)

1. Enrol in the Apple Developer Program (US$99/year) at https://developer.apple.com/programs/enroll/.
   Enrolling as an organisation (Obilabs) needs a D-U-N-S number and can take
   one to two weeks; enrolling as an individual is usually a day.
2. In Xcode (Settings → Accounts → Manage Certificates) or at
   https://developer.apple.com/account/resources/certificates, create two
   certificates: **Developer ID Application** and **Developer ID Installer**.
3. Export both from Keychain Access as one `.p12` with a password.
4. Create an app-specific password for your Apple ID at https://appleid.apple.com
   (Sign-In and Security → App-Specific Passwords).
5. Add these repository secrets (GitHub → Settings → Secrets and variables → Actions):

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | the `.p12` file, base64-encoded: `base64 -i certs.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | the password you chose when exporting |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Obilabs (TEAMID)` exactly as Keychain shows it |
| `APPLE_INSTALLER_IDENTITY` | `Developer ID Installer: Obilabs (TEAMID)` |
| `APPLE_ID` | the Apple ID email |
| `APPLE_PASSWORD` | the app-specific password |
| `APPLE_TEAM_ID` | the 10-character team ID |

The workflow already picks these up: it signs the app, notarises it with
Apple, staples the ticket, and signs the `.pkg`. Nothing else to change.

### Windows

Option A, free for open source: apply at https://signpath.org/ (SignPath
Foundation) with the repository link; approval takes a few weeks, then a
GitHub Action step signs the msi and exe.
Option B, ~US$10/month: Azure Trusted Signing, which Tauri supports through a
`signCommand` in `tauri.conf.json`.
Until one is in place, Windows shows a SmartScreen warning that users click
through with "More info → Run anyway".

### Linux

No signing needed. A GPG-signed apt repository can come later.

## Repository settings (once)

- Settings → General → Releases: enable **Immutable releases** so a published
  release's tag and assets can never be altered or deleted.
- Settings → Tags: optionally add a protection rule for `v*` so only admins can
  create release tags.

## Cutting a release

1. Bump the version in `busyflag/src-tauri/tauri.conf.json` and
   `busyflag/src-tauri/Cargo.toml` (keep them equal), commit, push, wait for CI
   to go green.
2. Tag and push:
   ```
   git tag -a v1.0.0 -m "Busyflag 1.0.0"
   git push origin v1.0.0
   ```
3. The workflow builds all platforms and creates a **draft** GitHub Release
   with the installers attached and auto-generated notes. Review the notes,
   then click Publish. With immutable releases on, that's the point of no
   return for the tag and its assets.
4. Announce: link to https://github.com/obilabs/busyflag/releases/latest.

## Version policy

Semantic versioning. 1.0.0 is the first stable release. Patch releases for
fixes, minor for features, major for anything that changes config format or
platform support.
