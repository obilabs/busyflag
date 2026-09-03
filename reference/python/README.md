# Python reference implementation

These scripts were the first working prototype (macOS, 2026-09-02) and remain the
reference for the Luxafor HID protocol and the macOS CoreAudio / CoreMediaIO detection
calls that the Rust code in `busyflag/src-tauri/src` ports one-to-one.

    pip3 install --user hidapi
    python3 luxafor.py demo            # colours, strobe, fade
    python3 micmon_macos.py            # print mic in-use changes
    python3 cammon_macos.py            # print camera in-use changes
    python3 busylight.py --cam         # red when busy, green when free
