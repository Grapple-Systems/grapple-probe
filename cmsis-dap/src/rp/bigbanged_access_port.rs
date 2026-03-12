// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use defmt;
use embassy_rp::{gpio, Peri};

pub struct Config {
    pub swclk_tck: Peri<'static, gpio::AnyPin>,
    pub swdio_tms: Peri<'static, gpio::AnyPin>,
    pub tdi: Option<Peri<'static, gpio::AnyPin>>,
    pub swo_tdo: Option<Peri<'static, gpio::AnyPin>>,
    pub swclk_tck_dir: Option<Peri<'static, gpio::AnyPin>>,
    pub swdio_tms_dir: Option<Peri<'static, gpio::AnyPin>>,
    pub tdi_dir: Option<Peri<'static, gpio::AnyPin>>,
    pub swo_tdo_dir: Option<Peri<'static, gpio::AnyPin>>,
    pub nreset: Option<Peri<'static, gpio::AnyPin>>,
    pub treset: Option<Peri<'static, gpio::AnyPin>>,
    pub uart_swo: bool,
}

impl Config {
    pub fn swd_only(swclk_tck: Peri<'static, gpio::AnyPin>, swdio_tms: Peri<'static, gpio::AnyPin>) -> Self {
        Self {
            swclk_tck, swdio_tms,
            tdi: None,
            swo_tdo: None,
            swclk_tck_dir: None,
            swdio_tms_dir: None,
            tdi_dir: None,
            swo_tdo_dir: None,
            nreset: None,
            treset: None,
            uart_swo: false,
        }
    }

    pub fn swd_jtag(swclk_tck: Peri<'static, gpio::AnyPin>, swdio_tms: Peri<'static, gpio::AnyPin>, tdi: Peri<'static, gpio::AnyPin>, swo_tdo: Peri<'static, gpio::AnyPin>) -> Self {
        Self {
            swclk_tck, swdio_tms,
            tdi: Some(tdi),
            swo_tdo: Some(swo_tdo),
            swclk_tck_dir: None,
            swdio_tms_dir: None,
            tdi_dir: None,
            swo_tdo_dir: None,
            nreset: None,
            treset: None,
            uart_swo: false,
        }
    }
}

pub struct BitbangedAccessPort<'a> {
    swclk_tck: gpio::Flex<'a>,
    swdio_tms: gpio::Flex<'a>,
    tdi: Option<gpio::Flex<'a>>,
    swo_tdo: Option<gpio::Flex<'a>>,
    nreset: Option<gpio::OutputOpenDrain<'a>>,
    treset: Option<gpio::OutputOpenDrain<'a>>,
    tick_duration: embassy_time::Duration,

    tdi_pinctrl: Option<IOFunctionControl>,
    swo_tdo_pinctrl: Option<IOFunctionControl>,

    swdio_tms_dir: Option<gpio::Output<'a>>,
    _swclk_tck_dir: Option<gpio::Output<'a>>,
    _tdi_dir: Option<gpio::Output<'a>>,
    _swo_tdo_dir: Option<gpio::Output<'a>>,
}

