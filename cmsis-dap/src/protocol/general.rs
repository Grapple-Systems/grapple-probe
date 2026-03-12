// Copyright (c) 2025-2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::{HostStatus, ConnectMode};
use super::common::*;

pub struct InfoCommand<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for InfoCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Error> {
        assert_eq!(inner.command, CommandType::Info);
        if inner.get_command() == CommandType::Info && inner.get_payload().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> InfoCommand<Buf> {
    pub fn get_id(&self) -> u8 {
        self.inner.get_payload()[0]
    }
}

pub struct InfoResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> InfoResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut packet = Packet::try_alloc(inner, CommandType::Info)?;
        if packet.get_payload_mut().len() >= 1 {
            Ok(Self { inner: packet })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn get_payload_mut(&mut self) -> &mut [u8] {
        &mut self.inner.get_payload_mut()[1..]
    }

    pub fn commit(mut self, payload_size: usize) -> usize {
        self.inner.get_payload_mut()[0] = payload_size as u8;
        self.inner.commit(payload_size + 1)
    }

    pub fn commit_bytes(mut self, bytes: &[u8]) -> usize {
        self.get_payload_mut()[..bytes.len()].copy_from_slice(bytes);
        self.commit(bytes.len())
    }

    pub fn commit_unrecognized(self) -> usize {
        self.commit(0)
    }
}

pub struct HostStatusCommand<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for HostStatusCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::HostStatus);
        if inner.get_payload().len() >= 2 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> HostStatusCommand<Buf> {
    pub fn get(&self) -> HostStatus {
        match self.inner.get_payload()[0] {
            0 => HostStatus::Connected(self.inner.get_payload()[1] != 0),
            1 => HostStatus::Running(self.inner.get_payload()[1] != 0),
            _ => HostStatus::Unknown(self.inner.get_payload()[0], self.inner.get_payload()[1]),
        }
    }
}

pub struct HostStatusResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> HostStatusResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::HostStatus)?;
        if inner.get_payload_mut().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn commit(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0;
        self.inner.commit(1)
    }
}

pub struct ConnectCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for ConnectCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::Connect);
        if inner.get_payload().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> ConnectCommand<Buf> {
    pub fn get_mode(&self) -> ConnectMode {
        match self.inner.get_payload()[0] {
            0 => ConnectMode::Default,
            1 => ConnectMode::SWD,
            2 => ConnectMode::JTAG,
            value => ConnectMode::Unknown(value),
        }
    }
}

pub struct ConnectResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> ConnectResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::Connect)?;
        if inner.get_payload_mut().len() >= 1 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn commit_ok(mut self, mode: ConnectMode) -> usize {
        self.inner.get_payload_mut()[0] = match mode {
            ConnectMode::SWD => 1,
            ConnectMode::JTAG => 2,
            _ => 0,
        };
        self.inner.commit(1)
    }

    pub fn commit_err(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0;
        self.inner.commit(1)
    }
}

pub struct DisconnectCommand<Buf> {
    _inner: Packet<Buf>,
}


impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for DisconnectCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::Disconnect);
        Ok(Self { _inner: inner })
    }
}

pub struct DisconnectResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> DisconnectResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::Disconnect)?;
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

pub struct WriteAbortCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for WriteAbortCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::WriteAbort);
        if inner.get_payload().len() >= 5 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> WriteAbortCommand<Buf> {
    pub fn get_dap_index(&self) -> u8 {
        self.inner.get_payload()[0]
    }

    pub fn get_abort_value(&self) -> u32 {
        u32::from_le_bytes(self.inner.get_payload()[1..5].try_into().unwrap())
    }
}

pub struct WriteAbortResponse<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsMut<[u8]>> WriteAbortResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::WriteAbort)?;
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

pub struct DelayCommand<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for DelayCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::Delay);
        if inner.get_payload().len() >= 2 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> DelayCommand<Buf> {
    pub fn get_delay_us(&self) -> u16 {
        u16::from_le_bytes(self.inner.get_payload()[..2].try_into().unwrap())
    }
}

pub struct DelayResponse<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsMut<[u8]>> DelayResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::Delay)?;
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

pub struct ResetTargetCommand<Buf> {
    _inner: Packet<Buf>
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for ResetTargetCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::ResetTarget);
        Ok(Self { _inner: inner })
    }
}

pub struct ResetTargetResponse<Buf> {
    inner: Packet<Buf>,
    device_specific: bool,
}

impl<Buf: AsMut<[u8]>> ResetTargetResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::ResetTarget)?;
        if inner.get_payload_mut().len() >= 2 {
            Ok(Self { inner, device_specific: false })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn set_device_specific(&mut self, value: bool) {
        self.device_specific = value;
    } 

    pub fn commit_ok(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0;
        self.inner.get_payload_mut()[1] = if self.device_specific { 1 } else { 0 };
        self.inner.commit(2)
    }

    #[allow(dead_code)]
    pub fn commit_err(mut self) -> usize {
        self.inner.get_payload_mut()[0] = 0xFF;
        self.inner.get_payload_mut()[1] = if self.device_specific { 1 } else { 0 };
        self.inner.commit(2)
    }
}