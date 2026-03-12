// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use core::sync::atomic;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConnectMode {
    Default,
    SWD,
    JTAG,
    Unknown(u8),
}

#[derive(Clone, PartialEq)]
pub enum HostStatus {
    Connected(bool),
    Running(bool),
    Unknown(u8, u8),
}

#[derive(Clone, Copy)]
pub struct Register {
    pub ap: bool,
    pub address: u8,
}

impl Register {
    pub fn access_port(address: u8) -> Self {
        Self {
            ap: true,
            address,
        }
    }

    pub fn debug_port(address: u8) -> Self {
        Self {
            ap: false,
            address,
        }
    }

    pub fn rdbuff() -> Self {
        Self::debug_port(0x03)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransferRequest(pub u8);

impl TransferRequest {
    pub fn is_read(&self) -> bool {
        (self.0 & 0x02) != 0
    }

    pub fn is_value_match(&self) -> bool {
        (self.0 & 0x10) != 0
    }

    pub fn is_match_mask(&self) -> bool {
        (self.0 & 0x20) != 0
    }

    pub fn timestamp(&self) -> bool {
        (self.0 & 0x80) != 0
    }

    pub fn get_register(&self) -> Register {
        let address = (self.0 >> 2) & 0x03;
        if self.0 & 0x01 == 0 {
            Register::debug_port(address)
        } else {
            Register::access_port(address)
        }
    }
}

#[derive(Default)]
pub struct JTAGTransferAck(pub u8);

impl JTAGTransferAck {
    pub fn is_wait(&self) -> bool {
        self.0 == 1
    }

    pub fn is_ok(&self) -> bool {
        (self.0 & 0x06) != 0
    }

    pub fn is_no_response(&self) -> bool {
        self.0 == 7 || self.0 == 0
    }
}

#[derive(Default)]
pub struct SWDTransferAck(pub u8);

impl SWDTransferAck {
    pub fn is_ok(&self) -> bool {
        self.0 == 1
    }

    pub fn is_wait(&self) -> bool {
        self.0 == 2
    }

    pub fn is_fault(&self) -> bool {
        self.0 == 4
    }

    pub fn is_no_response(&self) -> bool {
        self.0 == 7
    }
}

pub enum TransferAck {
    JTAG(JTAGTransferAck),
    SWD(SWDTransferAck),
}

impl Default for TransferAck {
    fn default() -> Self {
        Self::SWD(Default::default())
    }
}

impl TransferAck {
    pub fn jtag(value: u8) -> Self {
        Self::JTAG(JTAGTransferAck(value))
    }

    pub fn swd(value: u8) -> Self {
        Self::SWD(SWDTransferAck(value))
    }

    pub fn get_inner(&self) -> u8 {
        match self {
            Self::JTAG(ack) => ack.0,
            Self::SWD(ack) => ack.0,
        }
    }

    pub fn is_wait(&self) -> bool {
        match self {
            Self::JTAG(ack) => ack.is_wait(),
            Self::SWD(ack) => ack.is_wait(),
        }
    }

    pub fn is_ok(&self) -> bool {
        match self {
            Self::JTAG(ack) => ack.is_ok(),
            Self::SWD(ack) => ack.is_ok(),
        }
    }

    pub fn is_fault(&self) -> bool {
        match self {
            Self::JTAG(_) => false,
            Self::SWD(ack) => ack.is_fault(),
        }
    }

    pub fn is_no_response(&self) -> bool {
        match self {
            Self::JTAG(ack) => ack.is_no_response(),
            Self::SWD(ack) => ack.is_no_response(),
        }
    }
}

#[derive(Default)]
pub struct TransferStatus {
    pub ack: TransferAck,
    pub protocol_error: bool,
    pub value_mismatch: bool,
}

impl TransferStatus {
    pub fn from_ack(ack: TransferAck) -> Self {
        Self {
            ack,
            protocol_error: false,
            value_mismatch: false,
        }
    }

    pub fn ok() -> Self {
        Self {
            ack: TransferAck::swd(1),
            protocol_error: false,
            value_mismatch: false,
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.ack.is_ok() || self.protocol_error || self.value_mismatch
    }
}

#[derive(Clone, Copy)]
pub struct Pins(pub u8);

impl Pins {
    pub fn only_writable(self) -> Self {
        Self(self.0 & !0x08)
    }

    pub fn get_swclk_tck(&self) -> bool {
        self.0 & 0x01 != 0
    }

    pub fn set_swclk_tck(&mut self, value: bool) {
        self.0 = (self.0 & !0x01) | if value { 0x01 } else { 0 };
    }

    pub fn get_swdio_tms(&self) -> bool {
        self.0 & 0x02 != 0
    }

    pub fn set_swdio_tms(&mut self, value: bool) {
        self.0 = (self.0 & !0x02) | if value { 0x02 } else { 0 };
    }

    pub fn get_tdi(&self) -> bool {
        self.0 & 0x04 != 0
    }

    pub fn set_tdi(&mut self, value: bool) {
        self.0 = (self.0 & !0x04) | if value { 0x04 } else { 0 };
    }

    pub fn get_tdo(&self) -> bool {
        self.0 & 0x08 != 0
    }

    pub fn set_tdo(&mut self, value: bool) {
        self.0 = (self.0 & !0x08) | if value { 0x08 } else { 0 };
    }

    pub fn get_ntrst(&self) -> bool {
        self.0 & 0x20 != 0
    }

    pub fn set_ntrst(&mut self, value: bool) {
        self.0 = (self.0 & !0x20) | if value { 0x20 } else { 0 };
    }

    pub fn get_nreset(&self) -> bool {
        self.0 & 0x80 != 0
    }

    pub fn set_nreset(&mut self, value: bool) {
        self.0 = (self.0 & !0x80) | if value { 0x80 } else { 0 };
    }
}

#[derive(Clone, Copy, PartialEq, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum SWOTransport {
    None = 0,
    SWOData = 1,
    USBBulkEndpoint = 2,
}

#[derive(Clone, Copy, PartialEq, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum SWOMode {
    Off = 0,
    Uart = 1,
    Manchester = 2,
}

impl Default for SWOMode {
    fn default() -> Self {
        Self::Off
    }
}

#[derive(Clone, PartialEq, Default)]
pub struct SWOStatus {
    pub flags: SWOTraceStatus,
    pub buffered_traces: u32,
    pub next_trace_index: u32,
    pub next_trace_timestamp: u32,
}

#[derive(Clone, Copy, PartialEq, Default)] 
pub struct SWOTraceStatus(pub u8);

impl SWOTraceStatus {
    pub fn set_active(&mut self, active: bool) {
        self.0 = (self.0 & !0x01) | if active { 0x01 } else { 0x00 };
    }

    pub fn set_stream_error(&mut self, error: bool) {
        self.0 = (self.0 & !0x40) | if error { 0x40 } else { 0x00 };
    }

    pub fn set_buffer_overrun(&mut self, overrun: bool) {
        self.0 = (self.0 & !0x80) | if overrun { 0x80 } else { 0x00 };
    }

    pub fn is_buffer_overrun(&self) -> bool {
        self.0 & 0x80 == 0x80
    }
}

pub struct AbortToken(atomic::AtomicBool);

impl AbortToken {
    pub fn new() -> Self {
        Self(atomic::AtomicBool::new(false))
    }

    pub fn abort(&self) {
        self.0.store(true, atomic::Ordering::Relaxed);
    }

    pub fn is_set(&self) -> bool {
        self.0.load(atomic::Ordering::Relaxed)
    }
}