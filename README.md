# Busyflag

**Your light knows when you're on a call.** Busyflag turns a
[Luxafor Flag](https://luxafor.com) red while any app is using your microphone
or camera, amber while your screen is locked, and green when you're free. It
watches the operating system, not your meeting app, so it works with Teams,
Zoom, Meet, Chrome, Slack and everything else, with no setup.

![Tray icon states](busyflag/docs/tray-states.png)

- macOS, Windows and Linux (including Raspberry Pi), one small app each
- No accounts, no network, no telemetry; never needs microphone or camera permission
- Force busy for a while, pause, camera as busy, hold time, custom colours
- Activity log of what used your mic or camera and for how long, as a CSV you own
- Starts at login, survives unplugging, colour-blind-friendly tray glyphs
- Fleet-ready: silent installers and an admin defaults file

## Download

Installers for every platform are on the
[Releases page](https://github.com/obilabs/busyflag/releases). Current builds
are unsigned pre-releases; macOS and Windows will warn on first launch until
signing lands in 1.0.

## Documentation

- [App README](busyflag/README.md): features, how detection works, building from source, configuration
- [Deploying in an organisation](docs/ENTERPRISE.md): silent install, managed defaults, logs, rollout checklist
- [Releasing](docs/RELEASING.md) and the [road to 1.0](docs/V1-TODO.md)
- [Research notes](docs/RESEARCH.md) and the [Python prototype](reference/python/) that validated the protocol

## Reporting a problem

Tray → "Report a problem…" opens a prefilled issue. Questions and ideas go to
[Discussions](https://github.com/obilabs/busyflag/discussions).

## License

Apache 2.0, copyright 2026 [Obilabs](https://github.com/obilabs). Luxafor is a
trademark of Greynut Ltd; this project is not affiliated.
