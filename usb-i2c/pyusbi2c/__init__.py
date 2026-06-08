# Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
# SPDX-License-Identifier: MIT OR Apache-2.0

import typing

import usb

from . import protocol as proto

T = typing.TypeVar("T")

class ErrorResponse(Exception):
    @classmethod
    def config_error(cls, status: int):
        cls(status, "Invalid configuration argument")

    @classmethod
    def operation_error(cls, status: int):
        if status == 1:
            cls(status, "I2C device didn't respond")
        else:
            cls(status, "Invalid operation argument")

    def __init__(self, status: int, message):
        super().__init__(message)
        self.status = status

class GeneralError(Exception):
    def __init__(self):
        super().__init__("Got a general error usb response")

class NoResponse(Exception):
    def __init__(self):
        super().__init__("Didn't get a usb response")

class Device:
    @classmethod
    def open_first(cls: T) -> T:
        def is_device(dev):
            for cfg in dev:
                if get_interface(dev, cfg) is not None:
                    return True
            return False

        dev = usb.core.find(custom_match=is_device)
        if dev is None:
            raise ValueError('Device not found')
        
        return cls.open_dev(dev)
                    
    @classmethod
    def open_dev(cls: T, dev: usb.core.Device) -> T:
        try:
            cfg = dev.get_active_configuration()
        except usb.core.USBError:
            # set the active configuration. With no arguments, the first
            # configuration will be the active one
            dev.set_configuration()
            cfg = dev.get_active_configuration()

        # get an endpoint instance
        intf = get_interface(dev, cfg)

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
        
        # read any packets that might still be in the buffer
        try:
            while True:
                in_ep.read(in_ep.wMaxPacketSize, 1)
        except:
            pass
        
        return cls(out_ep, in_ep)
    
    def __init__(self, out_ep: usb.core.Endpoint, in_ep: usb.core.Endpoint):
        self.out_ep = out_ep
        self.in_ep = in_ep

    @property
    def max_data_len(self):
        return min(self.max_read_data_len, self.max_write_data_len, self.max_write_read_read_data_len, self.max_write_read_write_data_len)
    
    @property
    def max_write_data_len(self):
        return self.out_ep.wMaxPacketSize - proto.WriteRequest.HEADER_LEN
    
    @property
    def max_write_read_write_data_len(self):
        return self.out_ep.wMaxPacketSize - proto.WriteReadRequest.HEADER_LEN
    
    @property
    def max_write_read_read_data_len(self):
        return self.in_ep.wMaxPacketSize - proto.WriteReadResponse.HEADER_LEN
    
    @property
    def max_read_data_len(self):
        return self.in_ep.wMaxPacketSize - proto.ReadResponse.HEADER_LEN

    def configure(self, freq_hz: int) -> None:
        req = proto.ConfigureRequest(version = 0, freq_hz = freq_hz)
        res = self._command(req, proto.ConfigureResponse)
        if res.status != 0:
            raise ErrorResponse.config_error(res.status)
    
    def write(self, address: int, data: bytes) -> None:
        if len(data) > self.max_write_data_len:
            raise ValueError("write data too big")

        req = proto.WriteRequest(address = address)
        req.data = data
        res = self._command(req, proto.WriteResponse)
        if res.status != 0:
            raise ErrorResponse.operation_error(res.status)

    def read(self, address: int, len: int) -> bytes:
        if len > self.max_read_data_len:
            raise ValueError("read len too big")

        req = proto.ReadRequest(address = address, len = len)
        res = self._command(req, proto.ReadResponse)
        if res.status != 0:
            raise ErrorResponse.operation_error(res.status)
        return res.data

    def write_read(self, address: int, read_len: int, data: bytes) -> bytes:
        if read_len > self.max_write_read_read_data_len:
            raise ValueError("read len too big")
        if len(data) > self.max_write_read_write_data_len:
            raise ValueError("write buffer too big")

        req = proto.WriteReadRequest(address = address, read_len = read_len)
        req.data = data
        res = self._command(req, proto.WriteReadResponse)
        if res.status != 0:
            raise ErrorResponse.operation_error(res.status)
        return res.data
        
    def _command(self, req, Res: T) -> T:
        self.out_ep.write(bytes(req))

        # try to parse the response
        try:
            buf = self.in_ep.read(64)
        except usb.core.USBTimeoutError:
            raise NoResponse()
        res = proto.unpack(buf)
        if isinstance(res, Res):
            return res
        elif isinstance(res, proto.GeneralError):
            raise GeneralError()
        raise NoResponse()

class SMBusDevice:
    @classmethod
    def open_first(cls):
        return cls(Device.open_first())

    def __init__(self, dev: Device):
        self.dev = dev

    def write_i2c_block_data(self, i2c_address: int, register: int, values: list[int]) -> None:
        data = bytes([register]) + bytes(values)
        self.dev.write(i2c_address, data)
    
    def read_i2c_block_data(self, i2c_address: int, register: int, length: int) -> list[int]:
        data = bytes([register])
        return self.dev.write_read(i2c_address, length, data)
    
def get_interface(dev: usb.Device, cfg: usb.Configuration) -> usb.Interface | None:
    for intf in cfg:
        try:
            if usb.util.get_string(dev, intf.iInterface) == "Grapple I2C":
                return intf
        except:
            pass
    return None
