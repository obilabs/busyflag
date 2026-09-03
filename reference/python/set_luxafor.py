import usb.core
import usb.util

dev = usb.core.find(idVendor=0x04D8, idProduct=0xF372)

if dev is None:
    raise ValueError("Device not found")
else:
    print("Device found")
