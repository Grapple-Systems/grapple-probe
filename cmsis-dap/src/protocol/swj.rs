// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::Pins;
use super::common::*;

pub struct SWJPinsCommand<Buf> {
    inner: Packet<Buf>,
}


impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWJPinsCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::SWJPins);
        if inner.get_payload().len() >= 6 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWJPinsCommand<Buf> {
    pub fn get_output_pins(&self) -> Pins {
        Pins(self.inner.get_payload()[0])
    }

    pub fn get_selected_pins(&self) -> Pins {
        Pins(self.inner.get_payload()[1])
    }

    pub fn get_wait_us(&self) -> u32 {
        u32::from_le_bytes(self.inner.get_payload()[2..6].try_into().unwrap())
    }
}

pub struct SWJPinsResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> SWJPinsResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::SWJPins)?;
        if inner.get_payload_mut().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn commit(mut self, pins_input: Pins) -> usize {
        self.inner.get_payload_mut()[0] = pins_input.0;
        self.inner.commit(1)
    }
}

pub struct SWJClockCommand<Buf> {
    inner: Packet<Buf>,
}


impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWJClockCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::SWJClock);
        if inner.get_payload().len() >= 4 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWJClockCommand<Buf> {
    pub fn get_freq_hz(&self) -> u32 {
        u32::from_le_bytes(self.inner.get_payload()[..4].try_into().unwrap())
    }
}

pub struct SWJClockResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> SWJClockResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::SWJClock)?;
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

    pub fn commit_err(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0xFF;
        self.inner.commit(1)
    }
}

pub struct SWJSequenceCommand<Buf> {
    inner: Packet<Buf>,
    num_bits: usize,
}


impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWJSequenceCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::SWJSequence);
        if inner.get_payload().len() > 1 {
            let num_bits = inner.get_payload()[0];
            let num_bits = if num_bits == 0 { 256usize } else { num_bits as usize };
            let num_bytes = (num_bits + 7) / 8;
            if inner.get_payload().len() >= num_bytes + 1 {
                Ok(Self { num_bits, inner })
            } else {
                Err(Error::BufferNotBigEnough)
            }
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWJSequenceCommand<Buf> {
    pub fn get_sequence(&self) -> (usize, &[u8]) {
        (self.num_bits, &self.inner.get_payload()[1..])
    }
}

pub struct SWJSequenceResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> SWJSequenceResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::SWJSequence)?;
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

    pub fn commit_err(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0xFF;
        self.inner.commit(1)
    }
}