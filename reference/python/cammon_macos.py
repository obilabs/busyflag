#!/usr/bin/env python3
"""macOS camera-in-use detector using CoreMediaIO via ctypes (no extra deps).

Asks every CMIO device for kCMIODevicePropertyDeviceIsRunningSomewhere.
"""
import ctypes
import ctypes.util
import struct
import sys
import time

cmio = ctypes.cdll.LoadLibrary(ctypes.util.find_library("CoreMediaIO"))
cf = ctypes.cdll.LoadLibrary(ctypes.util.find_library("CoreFoundation"))


def fourcc(s):
    return struct.unpack(">I", s.encode("ascii"))[0]


kCMIOObjectSystemObject = 1
kCMIOHardwarePropertyDevices = fourcc("dev#")
kCMIOObjectPropertyScopeGlobal = fourcc("glob")
kCMIOObjectPropertyElementMain = 0
kCMIODevicePropertyDeviceIsRunningSomewhere = fourcc("gone")
kCMIOObjectPropertyName = fourcc("lnam")


class PropAddr(ctypes.Structure):
    _fields_ = [("mSelector", ctypes.c_uint32), ("mScope", ctypes.c_uint32), ("mElement", ctypes.c_uint32)]


cmio.CMIOObjectGetPropertyDataSize.argtypes = [ctypes.c_uint32, ctypes.POINTER(PropAddr), ctypes.c_uint32, ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
cmio.CMIOObjectGetPropertyData.argtypes = [ctypes.c_uint32, ctypes.POINTER(PropAddr), ctypes.c_uint32, ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(ctypes.c_uint32), ctypes.c_void_p]
cf.CFStringGetCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_long, ctypes.c_uint32]
cf.CFStringGetCString.restype = ctypes.c_bool
cf.CFRelease.argtypes = [ctypes.c_void_p]


def _get(obj, selector, scope=kCMIOObjectPropertyScopeGlobal):
    addr = PropAddr(selector, scope, kCMIOObjectPropertyElementMain)
    size = ctypes.c_uint32()
    if cmio.CMIOObjectGetPropertyDataSize(obj, ctypes.byref(addr), 0, None, ctypes.byref(size)) != 0:
        return None
    buf = ctypes.create_string_buffer(size.value)
    used = ctypes.c_uint32()
    if cmio.CMIOObjectGetPropertyData(obj, ctypes.byref(addr), 0, None, size, ctypes.byref(used), buf) != 0:
        return None
    return buf.raw[: used.value]


def all_devices():
    raw = _get(kCMIOObjectSystemObject, kCMIOHardwarePropertyDevices) or b""
    return list(struct.unpack(f"<{len(raw)//4}I", raw))


def device_name(dev):
    raw = _get(dev, kCMIOObjectPropertyName)
    if not raw or len(raw) < 8:
        return f"device {dev}"
    cfstr = struct.unpack("<Q", raw[:8])[0]
    out = ctypes.create_string_buffer(256)
    ok = cf.CFStringGetCString(cfstr, out, 256, 0x08000100)
    cf.CFRelease(cfstr)
    return out.value.decode("utf-8", "replace") if ok else f"device {dev}"


def is_running_somewhere(dev):
    raw = _get(dev, kCMIODevicePropertyDeviceIsRunningSomewhere)
    return bool(raw and struct.unpack("<I", raw[:4])[0])


def active_cameras():
    return [device_name(d) for d in all_devices() if is_running_somewhere(d)]


def camera_in_use():
    return bool(active_cameras())


if __name__ == "__main__":
    print("cameras:", [device_name(d) for d in all_devices()])
    last = None
    duration = float(sys.argv[1]) if len(sys.argv) > 1 else 1e9
    t0 = time.time()
    while time.time() - t0 < duration:
        cur = active_cameras()
        if cur != last:
            print(time.strftime("%H:%M:%S"), "CAMERA IN USE by:" if cur else "camera idle", cur)
            last = cur
        time.sleep(0.5)
