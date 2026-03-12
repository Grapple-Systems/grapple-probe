# Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import usb

def open_cmsis_dev() -> tuple[usb.core.Endpoint, usb.core.Endpoint]:
    def is_device(dev):
            for cfg in dev:
                for intf in cfg:
                    ifstr = usb.util.get_string(dev, intf.iInterface)
                    if ifstr and "CMSIS-DAP" in ifstr:
                        return True
            return False
    
    dev = usb.core.find(custom_match=is_device)
    if dev is None:
        raise ValueError('Device not found')

    try:
        cfg = dev.get_active_configuration()
    except usb.core.USBError:
        # set the active configuration. With no arguments, the first
        # configuration will be the active one
        dev.set_configuration()
        cfg = dev.get_active_configuration()

    # get an endpoint instance
    intf = cfg[(0,0)]

    out_ep = usb.util.find_descriptor(
        intf,
        # match the first OUT endpoint
        custom_match = \
        lambda e: \
            usb.util.endpoint_direction(e.bEndpointAddress) == \
            usb.util.ENDPOINT_OUT)
    in_ep = usb.util.find_descriptor(
        intf,
        # match the first IN endpoint
        custom_match = \
        lambda e: \
            usb.util.endpoint_direction(e.bEndpointAddress) == \
            usb.util.ENDPOINT_IN)
    
    return (out_ep, in_ep)

def do_info(count):
    import struct
    import time

    out_ep, in_ep = open_cmsis_dev()
    packet = struct.pack(">BB", 0, 4)

    def take_sample():
        start = time.time_ns()
        out_ep.write(packet)
        res = in_ep.read(64)
        duration = time.time_ns() - start
        if not res:
            raise RuntimeError("no response from target")
        return duration

    samples = [take_sample() for _ in range(count)]

    mean = sum(samples) / count
    smallest = min(samples)
    largest = max(samples)

    print("info %d times, mean: %d, min: %d, max: %d" % (count, mean, smallest, largest))

if __name__ == "__main__":
    do_info(10000)