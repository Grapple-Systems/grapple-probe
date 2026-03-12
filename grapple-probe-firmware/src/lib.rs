// Copyright (c) 2025-2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]

mod adc;
mod config_storage;
mod eeprom;
mod io;
mod pio_access_port;
mod pwm_vreg;

pub use adc::ADC;
pub use io::{DebouncedInputPin, LED};
pub use pio_access_port::{PioAccessPort, PioSWO};
pub use pwm_vreg::PWMVoltageRegulator;

use {defmt_rtt as _, panic_probe as _};
use embassy_rp::peripherals as periph;
use embassy_rp::{dma, flash, gpio, i2c, uart, Peri, pwm};
use embassy_sync::mutex;

pub type USBDriver<'a> = embassy_rp::usb::Driver<'a, periph::USB>;
pub type Uart = embassy_rp::uart::BufferedUart;
pub type AccessPort<'a> = pio_access_port::PioAccessPortPeriphs<'a, periph::PIO0, periph::UART0, Irqs, periph::PIN_8, periph::PIN_12,
    periph::PIN_13, periph::PIN_16, periph::PIN_9, periph::PIN_4, periph::PIN_3, 256>;
pub type Flash<'a> = flash::Flash<'a, periph::FLASH, flash::Async, {1024 * 1024}>;
pub type Storage<'a> = config_storage::ConfigStorage<Flash<'a>, 64>;

embassy_rp::bind_interrupts!(pub struct Irqs {
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<embassy_rp::peripherals::USB>;
    ADC_IRQ_FIFO => embassy_rp::adc::InterruptHandler;
    I2C0_IRQ => i2c::InterruptHandler<periph::I2C0>;
    I2C1_IRQ => i2c::InterruptHandler<periph::I2C1>;
    UART0_IRQ => embassy_rp::uart::BufferedInterruptHandler<periph::UART0>;
    UART1_IRQ => embassy_rp::uart::BufferedInterruptHandler<periph::UART1>;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<periph::PIO0>;
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<periph::PIO1>;
    DMA_IRQ_0 => dma::InterruptHandler<periph::DMA_CH0>, dma::InterruptHandler<periph::DMA_CH1>;
});

pub struct AlmostUart {
    base: Peri<'static, periph::UART1>,
    tx: Peri<'static, periph::PIN_24>,
    rx: Peri<'static, periph::PIN_21>,
}

impl AlmostUart {
    pub fn into_buffered<'a>(self, tx_buffer: &'a mut [u8], rx_buffer: &'a mut [u8]) -> uart::BufferedUart {
        let config = uart::Config::default();
        let uart = uart::BufferedUart::new(self.base, self.tx, self.rx, Irqs, tx_buffer, rx_buffer, config);
        uart
    }
}

static mut DEVICE_INFO: [u8; 32] = [0u8; 32];

