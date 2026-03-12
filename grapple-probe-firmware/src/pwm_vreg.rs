// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use embassy_rp::{gpio, Peri, pwm};

#[derive(Clone)]
pub struct Config {
    pub max_mv: u32,
    pub min_mv: u32,
    pub max_mv_count: u16,
    pub min_mv_count: u16,
}

impl Config {
    fn duty_count(&self, mv: u32) -> (u16, u32) {
        let mv_range = self.max_mv - self.min_mv;
        let mv = mv.min(self.max_mv).max(self.min_mv);

        let duty_count = if self.max_mv_count > self.min_mv_count {
            let count_range = self.max_mv_count - self.min_mv_count;
            (((mv - self.min_mv) * count_range as u32) / mv_range) as u16 + self.min_mv_count
        } else {
            let count_range = self.min_mv_count - self.max_mv_count;
            (((self.max_mv - mv) * count_range as u32) / mv_range) as u16 + self.max_mv_count
        };
        (duty_count, mv)
    }
}

pub struct PWMVoltageRegulator<'a> {
    pwm: pwm::Pwm<'a>,
    a_enable: Option<gpio::Output<'a>>,
    b_enable: Option<gpio::Output<'a>>,
    a_config: Config,
    b_config: Config,
    pwm_config: pwm::Config,
}

impl<'a> PWMVoltageRegulator<'a> {
    pub fn new_ab<P: pwm::Slice, A: pwm::ChannelAPin<P>, B: pwm::ChannelBPin<P>> (
        pwm: Peri<'a, P>,
        pin_a: Peri<'a, A>,
        pin_b: Peri<'a, B>,
        a_enable: Option<Peri<'a, gpio::AnyPin>>,
        b_enable: Option<Peri<'a, gpio::AnyPin>>,
        a_config: Config,
        b_config: Config,
    ) -> Self {
        let mut pwm_config = pwm::Config::default();
        pwm_config.top = 0xFFF;
        let pwm = pwm::Pwm::new_output_ab(pwm, pin_a, pin_b, pwm_config.clone());
        let a_enable = a_enable.map(|p| gpio::Output::new(p, false.into()));
        let b_enable = b_enable.map(|p| gpio::Output::new(p, false.into()));
        Self { pwm, a_enable, b_enable, a_config, b_config, pwm_config }
    }

    pub async fn set_voltage_mv_a(&mut self, mv: u32) -> u32 {
        if mv == 0 {
            if let Some(en) = self.a_enable.as_mut() {
                en.set_low();
            }
            self.pwm_config.compare_a = 0;
            self.pwm.set_config(&self.pwm_config);
            0
        }
        else {
            let (duty_count, mv) = self.a_config.duty_count(mv);
            self.pwm_config.compare_a = duty_count;
            self.pwm.set_config(&self.pwm_config);

            // wait 10 ms to let the pwm voltage generator settle
            embassy_time::Timer::after_millis(10).await;

            if let Some(en) = self.a_enable.as_mut() {
                en.set_high();
            }
            mv
        }
    }

    pub async fn set_voltage_mv_b(&mut self, mv: u32) -> u32 {
        if mv == 0 {
            if let Some(en) = self.b_enable.as_mut() {
                en.set_low();
            }
            self.pwm_config.compare_b = 0;
            self.pwm.set_config(&self.pwm_config);
            0
        }
        else {
            let (duty_count, mv) = self.b_config.duty_count(mv);
            self.pwm_config.compare_b = duty_count;
            self.pwm.set_config(&self.pwm_config);

            // wait 10 ms to let the pwm voltage generator settle
            embassy_time::Timer::after_millis(10).await;

            if let Some(en) = self.b_enable.as_mut() {
                en.set_high();
            }
            mv
        }
    }
}