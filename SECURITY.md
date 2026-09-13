# Security Policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

Report privately through either channel:

- **GitHub private vulnerability reporting** — on this repository, go to
  **Security → Report a vulnerability**.
- **Email** — security@obilabs.dev

Include what you found, how to reproduce it, the Busyflag version and your
operating system. We aim to acknowledge reports within a few business days and
will keep you updated while we work on a fix. We are happy to credit reporters
once a fix ships, unless you prefer to stay anonymous.

## Scope

Busyflag is a local tray app. It makes no network connections, has no
telemetry, and never asks for microphone or camera permission. Reports that
break those guarantees are especially welcome, as are issues such as:

- the settings window loading or executing content it should not;
- config, admin-defaults or activity-log handling that lets another local user
  or process escalate privileges or tamper with the app;
- installer, udev rule or login-item behaviour that grants more access than
  needed.

## Supported versions

Busyflag is pre-1.0. Fixes land on `main` and ship in the next release. Release
builds are currently unsigned, so only install builds downloaded from this
repository's Releases page.
