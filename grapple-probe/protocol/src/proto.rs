// Copyright (c) 2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;

use packet_gen::packet;

pub enum Packet<B> {
    GetStatusRequest(GetStatusRequest<B>),
    GetStatusResponse(GetStatusResponse<B>),
    ReadFieldRequest(ReadFieldRequest<B>),
    ReadFieldResponse(ReadFieldResponse<B>),
    WriteFieldRequest(WriteFieldRequest<B>),
    WriteFieldResponse(WriteFieldResponse<B>),
    ResetRequest(ResetRequest<B>),
}

impl<B: AsRef<[u8]>> Packet<B> {
    pub fn try_parse_request(inner: B) -> Result<Self, Error> {
        let base = BasePacket::try_parse(inner)?;
        match base.get_type().try_into() {
            Ok(PacketId::Status) => Ok(Self::GetStatusRequest(GetStatusRequest::try_from(base)?)),
            Ok(PacketId::ReadField) => Ok(Self::ReadFieldRequest(ReadFieldRequest::try_from(base)?)),
            Ok(PacketId::WriteField) => Ok(Self::WriteFieldRequest(WriteFieldRequest::try_from(base)?)),
            Ok(PacketId::Reset) => Ok(Self::ResetRequest(ResetRequest::try_from(base)?)),
            _ => Err(Error::InvalidPacket)
        }
    }

    pub fn try_parse_response(inner: B) -> Result<Self, Error> {
        let base = BasePacket::try_parse(inner)?;
        match base.get_type().try_into() {
            Ok(PacketId::Status) => Ok(Self::GetStatusResponse(GetStatusResponse::try_from(base)?)),
            Ok(PacketId::ReadField) => Ok(Self::ReadFieldResponse(ReadFieldResponse::try_from(base)?)),
            Ok(PacketId::WriteField) => Ok(Self::WriteFieldResponse(WriteFieldResponse::try_from(base)?)),
            _ => Err(Error::InvalidPacket)
        }
    }
}

packet!{pub GetStatusRequest(PacketId::Status as u8, GetStatusResponse) {}}
packet!{pub GetStatusResponse(PacketId::Status as u8) {
    flags: uint8,
    target_voltage: uint16,
    signal_voltage: uint16,
}}

packet!{pub ReadFieldRequest(PacketId::ReadField as u8, ReadFieldResponse) {
    field_id: uint8,
}}
packet!{pub ReadFieldResponse(PacketId::ReadField as u8) {
    status: uint8,
    field_id: uint8,
    data: [uint8],
}}

packet!{pub WriteFieldRequest(PacketId::WriteField as u8, WriteFieldResponse) {
    field_id: uint8,
    data: [uint8],
}}
packet!{pub WriteFieldResponse(PacketId::WriteField as u8) {
    status: uint8,
    field_id: uint8,
}}

packet!{pub ResetRequest(PacketId::Reset as u8) {
    reset_type: uint8,
}}

pub struct StatusFlags {
    pub inner: u8,
}

impl StatusFlags {
    pub fn default() -> Self {
        Self { inner: 0 }
    }

    pub fn gnd_detect(&self) -> bool {
        self.inner & 0x80 != 0
    }

    pub fn set_gnd_detect(&mut self, value: bool) {
        let value = if value { 0x80 } else { 0 };
        self.inner = (self.inner & !0x80) | value;
    }
}

#[derive(num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum ResetType {
    Panic = 255
}

struct BasePacket<B> {
    inner: B,
}

impl<B: AsRef<[u8]>> BasePacket<B> {
    pub fn try_parse(inner: B) -> Result<Self, Error> {
        if inner.as_ref().len() >= 2 {
            if inner.as_ref()[0] == 0xE0 {
                Ok(Self { inner })
            } else {
                Err(Error::InvalidPacket)
            }
        } else {
            Err(Error::NotEnoughBytes)
        }
    }

    fn get_type(&self) -> u8 {
        self.inner.as_ref()[1]
    }

    fn get_id(&self) -> u8 {
        0
    }

    fn get_payload(&self) -> &[u8] {
        &self.inner.as_ref()[2..]
    }
}

impl<B: AsMut<[u8]>> BasePacket<B> {
    pub fn try_alloc(mut inner: B) -> Result<Self, Error> {
        if inner.as_mut().len() >= 2 {
            inner.as_mut()[0] = 0xE0;
            Ok(Self{ inner })
        } else {
            Err(Error::NotEnoughBytes)
        }
    }

    fn set_type(&mut self, packet_type: u8) {
        self.inner.as_mut()[1] = packet_type
    }

    fn set_id(&mut self, _id: u8) {}

    fn get_payload_mut(&mut self) -> &mut [u8] {
        &mut self.inner.as_mut()[2..]
    }

    fn commit(self, payload_len: usize) -> usize {
        2 + payload_len
    }
}

#[derive(num_enum::TryFromPrimitive)]
#[repr(u8)]
enum PacketId {
    Status = 0,
    ReadField = 1,
    WriteField = 2,
    Reset = 254,
}