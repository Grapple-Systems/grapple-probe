// Copyright (c) 2025-2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg_attr(not(test), no_std)]

mod dap;
mod defmt_access_port;
mod dispatcher;
mod packet_buffer;
mod protocol;
mod swo;
mod types;
mod usb;

#[cfg(feature = "rp")]
pub mod rp;

#[cfg(test)]
mod test_access_port;

pub use defmt_access_port::DefmtAccessPort;
pub use packet_buffer::PacketBuffer;
pub use swo::{SWO, SWOTask, SWOAccess};
pub use types::{HostStatus, Pins};
pub use usb::{CMSISDapClass, State};

pub const MAX_PACKET_SIZE: usize = 64;

pub trait Reactor {
    fn get_firmware_version(&self) -> Option<&str> { None }

    /// Called when a host status request is received
    fn host_status(&self, _status: HostStatus) -> impl core::future::Future<Output = ()> {
        async { () }
    }

    fn unrecognized_packet(&self, command: &[u8], response: &mut [u8]) -> impl core::future::Future<Output = usize> {
        defmt::warn!("received unsupported dap command: {}", command[0]);
        async { protocol::Error::respond(response).unwrap_or(0) }
    }

    fn activity(&self) {}
}

pub trait SWJAccessPort {
    /// Set the swclk frequency
    fn set_frequency(&mut self, freq_hz: u32) -> bool;

    /// Set the pins in the mask to the out values, wait for them to get to the output level up to the specified wait time, return the current pin values.
    fn pins(&mut self, _out: Pins, _mask: Pins, _wait_us: u32) -> Pins {
        Pins(0)
    }

    /// Clock out a sequence of bits on the access port.
    fn write_sequence(&mut self, num_bits: usize, data: &[u8]) -> bool;
}

pub trait SWDAccessPort {
    const SUPPORTED: bool = false;

    /// Setup the port to be ready to use
    fn open(&mut self) -> bool {
        unimplemented!();
    }

    /// Release the port so it could be used with JTAG
    fn close(&mut self) -> bool {
        unimplemented!();
    }

    fn set_turnaround(&mut self, _value: usize) {
        unimplemented!()
    }

    /// Read bits from swdio while clocking swclk
    fn read(&mut self, _num_bits: usize, _data: &mut [u8]) -> bool {
        unimplemented!()
    }

    /// Write bits to the swdio while clocking swclk
    fn write(&mut self, _num_bits: usize, _data: &[u8]) -> bool {
        unimplemented!()
    }
}

pub trait SWOPort {
    /// True if the port supports uart swo.
    const SUPPORTS_UART: bool;
    /// True if the port supports manchester swo.
    const SUPPORTS_MANCHESTER: bool;

    /// Change the mode to uart with the specified baud rate.
    fn set_mode_uart(&mut self, _baudrate: u32) -> bool {
        if Self::SUPPORTS_UART {
            unimplemented!("set_mode_uart not implemented");
        }
        false
    }

    /// Change the mode to manchester with the specified baud rate.
    fn set_mode_manchester(&mut self, _baudrate: u32) -> bool {
        if Self::SUPPORTS_MANCHESTER {
            unimplemented!("set_mode_manchester not implemented");
        }
        false
    }

    /// Called when swo is set to off
    fn close(&mut self);

    /// Read some trace data from the device, and return the number of bytes read.
    fn read_trace_data(&mut self, data: &mut [u8]) -> impl core::future::Future<Output = usize>;
}

pub trait JTAGAccessPort {
    const SUPPORTED: bool = false;

    /// Setup the port for JTAG.
    fn open(&mut self) -> bool {
        unimplemented!()
    }

    /// Close the port so it can be use as SWD.
    fn close(&mut self) -> bool {
        unimplemented!()
    }

    /// Write num_bits of tdi and read from tdo simultaneously while holding tms as passed.
    fn transfer(&mut self, _num_bits: usize, _tms: bool, _tdi: &[u8], _tdo: &mut [u8]) -> bool {
        unimplemented!()
    }

    /// Toggle tck for the specified number of cycles, with tms and tdi set as passed, ignoring tdo.
    fn clock(&mut self, _cycles: usize, _tms: bool, _tdi: bool) -> bool {
        unimplemented!()
    }
}

#[cfg(test)]
mod test_defmt {
    #[defmt::global_logger]
    struct Logger;

    unsafe impl defmt::Logger for Logger {
        fn acquire() {}
        unsafe fn flush() {}
        unsafe fn release() {}
        unsafe fn write(_bytes: &[u8]) {}
    }
}