// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    BufferNotBigEnough,
    UnsupportedCommand(u8),
}

impl Error {
    pub fn respond<Buf: AsMut<[u8]>>(buf: Buf) -> Result<usize, Self> {
        let packet = Packet::try_alloc(buf, CommandType::Unsupported)?;
        Ok(packet.commit(0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum CommandType {
    Info = 0x00,
    HostStatus = 0x01,
    Connect = 0x02,
    Disconnect = 0x03,
    TransferConfigure = 0x04,
    Transfer = 0x05,
    TransferBlock = 0x06,
    TransferAbort = 0x07,
    WriteAbort = 0x08,
    Delay = 0x09,
    ResetTarget = 0x0A,
    SWJPins = 0x10,
    SWJClock = 0x11,
    SWJSequence = 0x12,
    SWDConfigure = 0x13,
    JTAGSequence = 0x14,
    JTAGConfigure = 0x15,
    JTAGIDCode = 0x16,
    SWOTransport = 0x17,
    SWOMode = 0x18,
    SWOBaudrate = 0x19,
    SWOControl = 0x1A,
    SWOStatus = 0x1B,
    SWOData = 0x1C,
    SWDSequence = 0x1D,
    SWOExtendedStatus = 0x1E,
    Unsupported = 0xFF,
}

pub trait CommandProperties {
    const TYPE: CommandType;
}

pub struct StandardResponse<C, Buf> {
    inner: Packet<Buf>,
    _command: core::marker::PhantomData<C>,
}

impl<C: CommandProperties, Buf: AsMut<[u8]>> StandardResponse<C, Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, C::TYPE)?;
        if inner.get_payload_mut().len() >= 1 {
            Ok(Self{inner, _command: core::marker::PhantomData})
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn commit_ok(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0;
        self.inner.commit(1)
    }

    pub fn commit_err(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0xFF;
        self.inner.commit(1)
    }
}

pub struct Packet<Buf> {
    pub command: CommandType,
    inner: Buf
}

impl<Buf: AsRef<[u8]>> Packet<Buf> {
    pub fn try_parse(inner: Buf) -> Result<Self, Error> {
        let command_byte = inner.as_ref().get(0).ok_or(Error::BufferNotBigEnough)?;
        match CommandType::try_from(*command_byte) {
            Ok(command) => Ok( Self { command, inner } ),
            Err(command_error) => Err(Error::UnsupportedCommand(command_error.number)),
        }
    }

    pub fn get_command(&self) -> CommandType {
        self.command
    }

    pub fn get_payload(&self) -> &[u8] {
        &self.inner.as_ref()[1..]
    }
}

impl<Buf: AsMut<[u8]>> Packet<Buf> {
    pub fn try_alloc(mut inner: Buf, command: CommandType) -> Result<Self, Error> {
        if inner.as_mut().len() >= 1 {
            Ok(Self { command, inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn get_payload_mut(&mut self) -> &mut [u8] {
        &mut self.inner.as_mut()[1..]
    }

    pub fn commit(mut self, payload_size: usize) -> usize {
        self.inner.as_mut()[0] = self.command as u8;
        payload_size + 1
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct TestCommandProps {}
    impl CommandProperties for TestCommandProps { const TYPE: CommandType = CommandType::Delay; }

    #[test]
    fn standard_response() {
        type TestCommandResponse<Buf> = StandardResponse<TestCommandProps, Buf>;

        let mut packet = [0u8; 2];

        let response = TestCommandResponse::try_alloc(&mut packet).expect("should be able to allocate response");
        assert_eq!(response.commit_ok(), 2);
        assert_eq!(packet, [9u8, 0u8]);

        let response = TestCommandResponse::try_alloc(&mut packet).expect("should be able to allocate response");
        assert_eq!(response.commit_err(), 2);
        assert_eq!(packet, [9u8, 0xFFu8]);

        let mut short_packet = [0u8; 1];
        assert!(matches!(TestCommandResponse::try_alloc(&mut short_packet), Err(Error::BufferNotBigEnough)), "shouldn't be able to allocate with a short buffer");
    }

}