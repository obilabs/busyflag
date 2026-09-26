# Linux test plan

Same checks as `WINDOWS-TESTING.md`: install, flag found, mic via a recorder or
a browser meeting page, camera via Cheese or a browser page, lock/unlock,
unplug/replug, second launch, Settings save, Export CSV. Collect evidence with:

```
sh busyflag/linux/collect-diagnostics.sh 60
```

It snapshots the system, USB/HID access, udev rule, sound server, lock state
and Busyflag's log, then samples mic/camera state every 2 s for 60 s while you
test. Names and home paths are redacted; read it before sharing.

## Result of test 1 (Ubuntu 24.04 live session, x86_64, 2026-09)
Passed: .deb install, udev rule (user could open the device), flag connected,
start at login, microphone detection (ALSA fallback, because the live image
lacks `pactl`), camera held by PipeWire, unplug noticed within 2 s. A camera
app opening the microphone is reported as mic activity, which is correct.
Not exercised: screen lock, app-name attribution. Diagnostics kept privately.