impl<'a> BitbangedAccessPort<'a> {
    pub fn new(config: Config) -> Self 
    {
        let mut swclk_tck = gpio::Flex::new(config.swclk_tck);
        swclk_tck.set_high();
        let swclk_dir = config.swclk_tck_dir.map(|pin| {
            gpio::Output::new(pin, gpio::Level::High)
        });
        swclk_tck.set_as_output();

        let mut swdio_tms = gpio::Flex::new(config.swdio_tms);
        swdio_tms.set_high();
        let swdio_tms_dir = config.swdio_tms_dir.map(|pin| {
            gpio::Output::new(pin, gpio::Level::High)
        });
        swdio_tms.set_as_output();

        let tdi_pinctrl = config.tdi.as_ref().map(|pin| unsafe {IOFunctionControl::steal(pin)});
        let mut tdi = config.tdi.map(|pin| {
            let mut tdi = gpio::Flex::new(pin);
            tdi.set_low();
            tdi
        });
        let tdi_dir = config.tdi_dir.map(|pin| {
            gpio::Output::new(pin, gpio::Level::High)
        });
        if let Some(tdi) = tdi.as_mut() {
            tdi.set_as_output();
        }

        let swo_tdo_pinctrl = config.swo_tdo.as_ref().map(|pin| unsafe {IOFunctionControl::steal(pin)});
        let swo_tdo = config.swo_tdo.map(|pin| {
            let mut swo_tdo = gpio::Flex::new(pin);
            swo_tdo.set_as_input();
            swo_tdo
        });
        let swo_tdo_dir = config.swo_tdo_dir.map(|pin| {
            gpio::Output::new(pin, gpio::Level::Low)
        });

        let nreset = config.nreset.map(|pin| gpio::OutputOpenDrain::new(pin, gpio::Level::High));
        let treset = config.treset.map(|pin| gpio::OutputOpenDrain::new(pin, gpio::Level::High));

        Self {
            swclk_tck,
            swdio_tms,
            tdi,
            swo_tdo,
            nreset,
            treset,
            tick_duration: embassy_time::Duration::from_micros(1),

            tdi_pinctrl,
            swo_tdo_pinctrl,

            swdio_tms_dir,
            _swclk_tck_dir: swclk_dir,
            _tdi_dir: tdi_dir,
            _swo_tdo_dir: swo_tdo_dir,
        }
    }
}

impl<'a> crate::SWJAccessPort for BitbangedAccessPort<'a> {
    fn set_frequency(&mut self, freq_hz: u32) -> bool {
        self.tick_duration = embassy_time::Duration::from_micros(
            (500000 / core::cmp::min(freq_hz, 500000)) as u64);
        defmt::info!("bbap: set tick duration to {} us", self.tick_duration.as_micros());
        true
    }

    fn pins(&mut self, out: crate::Pins, mask: crate::Pins, wait_us: u32) -> crate::Pins {
        // set the pins
        if mask.get_swclk_tck() {
            self.swclk_tck.set_level(out.get_swclk_tck().into());
        }
        if mask.get_swdio_tms() {
            self.swdio_tms.set_level(out.get_swdio_tms().into());
        }
        if mask.get_tdi() {
            if let Some(tdi) = self.tdi.as_mut() {
                tdi.set_level(out.get_tdi().into());
            }
        }
        if mask.get_nreset() {
            if let Some(nreset) = self.nreset.as_mut() {
                nreset.set_level(out.get_nreset().into());
            }
        }
        if mask.get_ntrst() {
            if let Some(ntrst) = self.treset.as_mut() {
                ntrst.set_level(out.get_ntrst().into());
            }
        }

        // wait for the pins to be what we set them to
        let pin_fut = async {
            if mask.get_swclk_tck() {
                if out.get_swclk_tck() {
                    self.swclk_tck.wait_for_high().await;
                } else {
                    self.swclk_tck.wait_for_low().await;
                }
            }
            if mask.get_swdio_tms() {
                if out.get_swdio_tms() {
                    self.swdio_tms.wait_for_high().await;
                } else {
                    self.swdio_tms.wait_for_low().await;
                }
            }
            if mask.get_tdi() {
                if let Some(tdi) = self.tdi.as_mut() {
                    if out.get_tdi() {
                        tdi.wait_for_high().await;
                    } else {
                        tdi.wait_for_low().await;
                    }
                }
            }
            if mask.get_nreset() {
                if let Some(nreset) = self.nreset.as_mut() {
                    if out.get_nreset() {
                        nreset.wait_for_high().await;
                    } else {
                        nreset.wait_for_low().await;
                    }
                }
            }
            if mask.get_ntrst() {
                if let Some(ntrst) = self.treset.as_mut() {
                    if out.get_ntrst() {
                        ntrst.wait_for_high().await;
                    } else {
                        ntrst.wait_for_low().await;
                    }
                }
            }
        };
        embassy_futures::block_on(embassy_time::with_timeout(embassy_time::Duration::from_micros(wait_us as u64), pin_fut)).ok();

        // read the pin values
        let mut in_pins = crate::Pins(0);
        if mask.get_swclk_tck() {
            in_pins.set_swclk_tck(self.swclk_tck.is_high());
        }
        if mask.get_swdio_tms() {
            in_pins.set_swdio_tms(self.swdio_tms.is_high());
        }
        if mask.get_tdi() {
            in_pins.set_tdi(self.tdi.as_ref().map(|pin| pin.is_high()).unwrap_or(false));
        }
        if mask.get_tdi() {
            in_pins.set_tdo(self.swo_tdo.as_ref().map(|pin| pin.is_high()).unwrap_or(false));
        }
        if mask.get_nreset() {
            in_pins.set_nreset(self.nreset.as_ref().map(|pin| pin.is_high()).unwrap_or(false));
        }
        if mask.get_ntrst() {
            in_pins.set_ntrst(self.treset.as_ref().map(|pin| pin.is_high()).unwrap_or(false));
        }

        in_pins
    }

