// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use embassy_rp::{i2c, Peri};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Config(i2c::ConfigError),
    Protocol(i2c::Error),
}

impl embedded_hal_async::i2c::Error for Error {
    fn kind(&self) -> embedded_hal_async::i2c::ErrorKind {
        match self {
            Self::Config(_) => embedded_hal_async::i2c::ErrorKind::Other,
            Self::Protocol(e) => e.kind(),
        }
    }
}

pub struct I2CDevice<'d, I: i2c::Instance> {
    inner: i2c::I2c<'d, I, i2c::Async>,
    config: i2c::Config,
}

impl<'d, I: i2c::Instance> I2CDevice<'d, I> {
    pub fn new(
        peri: Peri<'d, I>,
        scl: Peri<'d, impl i2c::SclPin<I>>,
        sda: Peri<'d, impl i2c::SdaPin<I>>,
        irq: impl embassy_rp::interrupt::typelevel::Binding<I::Interrupt, i2c::InterruptHandler<I>>,
        config: i2c::Config,
    ) -> Self {
        Self {
            inner: i2c::I2c::new_async(peri, scl, sda, irq, config),
            config,
        }
    }
}

impl<'d, I: i2c::Instance> embedded_hal_async::i2c::ErrorType for I2CDevice<'d, I> {
    type Error = Error;
}

impl<'d, I: i2c::Instance> embedded_hal_async::i2c::I2c for I2CDevice<'d, I> {

    async fn transaction(
            &mut self,
            address: u8,
            operations: &mut [embedded_hal_async::i2c::Operation<'_>],
        ) -> Result<(), Self::Error> {
        self.inner.transaction(address, operations).await.map_err(|e| Error::Protocol(e))
    }
}

impl<'d, I: i2c::Instance> crate::I2CDevice for I2CDevice<'d, I> {
    fn configure(&mut self, freq_hz: u32) -> Result<(), Self::Error> {
        use embassy_embedded_hal::SetConfig;

        self.config.frequency = freq_hz;
        self.inner.set_config(&self.config).map_err(|e| Error::Config(e))
    }
}