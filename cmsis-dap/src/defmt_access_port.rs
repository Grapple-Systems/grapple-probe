// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{SWJAccessPort, SWDAccessPort};
use crate::types::Pins;

pub struct DefmtAccessPort {}

impl SWJAccessPort for DefmtAccessPort {
    fn set_frequency(&mut self, freq_hz: u32) -> bool {
        defmt::info!("defmtap: set_frequency({})", freq_hz);
        true
    }

    fn write_sequence(&mut self, num_bits: usize, data: &[u8]) -> bool {
        defmt::info!("defmtap: swj sequence(num_pins={}, data={})", num_bits, data);
        true
    }

    fn pins(&mut self, out: Pins, mask: Pins, wait_us: u32) -> Pins {
        defmt::info!("defmtap: pins(out={}, mask={}, wait_us={})", out.0, mask.0, wait_us);
        Pins(0)
    }
}

impl SWDAccessPort for DefmtAccessPort {
    const SUPPORTED: bool = true;

    fn open(&mut self) -> bool {
        defmt::info!("defmtap: open");
        true
    }

    fn close(&mut self) -> bool {
        defmt::info!("defmtap: close");
        true
    }

    fn set_turnaround(&mut self, value: usize) {
        defmt::info!("defmtap: set turnaround {}", value);
    }

    fn read(&mut self, num_bits: usize, data: &mut [u8]) -> bool {
        defmt::info!("defmtap: read(num_bits={}, data_len={})", num_bits, data.len());
        true
    }

    fn write(&mut self, num_bits: usize, data: &[u8]) -> bool {
        defmt::info!("defmtap: write(num_bits={}, data={})", num_bits, data);
        true
    }
}