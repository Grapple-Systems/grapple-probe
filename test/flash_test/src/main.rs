// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::flash::{Async, ERASE_SIZE, FLASH_BASE};
use embassy_rp::peripherals::FLASH;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

const ADDR_OFFSET: u32 = 0x80000;
const FLASH_SIZE: usize = 1 * 1024 * 1024;
const PAGE_SIZE: usize = 256;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Hello World!");

    info!("{}", embassy_rp::rom_data::copyright_string());
    info!("Rom version number: {}", embassy_rp::rom_data::rom_version_number());
    info!("Rom git rev: {}", embassy_rp::rom_data::git_revision());

    // add some delay to give an attached debug probe time to parse the
    // defmt RTT header. Reading that header might touch flash memory, which
    // interferes with flash write operations.
    // https://github.com/knurling-rs/defmt/pull/683
    Timer::after_millis(10).await;

    let mut flash = embassy_rp::flash::Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);

    // Get JEDEC id
    let jedec = flash.blocking_jedec_id().unwrap();
    info!("jedec id: 0x{:x}", jedec);

    // Get unique id
    let mut uid = [0; 8];
    flash.blocking_unique_id(&mut uid).unwrap();
    info!("unique id: {:?}", uid);

    random_read_pages(&mut flash)
    //sequential_read_flash(&mut flash)
}

fn random_read_pages(flash: &mut embassy_rp::flash::Flash<'_, FLASH, Async, FLASH_SIZE>) -> ! {
    let mut rng = embassy_rp::clocks::RoscRng {};
    let mut buf = [0u8; 128*1024];

    loop {
        let start = embassy_time::Instant::now();
        for i in (0..buf.len()).step_by(PAGE_SIZE) {
            let addr = (rng.next_u32() % (FLASH_SIZE / PAGE_SIZE) as u32) * PAGE_SIZE as u32;
            // info!("{} <- {}", i, addr);
            flash.blocking_read(addr, &mut buf[i..i + PAGE_SIZE]).expect("failed to read page");
        }
        let duration = embassy_time::Instant::now() - start;

        info!("read 128 kb in {} us, sum: {}", duration.as_micros(), buf.iter().sum::<u8>());
    }
}

fn sequential_read_flash(flash: &mut embassy_rp::flash::Flash<'_, FLASH, Async, FLASH_SIZE>) -> ! {
    let mut buf = [0u8; 128*1024];
    
    loop {
        let start = embassy_time::Instant::now();
        flash.blocking_read(0, &mut buf).expect("failed to read block");
        let duration = embassy_time::Instant::now() - start;

        info!("read 128 kb in {} us, sum: {}", duration.as_micros(), buf.iter().sum::<u8>());
    }
}
