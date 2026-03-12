// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use packet_gen::packet;

pub enum Packet<B> {
    ConfigureRequest(ConfigureRequest<B>),
    ConfigureResponse(ConfigureResponse<B>),
    WriteRequest(WriteRequest<B>),
    WriteResponse(WriteResponse<B>),
    ReadRequest(ReadRequest<B>),
    ReadResponse(ReadResponse<B>),
    WriteReadRequest(WriteReadRequest<B>),
    WriteReadResponse(WriteReadResponse<B>),
}

impl<B: AsRef<[u8]>> Packet<B> {
    pub fn try_parse_request(inner: B) -> Result<Self, Error> {
        let base = BasePacket::try_parse(inner)?;
        match base.get_type().try_into() {
            Ok(CommandId::Configure) => Ok(Self::ConfigureRequest(ConfigureRequest::try_from(base)?)),
            Ok(CommandId::Write) => Ok(Self::WriteRequest(WriteRequest::try_from(base)?)),
            Ok(CommandId::Read) => Ok(Self::ReadRequest(ReadRequest::try_from(base)?)),
            Ok(CommandId::WriteRead) => Ok(Self::WriteReadRequest(WriteReadRequest::try_from(base)?)),
            _ => Err(Error::UnknownId)
        }
    }

    pub fn try_parse_response(inner: B) -> Result<Self, Error> {
        let base = BasePacket::try_parse(inner)?;
        match base.get_type().try_into() {
            Ok(CommandId::Configure) => Ok(Self::ConfigureResponse(ConfigureResponse::try_from(base)?)),
            Ok(CommandId::Write) => Ok(Self::WriteResponse(WriteResponse::try_from(base)?)),
            Ok(CommandId::Read) => Ok(Self::ReadResponse(ReadResponse::try_from(base)?)),
            Ok(CommandId::WriteRead) => Ok(Self::WriteReadResponse(WriteReadResponse::try_from(base)?)),
            _ => Err(Error::UnknownId)
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, defmt::Format)]
pub enum Error {
    NotEnoughBytes,
    UnknownId,
}

packet!{pub ConfigureRequest(CommandId::Configure as u8, ConfigureResponse) {
    version: uint8,
    freq_hz: uint32,
}}
packet!{pub ConfigureResponse(CommandId::Configure as u8) {
    result: uint8,
}}

packet!{pub WriteRequest(CommandId::Write as u8, WriteResponse) {
    address: uint16,
    data: [uint8],
}}
packet!{pub WriteResponse(CommandId::Write as u8) {
    result: uint8,
}}

packet!{pub ReadRequest(CommandId::Read as u8, ReadResponse) {
    address: uint16,
    len: uint8,
}}
packet!{pub ReadResponse(CommandId::Read as u8) {
    result: uint8,
    data: [uint8],
}}

packet!{pub WriteReadRequest(CommandId::WriteRead as u8, WriteReadResponse) {
    address: uint16,
    read_len: uint8,
    write_data: [uint8],
}}
packet!{pub WriteReadResponse(CommandId::WriteRead as u8) {
    result: uint8,
    read_data: [uint8],
}}

packet!{pub GeneralError(CommandId::GeneralError as u8) {}}

#[derive(Debug, PartialEq, Clone, Copy, num_enum::TryFromPrimitive)]
#[repr(u8)]
enum CommandId {
    Configure = 0,
    Write = 1,
    Read = 2,
    WriteRead = 3,
    GeneralError = 255,
}

struct BasePacket<B> {
    inner: B
}

impl<B: AsRef<[u8]>> BasePacket<B> {
    pub fn try_parse(inner: B) -> Result<Self, Error> {
        if inner.as_ref().len() >= 2 {
            Ok(Self { inner })
        } else {
            Err(Error::NotEnoughBytes)
        }
    }

    pub fn get_type(&self) -> u8 {
        self.inner.as_ref()[0]
    }

    pub fn get_id(&self) -> u8 {
        self.inner.as_ref()[1]
    }

    pub fn get_payload(&self) -> &[u8] {
        &self.inner.as_ref()[2..]
    }
}

impl<B: AsMut<[u8]>> BasePacket<B> {
    pub fn try_alloc(mut inner: B) -> Result<Self, Error> {
        if inner.as_mut().len() >= 2 {
            Ok(Self { inner })
        } else {
            Err(Error::NotEnoughBytes)
        }
    }

    pub fn set_type(&mut self, packet_type: u8) {
        self.inner.as_mut()[0] = packet_type
    }

    pub fn set_id(&mut self, id: u8) {
        self.inner.as_mut()[1] = id;
    }

    pub fn get_payload_mut(&mut self) -> &mut [u8] {
        &mut self.inner.as_mut()[2..]
    }

    pub fn commit(self, len: usize) -> usize {
        2 + len
    }
}