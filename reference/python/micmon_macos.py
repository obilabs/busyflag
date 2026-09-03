#!/usr/bin/env python3
"""macOS microphone-in-use detector using CoreAudio via ctypes (no extra deps).

Checks every audio device that has input channels and asks CoreAudio for
kAudioDevicePropertyDeviceIsRunningSomewhere, which is true whenever ANY
process on the system has an active capture on that device.

Run directly to poll and print state changes.
"""
import ctypes
import ctypes.util
import struct
import sys
import time

ca = ctypes.cdll.LoadLibrary(ctypes.util.find_library("CoreAudio"))
cf = ctypes.cdll.LoadLibrary(ctypes.util.find_library("CoreFoundation"))


def fourcc(s):
    return struct.unpack(">I", s.encode("ascii"))[0]


kAudioObjectSystemObject = 1
kAudioHardwarePropertyDevices = fourcc("dev#")
kAudioObjectPropertyScopeGlobal = fourcc("glob")
kAudioDevicePropertyScopeInput = fourcc("inpt")
kAudioObjectPropertyElementMain = 0
kAudioDevicePropertyStreamConfiguration = fourcc("slay")
kAudioDevicePropertyDeviceIsRunningSomewhere = fourcc("gone")
kAudioObjectPropertyName = fourcc("lnam")


class PropAddr(ctypes.Structure):
    _fields_ = [("mSelector", ctypes.c_uint32), ("mScope", ctypes.c_uint32), ("mElement", ctypes.c_uint32)]


ca.AudioObjectGetPropertyDataSize.argtypes = [ctypes.c_uint32, ctypes.POINTER(PropAddr), ctypes.c_uint32, ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
ca.AudioObjectGetPropertyData.argtypes = [ctypes.c_uint32, ctypes.POINTER(PropAddr), ctypes.c_uint32, ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32), ctypes.c_void_p]
cf.CFStringGetCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_long, ctypes.c_uint32]
cf.CFStringGetCString.restype = ctypes.c_bool
cf.CFRelease.argtypes = [ctypes.c_void_p]


def _get(obj, selector, scope, ctype):
    addr = PropAddr(selector, scope, kAudioObjectPropertyElementMain)
    size = ctypes.c_uint32()
    if ca.AudioObjectGetPropertyDataSize(obj, ctypes.byref(addr), 0, None, ctypes.byref(size)) != 0:
        return None
    buf = ctypes.create_string_buffer(size.value)
    if ca.AudioObjectGetPropertyData(obj, ctypes.byref(addr), 0, None, ctypes.byref(size), buf) != 0:
        return None
    return buf.raw[: size.value]


def all_devices():
    raw = _get(kAudioObjectSystemObject, kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal, None) or b""
    return list(struct.unpack(f"<{len(raw)//4}I", raw))


def input_channels(dev):
    raw = _get(dev, kAudioDevicePropertyStreamConfiguration, kAudioDevicePropertyScopeInput, None)
    if not raw:
        return 0
    nbuf = struct.unpack("<I", raw[:4])[0]
    chans = 0
    off = 8  # mBuffers[] starts at offset 8 (AudioBuffer is pointer-aligned)
    for _ in range(nbuf):  # AudioBuffer: UInt32 mNumberChannels, UInt32 mDataByteSize, void* mData -> 16 bytes
        chans += struct.unpack("<I", raw[off:off + 4])[0]
        off += 16
    return chans


def device_name(dev):
    raw = _get(dev, kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal, None)
    if not raw or len(raw) < 8:
        return f"device {dev}"
    cfstr = struct.unpack("<Q", raw[:8])[0]
    out = ctypes.create_string_buffer(256)
    ok = cf.CFStringGetCString(cfstr, out, 256, 0x08000100)  # kCFStringEncodingUTF8
    cf.CFRelease(cfstr)
    return out.value.decode("utf-8", "replace") if ok else f"device {dev}"


def is_running_somewhere(dev):
    raw = _get(dev, kAudioDevicePropertyDeviceIsRunningSomewhere, kAudioObjectPropertyScopeGlobal, None)
    return bool(raw and struct.unpack("<I", raw[:4])[0])


def active_input_devices():
    """Return names of input-capable devices currently being captured by any process."""
    return [device_name(d) for d in all_devices() if input_channels(d) > 0 and is_running_somewhere(d)]


def mic_in_use():
    return bool(active_input_devices())


if __name__ == "__main__":
    print("input devices:", [device_name(d) for d in all_devices() if input_channels(d) > 0])
    last = None
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 1e9
    t0 = time.time()
    while time.time() - t0 < duration:
        cur = active_input_devices()
        if cur != last:
            print(time.strftime("%H:%M:%S"), "MIC IN USE by:" if cur else "mic idle", cur)
            last = cur
        time.sleep(0.5)
