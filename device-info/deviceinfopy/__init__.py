# Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
# SPDX-License-Identifier: MIT OR Apache-2.0

import struct

class DeviceInfoBuilder:
    def __init__(self):
        self._payload = bytes()

    def add_device_id(self, value: str) -> None:
        self._add_field(0, value.encode())

    def finish(self) -> bytes:
        data = struct.pack("<H", len(self._payload)) + self._payload
        sum = _checksum(data)
        return struct.pack("<H", sum) + data

    def _add_field(self, id: int, payload: bytes) -> None:
        head = struct.pack("<HH", id, len(payload))
        self._payload += head + payload

def _checksum(data: bytes) -> int:
    def reduce(v: int) -> int:
        l = v & 0xFF
        r = v >> 8
        res = l + r
        if res == 0xFF:
            return 0
        else:
            return res

    a, b = 0, 0
    for chunk in [data[i:i+21] for i in range(0, len(data), 21)]:
        for byte in chunk:
            a += byte
            b += a
        a = reduce(a)
        b = reduce(b)

    a = reduce(a)
    b = reduce(b)
    return (b << 8) | a

import unittest
class TestBuilder(unittest.TestCase):
    def test_canned_info(self):
        import binascii
        builder = DeviceInfoBuilder()
        builder.add_device_id("gprb-5xzpf-00002")
        out_hex = binascii.b2a_hex(builder.finish())
        self.assertEqual(out_hex, b'1dc7140000001000677072622d35787a70662d3030303032')

if __name__ == "__main__":
    unittest.main()