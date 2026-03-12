// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use embassy_rp::{i2c, interrupt, Peri};

pub struct M24C64<'d, I: i2c::Instance> {
    bus: i2c::I2c<'d, I, i2c::Async>,
    addr: u8,
}

impl<'d, I: i2c::Instance> M24C64<'d, I> {
    pub fn new(addr: u8, peri: Peri<'d, I>, scl: Peri<'d, impl i2c::SclPin<I>>, sda: Peri<'d, impl i2c::SdaPin<I>>, irq: impl interrupt::typelevel::Binding<I::Interrupt, i2c::InterruptHandler<I>>) -> Self {
        let mut config = i2c::Config::default();
        config.frequency = 1_000_000;
        Self {
            bus: i2c::I2c::new_async(peri, scl, sda, irq, config),
            addr,
        }
    }

    pub async fn read(&mut self, address: u16, data: &mut [u8]) -> Result<(), i2c::Error> {
        let addr = 0b1010000 | (self.addr & 0b111);
        self.bus.write_read_async(addr, address.to_be_bytes(), data).await
    }
}