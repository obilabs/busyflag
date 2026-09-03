#!/usr/bin/env python3
"""Minimal Luxafor Flag driver (HID) + CLI.

Protocol (9-byte HID output report, byte 0 = report id 0x00):
  01 LED R G B            static colour
  02 LED R G B 00 SPEED   fade to colour
  03 LED R G B SPEED 00 REPEAT   strobe/blink
  04 WAVE R G B 00 REPEAT SPEED  wave
  06 PATTERN REPEAT       built-in pattern
LED: 0xFF all, 0x41 front (tab side, LEDs 1-3), 0x42 back (LEDs 4-6), 1..6 single LED.

Usage:
  python3 luxafor.py red | green | blue | off | "#ff8800" | 255,128,0
  python3 luxafor.py blink red [speed=10] [repeat=5]
  python3 luxafor.py fade blue [speed=30]
  python3 luxafor.py demo
"""
import sys
import time

import hid

VID, PID = 0x04D8, 0xF372

LED_ALL, LED_FRONT, LED_BACK = 0xFF, 0x41, 0x42

COLOURS = {
    "red": (255, 0, 0), "green": (0, 255, 0), "blue": (0, 0, 255),
    "yellow": (255, 255, 0), "orange": (255, 100, 0), "purple": (128, 0, 255),
    "magenta": (255, 0, 255), "cyan": (0, 255, 255), "white": (255, 255, 255),
    "off": (0, 0, 0),
}


def parse_colour(s):
    s = s.strip().lower()
    if s in COLOURS:
        return COLOURS[s]
    if s.startswith("#") and len(s) == 7:
        return tuple(int(s[i:i + 2], 16) for i in (1, 3, 5))
    parts = s.split(",")
    if len(parts) == 3:
        return tuple(max(0, min(255, int(p))) for p in parts)
    raise ValueError(f"unknown colour: {s}")


class Luxafor:
    def __init__(self):
        self.dev = hid.device()
        self.dev.open(VID, PID)
        self.dev.set_nonblocking(0)

    def close(self):
        self.dev.close()

    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.close()

    def _write(self, payload):
        buf = [0x00] + list(payload)
        buf += [0] * (9 - len(buf))
        n = self.dev.write(buf)
        if n < 0:
            raise IOError("HID write failed")
        return n

    def colour(self, rgb, led=LED_ALL):
        r, g, b = rgb
        self._write([0x01, led, r, g, b])

    def off(self):
        self.colour((0, 0, 0))

    def fade(self, rgb, speed=20, led=LED_ALL):
        r, g, b = rgb
        self._write([0x02, led, r, g, b, 0x00, speed])

    def blink(self, rgb, speed=10, repeat=5, led=LED_ALL):
        """Hardware strobe. speed: lower = faster (1..255). repeat: 0 = forever."""
        r, g, b = rgb
        self._write([0x03, led, r, g, b, speed, 0x00, repeat])

    def wave(self, rgb, wave_type=2, speed=30, repeat=3):
        r, g, b = rgb
        self._write([0x04, wave_type, r, g, b, 0x00, repeat, speed])

    def pattern(self, pattern=1, repeat=1):
        """Built-in patterns 1..8 (1 Luxafor, 2 random1, 3 random2, 4 random3, 5 police, 6 random4, 7 random5, 8 rainbow)."""
        self._write([0x06, pattern, repeat])


def main(argv):
    if not argv:
        print(__doc__)
        return 1
    cmd = argv[0].lower()
    with Luxafor() as lux:
        if cmd == "demo":
            print("static: red, green, blue")
            for c in ("red", "green", "blue"):
                lux.colour(COLOURS[c]); time.sleep(0.6)
            print("front red / back blue")
            lux.colour(COLOURS["red"], LED_FRONT); lux.colour(COLOURS["blue"], LED_BACK); time.sleep(1.0)
            print("software blink yellow x3")
            for _ in range(3):
                lux.colour(COLOURS["yellow"]); time.sleep(0.25); lux.off(); time.sleep(0.25)
            print("hardware strobe magenta x5")
            lux.blink(COLOURS["magenta"], speed=10, repeat=5); time.sleep(2.0)
            print("fade to cyan")
            lux.fade(COLOURS["cyan"], speed=40); time.sleep(1.5)
            print("off")
            lux.off()
            return 0
        if cmd == "blink":
            rgb = parse_colour(argv[1]) if len(argv) > 1 else COLOURS["red"]
            speed = int(argv[2]) if len(argv) > 2 else 10
            repeat = int(argv[3]) if len(argv) > 3 else 5
            lux.blink(rgb, speed, repeat)
            return 0
        if cmd == "fade":
            rgb = parse_colour(argv[1]) if len(argv) > 1 else COLOURS["blue"]
            speed = int(argv[2]) if len(argv) > 2 else 30
            lux.fade(rgb, speed)
            return 0
        if cmd == "pattern":
            lux.pattern(int(argv[1]) if len(argv) > 1 else 1, int(argv[2]) if len(argv) > 2 else 1)
            return 0
        # otherwise treat as a colour
        lux.colour(parse_colour(cmd))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