    fn write_sequence(&mut self, num_bits: usize, data: &[u8]) -> bool {
        // even jtag is supposed to do this on swdio/tms so always use the swd write.
        <Self as crate::SWDAccessPort>::write(self, num_bits, data)
    }
}

impl<'a> crate::SWDAccessPort for BitbangedAccessPort<'a> {
    const SUPPORTED: bool = true;

    fn open(&mut self) -> bool {
        if let Some(pinctrl) = self.tdi_pinctrl.as_ref() {
            pinctrl.set_function(&IOFunction::SWO);
        }
        if let Some(pinctrl) = self.swo_tdo_pinctrl.as_ref() {
            pinctrl.set_function(&IOFunction::SWO);
        }
        true
    }

    fn close(&mut self) -> bool {
        true
    }

    fn read(&mut self, mut num_bits: usize, data: &mut [u8]) -> bool {
        assert!(num_bits <= (data.len() * 8));

        self.swdio_tms.set_as_input();
        if let Some(dir) = self.swdio_tms_dir.as_mut() {
            dir.set_low();
        }

        let mut ticker = embassy_time::Ticker::every(self.tick_duration);

        for b in data {
            let b_bits = core::cmp::min(num_bits, 8);
            *b = 0;
            for i in 0..b_bits {
                embassy_futures::block_on(ticker.next());
                self.swclk_tck.set_low();
                if self.swdio_tms.is_high() {
                    *b |= 1 << i;
                }
                embassy_futures::block_on(ticker.next());
                self.swclk_tck.set_high();
            }
            num_bits -= b_bits;
        }

        true
    }

    fn write(&mut self, mut num_bits: usize, data: &[u8]) -> bool {
        assert!(num_bits <= (data.len() * 8));

        if let Some(dir) = self.swdio_tms_dir.as_mut() {
            dir.set_high();
        }
        self.swdio_tms.set_as_output();

        let mut ticker = embassy_time::Ticker::every(self.tick_duration);

        for b in data {
            let b_bits = core::cmp::min(num_bits, 8);
            for i in 0..b_bits {
                let bit = (*b & (1 << i)) != 0;
                embassy_futures::block_on(embassy_time::Timer::after(embassy_time::Duration::from_micros(5)));
                self.swclk_tck.set_low();
                self.swdio_tms.set_level(bit.into());
                embassy_futures::block_on(embassy_time::Timer::after(embassy_time::Duration::from_micros(5)));
                self.swclk_tck.set_high();
            }
            num_bits -= b_bits;
        }

        true
    }
}

impl<'a> crate::JTAGAccessPort for BitbangedAccessPort<'a> {
    const SUPPORTED: bool = true;