pub struct Board {
    usb: Option<Peri<'static, periph::USB>>,
    access_port_periphs: Option<AccessPort<'static>>,
    target_power: Option<(Peri<'static, periph::PWM_SLICE5>, Peri<'static, periph::PIN_26>, Peri<'static, periph::PIN_27>, Peri<'static, periph::PIN_25>, Peri<'static, periph::PIN_28>)>,
    led_pins: Option<(Peri<'static, periph::PWM_SLICE1>, Peri<'static, periph::PWM_SLICE2>, Peri<'static, periph::PIN_18>, Peri<'static, periph::PIN_19>, Peri<'static, periph::PIN_20>)>,
    adc: Option<(Peri<'static, periph::ADC>, Peri<'static, periph::DMA_CH0>, Peri<'static, periph::PIN_29>)>,
    gnd_detect_pin: Option<Peri<'static, periph::PIN_2>>,
    key5v: Option<Peri<'static, periph::PIN_17>>,
    uart: Option<AlmostUart>,
    i2c: Option<(Peri<'static, periph::I2C1>, Peri<'static, periph::PIN_22>, Peri<'static, periph::PIN_23>)>,
    core1: Option<Peri<'static, periph::CORE1>>,
    eeprom: eeprom::M24C64<'static, periph::I2C0>,
    flash: Option<Flash<'static>>,
}

impl Board {
    pub fn open() -> Self {
        let clk_config = embassy_rp::clocks::ClockConfig::system_freq(200_000_000).expect("couldn't get 200 MHz");
        let cfg = embassy_rp::config::Config::new(clk_config);
        let p = embassy_rp::init(cfg);

        let access_port = AccessPort {
            swd_token: mutex::Mutex::new(()),
            swo_token: mutex::Mutex::new(()),
            pio_swd_jtag: p.PIO0,
            swo: p.UART0,
            irqs: Irqs,
            swclk_tck: p.PIN_8,
            swdio_tms: p.PIN_12,
            swo_tdo: p.PIN_13,
            tdi: p.PIN_16,
            swdio_tms_dir: p.PIN_9,
            resetn: Some(p.PIN_4),
            tresetn: Some(p.PIN_3),
            swo_buffer: core::cell::UnsafeCell::new([0u8; 256]),
        };

        // setup the unchanging pin directions;
        let swo_tdo_dir = embassy_rp::gpio::Output::new(p.PIN_5, false.into());
        core::mem::forget(swo_tdo_dir);
        let swclk_tck_dir = embassy_rp::gpio::Output::new(p.PIN_6, true.into());
        core::mem::forget(swclk_tck_dir);
        let tdi_dir = embassy_rp::gpio::Output::new(p.PIN_15, true.into());
        core::mem::forget(tdi_dir);

        let uart = AlmostUart {
            base: p.UART1,
            tx: p.PIN_24,
            rx: p.PIN_21,
        };

        // read some from the eeprom
        let mut nvm = eeprom::M24C64::new(0, p.I2C0, p.PIN_1, p.PIN_0, Irqs);
        unsafe { embassy_futures::block_on(nvm.read(0, &mut DEVICE_INFO)).expect("failed to read from eeprom") };

        // get the flash
        let flash = Some(Flash::new(p.FLASH, p.DMA_CH1, Irqs));

        Self {
            usb: Some(p.USB),
            access_port_periphs: Some(access_port),
            target_power: Some((p.PWM_SLICE5, p.PIN_26, p.PIN_27, p.PIN_25, p.PIN_28)),
            led_pins: Some((p.PWM_SLICE1, p.PWM_SLICE2, p.PIN_18, p.PIN_19, p.PIN_20)),
            adc: Some((p.ADC, p.DMA_CH0, p.PIN_29)),
            gnd_detect_pin: Some(p.PIN_2),
            key5v: Some(p.PIN_17),
            uart: Some(uart),
            i2c: Some((p.I2C1, p.PIN_22, p.PIN_23)),
            core1: Some(p.CORE1),
            eeprom: nvm,
            flash,
        }
    }

    pub fn get_device_info(&mut self) -> Result<device_info::DeviceInfo<'static>, device_info::Error> {
        unsafe { device_info::DeviceInfo::try_parse(&DEVICE_INFO) }
    }

    pub fn take_usb<'a>(&mut self) -> USBDriver<'a> {
        let usb = self.usb.take().expect("usb already taken");
        embassy_rp::usb::Driver::new(usb, Irqs)
    }

    pub fn take_access_port<'a>(&mut self) -> AccessPort<'a> {
        self.access_port_periphs.take().expect("access port already taken")
    }

    pub fn take_target_power<'a>(&mut self) -> pwm_vreg::PWMVoltageRegulator<'a> {
        let (pwm, trans_pwm, t_pwm, trans_en, t_en) = self.target_power.take().expect("target power already taken");
        let config = pwm_vreg::Config {
            max_mv: 3300,
            min_mv: 1800,
            max_mv_count: 340,
            min_mv_count: 3257,
        };
        pwm_vreg::PWMVoltageRegulator::new_ab(pwm, trans_pwm, t_pwm, Some(trans_en.into()), Some(t_en.into()), config.clone(), config)
    }

    pub fn take_led<'a>(&mut self) -> LED<'a> {
        let (pwm1, pwm2, red, green, blue) = self.led_pins.take().expect("led already taken");
        let mut config = pwm::Config::default();
        config.invert_a = true;
        config.invert_b = true;
        let red_green = pwm::Pwm::new_output_ab(pwm1, red, green, config.clone());
        let blue = pwm::Pwm::new_output_a(pwm2, blue, config.clone());
        let (maybe_red, maybe_green) = red_green.split();
        let (maybe_blue, _) = blue.split();
        LED {
            red: maybe_red.expect("couldn't unwrap red led pwm"),
            green: maybe_green.expect("couldn't unwrap green led pwm"),
            blue: maybe_blue.expect("couldn't unwrap blue led pwm"),
        }
    }

    pub fn take_adc<'a>(&mut self) -> ADC<'a> {
        let (adc, dma_channel, adc_pin) = self.adc.take().expect("adc already taken");
        let config = embassy_rp::adc::Config::default();
        ADC {
            adc: embassy_rp::adc::Adc::new(adc, Irqs, config),
            adc_dma: dma::Channel::new(dma_channel, Irqs),
            tvcc_channel: embassy_rp::adc::Channel::new_pin(adc_pin, embassy_rp::gpio::Pull::None),
        }
    }

    pub fn take_ground_detect<'a>(&mut self) -> gpio::Input<'a> {
        let gnd_detect = self.gnd_detect_pin.take().expect("ground detect already taken");
        gpio::Input::new(gnd_detect, gpio::Pull::Up)
    }

    pub fn take_key5v<'a>(&mut self) -> gpio::Output<'a> {
        let pin = self.key5v.take().expect("5 V key pin already taken");
        gpio::Output::new(pin, false.into())
    }

    pub fn take_uart(&mut self) -> AlmostUart {
        self.uart.take().expect("uart already taken")
    }

    pub fn take_i2c(&mut self) -> usb_i2c::rpi::I2CDevice<'static, periph::I2C1> {
        let (dev, sda, scl) = self.i2c.take().expect("i2c already taken");
        usb_i2c::rpi::I2CDevice::new(dev, scl, sda, Irqs, i2c::Config::default())
    }

    pub fn take_core1(&mut self) -> Peri<'static, periph::CORE1> {
        self.core1.take().expect("core1 already taken")
    }

    pub fn take_storage(&mut self) -> Storage<'static> {
        let storage = self.flash.take().expect("flash already taken");
        Storage::new(storage, get_storage_range())
    }
}

fn get_storage_range() -> core::ops::Range<u32> {
    extern "C" {
        static __partition_storage_start: u32;
        static __partition_storage_end: u32;
    }

    unsafe {
        let start = &__partition_storage_start as *const u32 as u32;
        let end = &__partition_storage_end as *const u32 as u32;
        start..end
    }
}