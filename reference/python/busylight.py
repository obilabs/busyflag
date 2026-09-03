#!/usr/bin/env python3
"""Proof-of-concept free/busy loop: mic in use -> red, idle -> green.

Usage: python3 busylight.py [seconds_to_run] [--cam]
Ctrl-C to stop; light is turned off on exit.
"""
import sys
import time

from luxafor import Luxafor, COLOURS

if sys.platform == "darwin":
    from micmon_macos import mic_in_use, active_input_devices
    from cammon_macos import camera_in_use, active_cameras
else:
    raise SystemExit("only the macOS detector exists so far")

BUSY = COLOURS["red"]
FREE = COLOURS["green"]


def main():
    duration = float(sys.argv[1]) if len(sys.argv) > 1 and not sys.argv[1].startswith("-") else 1e9
    use_cam = "--cam" in sys.argv
    last = None
    t0 = time.time()
    with Luxafor() as lux:
        try:
            while time.time() - t0 < duration:
                busy = mic_in_use() or (use_cam and camera_in_use())
                if busy != last:
                    lux.fade(BUSY if busy else FREE, speed=10)
                    who = active_input_devices() + (active_cameras() if use_cam else []) if busy else ""
                    print(time.strftime("%H:%M:%S"), "BUSY" if busy else "FREE", who)
                    last = busy
                time.sleep(0.5)
        finally:
            lux.off()
            print("light off")


if __name__ == "__main__":
    main()
