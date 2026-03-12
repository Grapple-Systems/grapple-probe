// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use super::common::*;

pub struct JTAGSequenceCommand<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for JTAGSequenceCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::JTAGSequence);
        if inner.get_payload().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> JTAGSequenceCommand<Buf> {
    pub fn get_sequences<'a>(&'a self) -> JTAGSequenceIterator<'a, Buf> {
        let sequences_left = self.inner.get_payload()[0] as usize;
        JTAGSequenceIterator {
            parent: self,
            payload_idx: 1,
            sequences_left,
        }
    }
}

pub struct JTAGSequenceResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> JTAGSequenceResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::JTAGSequence)?;
        if inner.get_payload_mut().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn get_tdo_data_mut(&mut self) -> &mut[u8] {
        &mut self.inner.get_payload_mut()[1..]
    }

    pub fn commit_ok(mut self, tdo_size: usize) -> usize {
        self.inner.get_payload_mut()[0] = 0;
        self.inner.commit(1 + tdo_size)
    }

    pub fn commit_err(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0xFF;
        self.inner.commit(1)
    }
}

pub struct JTAGSequenceIterator<'a, Buf> {
    parent: &'a JTAGSequenceCommand<Buf>,
    payload_idx: usize,
    sequences_left: usize,
}

impl<'a, Buf: AsRef<[u8]>> Iterator for JTAGSequenceIterator<'a, Buf> {
    type Item = JTAGSequence<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sequences_left > 0 {
            if let Ok(sequence) = JTAGSequence::try_from(&self.parent.inner.get_payload()[self.payload_idx..]) {
                self.payload_idx += sequence.inner.len();
                self.sequences_left -= 1;
                return Some(sequence);
            }
        }
        None
    }
}

pub struct JTAGSequence<'a> {
    inner: &'a [u8],
    num_bits: usize,
}

impl<'a> TryFrom<&'a [u8]> for JTAGSequence<'a> {
    type Error = Error;

    fn try_from(inner: &'a [u8]) -> Result<Self, Self::Error> {
        if inner.as_ref().len() >= 1 {
            let num_bits = (inner.as_ref()[0] & 0x3F) as usize;
            let num_bits = if num_bits == 0 { 64usize } else { num_bits };
            let num_bytes = (num_bits + 7) / 8;
            if inner.as_ref().len() >= 1 + num_bytes {
                return Ok(Self { inner: &inner.as_ref()[0..1+num_bytes], num_bits });
            }
        }
        Err(Error::BufferNotBigEnough)
    }
}

impl<'a> JTAGSequence<'a> {
    pub fn get_num_bits(&self) -> usize  {
        self.num_bits
    }

    pub fn get_tms(&self) -> bool {
        self.inner.as_ref()[0] & 0x40 == 0x40
    }

    pub fn get_capture_tdo(&self) -> bool {
        self.inner.as_ref()[0] & 0x80 == 0x80
    }

    pub fn get_tdi_data(&self) -> &[u8] {
        &self.inner.as_ref()[1..]
    }
}

pub struct JTAGConfigureProps {}
impl CommandProperties for JTAGConfigureProps {
    const TYPE: CommandType = CommandType::JTAGConfigure;
}

pub struct JTAGConfigureCommand<Buf> {
    inner: Packet<Buf>,
    num_devices: usize,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for JTAGConfigureCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::JTAGConfigure);
        if inner.get_payload().len() >= 1 {
            let num_devices = inner.get_payload()[0] as usize;
            if inner.get_payload().len() >= (1 + num_devices) {
                return Ok(Self { inner, num_devices });
            }
        }
        Err(Error::BufferNotBigEnough)
    }
}

impl<Buf: AsRef<[u8]>> JTAGConfigureCommand<Buf> {
    pub fn get_ir_lengths(&self) -> &[u8] {
        &self.inner.get_payload()[1..(self.num_devices + 1)]
    }
}

pub type JTAGConfigureResponse<Buf> = StandardResponse<JTAGConfigureProps, Buf>;

pub struct JTAGIDCodeCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for JTAGIDCodeCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::JTAGIDCode);
        if inner.get_payload().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> JTAGIDCodeCommand<Buf> {
    pub fn get_index(&self) -> u8 {
        self.inner.get_payload()[0]
    }
}

pub struct JTAGIDCodeResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> JTAGIDCodeResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::JTAGIDCode)?;
        if inner.get_payload_mut().len() >= 5 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn commit_ok(mut self, idcode: u32) -> usize {
        self.inner.get_payload_mut()[0] = 0;
        self.inner.get_payload_mut()[1..5].copy_from_slice(&idcode.to_le_bytes());
        self.inner.commit(5)
    }

    pub fn commit_err(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0xFF;
        self.inner.commit(5)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn jtag_sequence_request() {
        let request_data = [20u8, 4, 65, 0, 1, 0, 128, 255, 255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255];

        let packet = Packet::try_parse(&request_data).expect("this is a valid packet");
        let request = JTAGSequenceCommand::try_from(packet).expect("this is a valid sequence packet");
        let mut iter = request.get_sequences();

        let sequence = iter.next().expect("should have this sequence");
        assert!(!sequence.get_capture_tdo());
        assert!(sequence.get_tms());
        assert_eq!(sequence.get_num_bits(), 1);
        assert_eq!(sequence.get_tdi_data(), &[0u8]);

        let sequence = iter.next().expect("should have this sequence");
        assert!(!sequence.get_capture_tdo());
        assert!(!sequence.get_tms());
        assert_eq!(sequence.get_num_bits(), 1);
        assert_eq!(sequence.get_tdi_data(), &[0u8]);

        let sequence = iter.next().expect("should have this sequence");
        assert!(sequence.get_capture_tdo());
        assert!(!sequence.get_tms());
        assert_eq!(sequence.get_num_bits(), 64);
        assert_eq!(sequence.get_tdi_data(), &[255u8, 255, 255, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn test_sequence_request2() {
        let request_data = [20u8, 30, 1, 0, 65, 0, 1, 0, 1, 0, 8, 135, 65, 14, 1, 0, 65, 0, 65, 0, 1, 0, 65, 0, 65, 0, 1, 0, 1, 0, 5, 28, 4, 255, 65, 255, 1, 1, 65, 0, 65, 0, 1, 0, 65, 0, 1, 0, 1, 0, 8, 135, 65, 254, 1, 254, 65, 0, 65, 0, 1, 0];

        let packet = Packet::try_parse(&request_data).expect("this is a valid packet");
        let request = JTAGSequenceCommand::try_from(packet).expect("this is a valid sequence packet");
        let mut iter = request.get_sequences();

        iter.next().expect("has 30 sequences");
        iter.next().expect("has 30 sequences");
    }

    #[test]
    fn test_jtag_configure_larger_payload() {
        let request_data = [0x15u8, 1, 4, 0];
        let packet = Packet::try_parse(&request_data).expect("valid packet");
        let request = JTAGConfigureCommand::try_from(packet).expect("valid jtag config command");

        assert_eq!(request.get_ir_lengths().len(), 1);
        assert_eq!(request.get_ir_lengths()[0], 4);
    }
}