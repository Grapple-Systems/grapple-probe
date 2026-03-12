// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

const ADC_CLOCK_RATE_HZ: usize = 48_000_000;
const ADC_VREF_MV: u32 = 3300;

pub struct ADCSweep {
    pub tvcc_mv: u32,
}

pub struct ADC<'a> {
    pub adc: embassy_rp::adc::Adc<'a, embassy_rp::adc::Async>,
    pub adc_dma: embassy_rp::dma::Channel<'a>,
    pub tvcc_channel: embassy_rp::adc::Channel<'a>,
}

impl<'a> ADC<'a> {
    pub async fn read(&mut self) -> ADCSweep {
        // sample at 1000 Hz
        let div = (ADC_CLOCK_RATE_HZ / 1000 - 1) as u16;
        let mut samples = [0u16; 100];
        self.adc.read_many(&mut self.tvcc_channel, &mut samples, div, &mut self.adc_dma).await.expect("failed to read the adc");
        let avg = samples.iter().map(|v| *v as u32).sum::<u32>() / samples.len() as u32;
        let mv = avg * ADC_VREF_MV / (1 << 12);
        let tvcc_mv = 2 * mv;

        ADCSweep {
            tvcc_mv
        }
    }
}