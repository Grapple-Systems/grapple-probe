# Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
# SPDX-License-Identifier: MIT OR Apache-2.0

import dpkt
import struct

class _Header(dpkt.Packet):
    __byte_order__ = "<"
    __hdr__ = (
        ("type", "B", None),
        ("id", "B", 0),
    )

class _Packet(dpkt.Packet):
    HEADER_LEN = 2
    __byte_order__ = "<"

    def __init__(self, *args, **kwargs):
        if args and isinstance(args[0], _Header):
            setattr(self, "h", args[0])
            return super().__init__(args[0].data)
        else:
            setattr(self, "h", _Header(**kwargs))
            self.h.type = self._type
            return super().__init__(*args, **kwargs)
        
    def unpack(self, buf):
        super().unpack(buf)
        self._unpack_repeat()
        
    def __bytes__(self):
        return bytes(self.h) + super().__bytes__() + self._pack_repeat()
    
    @property
    def id(self):
        return self.h.id
    
    @id.setter
    def id(self, value):
        self.h.id = value

    def _unpack_repeat(self):
        try:
            if type(self.__repeat__[1]) == str:
                # slice
                count = len(self.data) // struct.calcsize("=%s" % self.__repeat__[1])
                fmt = "%s%d%s" % (self.__byte_order__, count, self.__repeat__[1])
                repeat = struct.unpack(fmt, self.data)
                if self.__repeat__[1] == "s":
                    setattr(self, self.__repeat__[0], repeat[0])
                else:
                    setattr(self, self.__repeat__[0], repeat)
            else:
                # repeat with fields
                size = len(self.__repeat__[1]())
                count = len(self.data) // size
                repeat = [self.__repeat__[1](self.data[i:i+size]) for i in range(0, count * size, size)]
                setattr(self, self.__repeat__[0], repeat)
        except:
            pass

    def _pack_repeat(self):
        try:
            repeat = getattr(self, self.__repeat__[0])
            if type(self.__repeat__[1]) == str:
                # slice
                fmt = "%s%d%s" % (self.__byte_order__, len(repeat), self.__repeat__[1])
                if self.__repeat__[1] == "s":
                    return struct.pack(fmt, repeat)
                else:
                    return struct.pack(fmt, *repeat)
            else:
                # repeat with fields
                return b''.join([bytes(r) for r in repeat])
        except:
            return b''
        
class ConfigureRequest(_Packet):
    _type = 0x00
    __hdr__ = [
        ("version", "B", None),
        ("freq_hz", "I", None),
    ]

class ConfigureResponse(_Packet):
    _type = 0x00
    __hdr__ = [
        ("status", "B", None),
    ]
        
class WriteRequest(_Packet):
    HEADER_LEN = _Packet.HEADER_LEN + 2
    _type = 0x01
    __hdr__ = [
        ("address", "H", None),
        # data field already setup
    ]

class WriteResponse(_Packet):
    _type = 0x01
    __hdr__ = [
        ("status", "B", None),
    ]

class ReadRequest(_Packet):
    _type = 0x02
    __hdr__ = [
        ("address", "H", None),
        ("len", "B", None),
    ]

class ReadResponse(_Packet):
    HEADER_LEN = _Packet.HEADER_LEN + 1
    _type = 0x02
    __hdr__ = [
        ("status", "B", None),
        # data field already setup
    ]

class WriteReadRequest(_Packet):
    HEADER_LEN = _Packet.HEADER_LEN + 3
    _type = 0x03
    __hdr__ = [
        ("address", "H", None),
        ("read_len", "B", None),
        # data field already setup
    ]

class WriteReadResponse(_Packet):
    HEADER_LEN = _Packet.HEADER_LEN + 1
    _type = 0x03
    __hdr__ = [
        ("status", "B", None),
        # data field already setup
    ]

class GeneralError(_Packet):
    _type = 0xFF

_responses = {
    0x00: ConfigureResponse,
    0x01: WriteResponse,
    0x02: ReadResponse,
    0x03: WriteReadResponse,
    0xFF: GeneralError,
}

def unpack(buf: bytes):
    h = _Header(buf)
    res = _responses.get(h.type)
    if res is not None:
        return res(h)
    return None