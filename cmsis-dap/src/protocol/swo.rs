// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::{SWOMode, SWOStatus, SWOTransport};
use super::common::*;

pub struct SWOTransportProps {}
impl CommandProperties for SWOTransportProps {
    const TYPE: CommandType = CommandType::SWOTransport;
}

pub struct SWOTransportCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWOTransportCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.get_command(), CommandType::SWOTransport);
        if inner.get_payload().len() >= 1 {
            Ok(Self{ inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWOTransportCommand<Buf> {
    pub fn try_get_transport(&self) -> Option<SWOTransport> {
        SWOTransport::try_from(self.inner.get_payload()[0]).ok()
    }
}

pub type SWOTransportResponse<Buf> = StandardResponse<SWOTransportProps, Buf>;

pub struct SWOModeProps {}
impl CommandProperties for SWOModeProps {
    const TYPE: CommandType = CommandType::SWOMode;
}

pub struct SWOModeCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWOModeCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.get_command(), CommandType::SWOMode);
        if inner.get_payload().len() >= 1 {
            Ok(Self{ inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWOModeCommand<Buf> {
    pub fn try_get_mode(&self) -> Option<SWOMode> {
        SWOMode::try_from(self.inner.get_payload()[0]).ok()
    }
}

pub type SWOModeResponse<Buf> = StandardResponse<SWOModeProps, Buf>;

pub struct SWOBaudrateCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWOBaudrateCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.get_command(), CommandType::SWOBaudrate);
        if inner.get_payload().len() >= 4 {
            Ok(Self{ inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWOBaudrateCommand<Buf> {
    pub fn get_baudrate(&self) -> u32 {
        u32::from_le_bytes(self.inner.get_payload()[..4].try_into().unwrap())
    }
}

pub struct SWOBaudrateResponse<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsMut<[u8]>> SWOBaudrateResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::SWOBaudrate)?;
        if inner.get_payload_mut().len() >= 4 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn commit(mut self, baudrate: u32) -> usize {
        self.inner.get_payload_mut()[..4].copy_from_slice(&baudrate.to_le_bytes());
        self.inner.commit(4)
    }
}

pub struct SWOControlProps {}
impl CommandProperties for SWOControlProps {
    const TYPE: CommandType = CommandType::SWOControl;
}

pub struct SWOControlCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWOControlCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.get_command(), CommandType::SWOControl);
        if inner.get_payload().len() >= 1 {
            Ok(Self{ inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWOControlCommand<Buf> {
    pub fn is_start(&self) -> bool {
        self.inner.get_payload()[0] != 0
    }
}

pub type SWOControlResponse<Buf> = StandardResponse<SWOControlProps, Buf>;

pub struct SWOStatusCommand {}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWOStatusCommand {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.get_command(), CommandType::SWOStatus);
        Ok(Self{})
    }
}

pub struct SWOStatusResponse<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsMut<[u8]>> SWOStatusResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::SWOStatus)?;
        if inner.get_payload_mut().len() >= 5 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn commit(mut self, status: &SWOStatus) -> usize {
        self.inner.get_payload_mut()[0] = status.flags.0;
        self.inner.get_payload_mut()[1..5].copy_from_slice(&status.buffered_traces.to_le_bytes());
        self.inner.commit(5)
    }
}

pub struct SWOStatusControl(pub u8);
impl SWOStatusControl {
    pub fn status_requested(&self) -> bool {
        (self.0 & 0x01) == 0x01
    }

    pub fn count_requested(&self) -> bool {
        (self.0 & 0x02) == 0x02
    }

    pub fn idxts_requested(&self) -> bool {
        (self.0 & 0x04) == 0x04
    }

    pub fn get_fields(&self) -> [u8; 3] {
        [(self.0 & 0x01), (self.0 >> 1 & 0x01), (self.0 >> 2 & 0x01)]
    }
}

pub struct SWOExtendedStatusCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWOExtendedStatusCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.get_command(), CommandType::SWOExtendedStatus);
        if inner.get_payload().len() >= 1 {
            Ok(Self{ inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWOExtendedStatusCommand<Buf> {
    pub fn get_control(&self) -> SWOStatusControl {
        SWOStatusControl(self.inner.get_payload()[0])
    }
}

pub struct SWOExtendedStatusResponse<Buf> {
    inner: Packet<Buf>,
    control: SWOStatusControl,
}

impl<Buf: AsMut<[u8]>> SWOExtendedStatusResponse<Buf> {
    pub fn try_alloc(inner: Buf, control: SWOStatusControl) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::SWOExtendedStatus)?;
        let payload_len: usize = control.get_fields().iter().
            zip([1usize, 4, 8].iter()).
            map(|(a, b)| *a as usize * b).
            sum();
        if inner.get_payload_mut().len() >= payload_len {
            Ok(Self{inner, control})
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn commit(mut self, status: &SWOStatus) -> usize {
        let mut idx = 0;
        if self.control.status_requested() {
            self.inner.get_payload_mut()[idx] = status.flags.0;
            idx += 1;
        }
        if self.control.count_requested() {
            self.inner.get_payload_mut()[idx..idx+4].copy_from_slice(&status.buffered_traces.to_le_bytes());
            idx += 4;
        }
        if self.control.idxts_requested() {
            self.inner.get_payload_mut()[idx..idx+4].copy_from_slice(&status.next_trace_index.to_le_bytes());
            self.inner.get_payload_mut()[idx+4..idx+8].copy_from_slice(&status.next_trace_timestamp.to_le_bytes());
            idx += 8;
        }
        self.inner.commit(idx)
    }
}

pub struct SWODataCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for SWODataCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.get_command(), CommandType::SWOData);
        if inner.get_payload().len() >= 2 {
            Ok(Self{ inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> SWODataCommand<Buf> {
    pub fn get_max_trace_count(&self) -> u16 {
        u16::from_le_bytes(self.inner.get_payload()[..2].try_into().unwrap())
    }
}

pub struct SWODataResponse<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsMut<[u8]>> SWODataResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::SWOData)?;
        if inner.get_payload_mut().len() >= 3 {
            Ok(Self{inner})
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn get_trace_data_mut(&mut self) -> &mut [u8] {
        &mut self.inner.get_payload_mut()[3..]
    }

    pub fn commit(mut self, status: &SWOStatus, trace_count: u16) -> usize {
        self.inner.get_payload_mut()[0] = status.flags.0;
        self.inner.get_payload_mut()[1..3].copy_from_slice(&trace_count.to_le_bytes());
        self.inner.commit(3 + trace_count as usize)
    }
}