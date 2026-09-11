// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

// Created to replicate https://github.com/Grapple-Systems/grapple-probe/issues/30

#![no_std]
#![no_main]

use grapple_probe::PioAccessPort;
use cmsis_dap::SWDAccessPort;
use cmsis_dap::SWJAccessPort;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut board = grapple_probe::Board::open();

    let mut wd = board.take_watchdog();
    
    let ap_raw = board.take_access_port();
    let mut ap = PioAccessPort::new(&ap_raw);
    ap.set_frequency(100_000);
    assert!(ap.open());

    let mut data = [0u8; 8];
    assert!(ap.read(64, &mut data));
    assert_eq!(embassy_rp::pac::PIO0.flevel().read().rx0(), 0);

    defmt::info!("done!");

    loop {
        wd.feed(embassy_time::Duration::from_millis(100));
        embassy_time::Timer::after_millis(50).await;
    }
}