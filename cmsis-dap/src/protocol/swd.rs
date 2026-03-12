// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use super::common::*;

pub struct SWDConfigureCommand<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWDConfigureCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::SWDConfigure);
        if inner.get_payload().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWDConfigureCommand<Buf> {
    pub fn get_turnaround(&self) -> usize {
        (self.inner.get_payload()[0] & 0x03) as usize + 1
    }

    pub fn get_always_data_phase(&self) -> bool {
        (self.inner.get_payload()[0] & 0x04) != 0
    }
}

pub struct SWDConfigureResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> SWDConfigureResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::SWDConfigure)?;
        if inner.get_payload_mut().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn commit_ok(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0;
        self.inner.commit(1)
    }
}

pub struct SWDSequenceIterator<'a, Buf> {
    inner: &'a SWDSequenceCommand<Buf>,
    payload_index: usize,
    sequences_left: usize,
}

impl<'a, Buf: AsRef<[u8]>> Iterator for SWDSequenceIterator<'a, Buf> {
    type Item = (usize, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.sequences_left > 0 {
            let i = self.payload_index;
            let info = self.inner.inner.get_payload().get(i)?;
            let is_output = (info & 0x80) == 0;
            let num_bits = (info & 0x3F) as usize;
            if is_output {
                let num_bytes = (num_bits + 7) / 8;
                let data = self.inner.inner.get_payload().get(i+1..i+1+num_bytes)?;
                self.payload_index += data.len() + 1;
                self.sequences_left -= 1;
                Some((num_bits, data))
            } else {
                self.payload_index += 1;
                self.sequences_left -= 1;
                Some((num_bits, &[]))
            }
        } else {
            None
        }
    }
}

pub struct SWDSequenceCommand<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWDSequenceCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::SWDSequence);
        if inner.get_payload().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWDSequenceCommand<Buf> {
    pub fn sequences<'a>(&'a self) -> SWDSequenceIterator<'a, Buf> {
        let num_sequences = self.inner.get_payload()[0] as usize;
        SWDSequenceIterator {
            inner: self,
            payload_index: 1,
            sequences_left: num_sequences,
        }
    }
}

pub struct SWDSequenceData<'a, Buf> {
    parent: &'a mut SWDSequenceResponse<Buf>,
    num_bytes: usize,
}

impl<'a, Buf: AsMut<[u8]>> SWDSequenceData<'a, Buf> {
    pub fn data(&mut self) -> &mut [u8] {
        let i = self.parent.packet_index;
        &mut self.parent.inner.get_payload_mut()[i..i+self.num_bytes]
    }

    pub fn commit(self) {
        self.parent.packet_index += self.num_bytes;
    }
}

pub struct SWDSequenceResponse<Buf> {
    inner: Packet<Buf>,
    packet_index: usize,
}

impl<Buf: AsMut<[u8]>> SWDSequenceResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::SWDSequence)?;
        if inner.get_payload_mut().len() >= 1 {
            Ok(Self { inner, packet_index: 1 })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn try_alloc_data<'a>(&'a mut self, num_bits: usize) -> Option<SWDSequenceData<'a, Buf>> {
        let num_bytes = (num_bits + 7) / 8;
        if (self.inner.get_payload_mut().len() - self.packet_index) >= num_bytes {
            Some(SWDSequenceData {
                parent: self,
                num_bytes
            })
        } else {
            None
        }
    }

    pub fn commit_ok(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0;
        self.inner.commit(self.packet_index)
    }

    pub fn commit_err(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0xFF;
        self.inner.commit(self.packet_index)
    }
}