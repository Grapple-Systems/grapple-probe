// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use embassy_rp::{gpio, Peri, pwm};

pub struct DebouncedInputPin<'a> {
    pin: gpio::Input<'a>,
    stable_duration: embassy_time::Duration,
    state: bool,
}

impl<'a> DebouncedInputPin<'a> {
    pub fn new(pin: Peri<'a, gpio::AnyPin>, pull: gpio::Pull, stable_duration: embassy_time::Duration) -> Self {
        let pin = gpio::Input::new(pin, pull);
        let state = pin.is_high();
        Self { pin, stable_duration, state }
    }

    pub fn get_state(&self) -> bool {
        self.state
    }

    pub async fn wait_change(&mut self) -> bool {
        loop {
            if self.state {
                self.pin.wait_for_low().await;
                if embassy_time::with_timeout(self.stable_duration, self.pin.wait_for_high()).await != Ok(()) {
                    self.state = false;
                    return self.state;
                }
            } else {
                self.pin.wait_for_high().await;
                if embassy_time::with_timeout(self.stable_duration, self.pin.wait_for_low()).await != Ok(()) {
                    self.state = true;
                    return self.state;
                }
            }
        }
    }
}

pub struct LED<'a> {
    pub red: pwm::PwmOutput<'a>,
    pub green: pwm::PwmOutput<'a>,
    pub blue: pwm::PwmOutput<'a>,
}

impl<'a> LED<'a> {
    pub fn set(&mut self, red: u8, green: u8, blue: u8) {
        use pwm::SetDutyCycle;

        _ = self.red.set_duty_cycle_percent(red);
        _ = self.green.set_duty_cycle_percent(green);
        _ = self.blue.set_duty_cycle_percent(blue);
    }
}