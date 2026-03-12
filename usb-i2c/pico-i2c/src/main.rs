// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

#![no_std]
#![no_main]

use {defmt_rtt as _, panic_probe as _};

use embassy_rp::{i2c, usb, peripherals as pers};

embassy_rp::bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<pers::USB>;
    I2C0_IRQ => i2c::InterruptHandler<pers::I2C0>;
});

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    defmt::info!("usb i2c for rpi-pico");

    let p = embassy_rp::init(Default::default());

    let device = usb_i2c::rpi::I2CDevice::new(p.I2C0, p.PIN_1, p.PIN_0, Irqs, Default::default());
    let mut i2c = usb_i2c::I2CClass::new();

    let usb_driver = usb::Driver::new(p.USB, Irqs);
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB raw example");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut msos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut builder = embassy_usb::Builder::new(
        usb_driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    let mut i2c_usb = i2c.build(device, &mut builder);

    let mut usb = builder.build();
    let usb_fut = usb.run();
    let i2c_fut = i2c_usb.run();

    embassy_futures::join::join(usb_fut, i2c_fut).await;
}
