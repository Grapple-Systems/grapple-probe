# Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
# SPDX-License-Identifier: MIT OR Apache-2.0

from . import Device, SMBusDevice

import binascii
import struct
import sys

def open_device(args):
    return Device.open_first()

def configure(args):
    print("setting frequency to %d hz" % args.frequency)
    device = open_device(args)
    device.configure(args.frequency)

def read(args):
    print("reading %d bytes from %d" % (args.len, args.address))
    device = open_device(args)
    read_data = device.read(args.address, args.len)
    print(binascii.b2a_hex(read_data))

def write(args):
    print("writing 0x%s to %d" % (binascii.b2a_hex(args.bytes_hex), args.address))
    device = open_device(args)
    device.write(args.address, args.bytes_hex)

def write_read(args):
    device = open_device(args)
    read_data = device.write_read(args.address, args.read_len, args.write_bytes)
    print(binascii.b2a_hex(read_data))

def bh1745_get_rgb(args):
    import bh1745

    device = SMBusDevice(open_device(args))
    bh = bh1745.BH1745(i2c_dev=device)
    bh.setup()
    if bh.ready():
        print(bh.get_rgb_scaled())
    else:
        print("Couldn't setup the bh1745")

def write_device_info(args):
    import pathlib
    device_info_path = (pathlib.Path(__file__).parent.parent.parent / "device-info").resolve()
    sys.path.append(str(device_info_path))
    from deviceinfopy import DeviceInfoBuilder

    dev = open_device(args)

    info = DeviceInfoBuilder()
    info.add_device_id(args.device_id)
    data = info.finish()

    address = 0b1010000

    dev.write(address, struct.pack(">H", 0) + data)
    
def parse_args():
    import argparse
    parser = argparse.ArgumentParser(prog="USB I2C", description="Interact with a USB to I2C device")
    
    subcommands = parser.add_subparsers(help="subcommand help")

    configure_cmd = subcommands.add_parser('c', help="configure the i2c bus")
    configure_cmd.add_argument('frequency', type=int, help="Frequency of the i2c bus in hz")
    configure_cmd.set_defaults(func = configure)

    read_cmd = subcommands.add_parser('r', help="Read data from a device on the i2c bus")
    read_cmd.add_argument('address', type=int, help="Address of device without the read/write bit")
    read_cmd.add_argument('len', type=int, help="Number of bytes to read")
    read_cmd.set_defaults(func = read)

    write_cmd = subcommands.add_parser('w', help="Write data to a device on the i2c bus")
    write_cmd.add_argument('address', type=int, help="Address of device without the read/write bit")
    write_cmd.add_argument('bytes_hex', type=binascii.a2b_hex, help="Bytes to write in hexadecimal without '0x'")
    write_cmd.set_defaults(func = write)

    write_read_cmd = subcommands.add_parser('wr', help="Write data then read data from a device on the i2c bus")
    write_read_cmd.add_argument('address', type=int, help="Address of device without the read/write bit")
    write_read_cmd.add_argument('read_len', type=int, help="Number of bytes to read")
    write_read_cmd.add_argument('write_bytes', type=binascii.a2b_hex, help="Bytes to write in hexadecimal without '0x'")
    write_read_cmd.set_defaults(func = write_read)

    bh_cmd = subcommands.add_parser('bh', help="Get the scaled rgb values on a bh1745")
    bh_cmd.set_defaults(func = bh1745_get_rgb)

    write_device_info_cmd = subcommands.add_parser('wdi', help="Build and write a device info package to eeprom")
    write_device_info_cmd.add_argument('device_id', type=str, help="Unique identifier string for the device")
    write_device_info_cmd.set_defaults(func = write_device_info)

    return parser.parse_args()

def main():
    args = parse_args()
    args.func(args)

if __name__ == "__main__":
    main()