    fn open(&mut self) -> bool {
        if self.tdi.is_none() || self.swo_tdo.is_none() {
            return false;
        }

        if let Some(pinctrl) = self.tdi_pinctrl.as_ref() {
            pinctrl.set_function(&IOFunction::Gpio);
        }
        if let Some(pinctrl) = self.swo_tdo_pinctrl.as_ref() {
            pinctrl.set_function(&IOFunction::Gpio);
        }

        if let Some(dir) = self.swdio_tms_dir.as_mut() {
            dir.set_high();
        }
        self.swdio_tms.set_as_output();

        true
    }

    fn close(&mut self) -> bool {
        true
    }

    fn clock(&mut self, cycles: usize, tms: bool, tdi: bool) -> bool {
        self.swdio_tms.set_level(tms.into());
        self.tdi.as_mut().expect("should have tdi").set_level(tdi.into());

        let mut ticker = embassy_time::Ticker::every(self.tick_duration);
        for _ in 0..cycles {
            self.swclk_tck.set_low();
            embassy_futures::block_on(embassy_time::Timer::after(embassy_time::Duration::from_micros(5)));
            self.swclk_tck.set_high();
            embassy_futures::block_on(embassy_time::Timer::after(embassy_time::Duration::from_micros(5)));
        }

        true
    }

    fn transfer(&mut self, mut num_bits: usize, tms: bool, tdi: &[u8], tdo: &mut [u8]) -> bool {
        let tdi_pin = self.tdi.as_mut().expect("should have the tdi pin");
        let tdo_pin = self.swo_tdo.as_mut().expect("should have the tdo pin");

        let num_bytes = core::cmp::min((num_bits + 7) / 8, tdi.len());

        self.swdio_tms.set_level(tms.into());

        //let mut ticker = embassy_time::Ticker::every(self.tick_duration);

        for i in 0..num_bytes {
            if let Some(byte) = tdo.get_mut(i) {
                *byte = 0
            }
            let b_bits = core::cmp::min(num_bits, 8);
            num_bits -= b_bits;
            for bi in 0..b_bits {
                let bit = (tdi[i] >> bi) & 0x01 == 0x01;
                tdi_pin.set_level(bit.into());
                self.swclk_tck.set_low();
                embassy_futures::block_on(embassy_time::Timer::after(embassy_time::Duration::from_micros(5)));
                //ticker.next().await;

                if let Some(byte) = tdo.get_mut(i) {
                    if tdo_pin.is_high() {
                        *byte |= 1 << bi;
                    }
                }
                self.swclk_tck.set_high();
                embassy_futures::block_on(embassy_time::Timer::after(embassy_time::Duration::from_micros(5)));
                //ticker.next().await;
            }
        }
        true
    }
}

enum IOFunction {
    Gpio,
    SWO,
}

impl IOFunction {
    fn get_pac_funcsel(&self) -> u8 {
        match self {
            IOFunction::Gpio => 5,
            IOFunction::SWO => 2,
        }
    } 
}

struct IOFunctionControl {
    pin: Peri<'static, gpio::AnyPin>,
}

impl IOFunctionControl {
    pub unsafe fn steal(pin: &gpio::AnyPin) -> Self {
        use embassy_rp::gpio::Pin;
        Self {
            pin: gpio::AnyPin::steal(pin.pin()),
        }
    }

    pub fn set_function(&self, func: &IOFunction) {
        self.gpio().ctrl().modify(|reg| {
            reg.set_funcsel(func.get_pac_funcsel());
        })
    }

    fn gpio(&self) -> embassy_rp::pac::io::Gpio {
        use embassy_rp::gpio::Pin;
        let bank = match self.pin.bank() {
            embassy_rp::gpio::Bank::Bank0 => embassy_rp::pac::IO_BANK0,
        };
        bank.gpio(self.pin.pin() as _)
    }
}