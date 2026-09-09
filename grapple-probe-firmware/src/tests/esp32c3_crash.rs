#![no_std]
#![no_main]

use grapple_probe::PioAccessPort;
use cmsis_dap::JTAGAccessPort;
use cmsis_dap::SWJAccessPort;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut board = grapple_probe::Board::open();

    
    let ap_raw = board.take_access_port();
    let mut ap = PioAccessPort::new(&ap_raw);
    ap.set_frequency(100_000);
    assert!(ap.open());

    //[20, 12, 65, 0, 65, 0, 65, 0, 1, 0, 65, 0, 1, 0, 1, 0, 128, 255, 255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255]
    // assert!(ap.transfer(1, true, &[0], &mut []));
    // assert!(ap.transfer(1, true, &[0], &mut []));
    // assert!(ap.transfer(1, true, &[0], &mut []));
    // assert!(ap.transfer(1, false, &[0], &mut []));
    // assert!(ap.transfer(1, true, &[0], &mut []));
    // assert!(ap.transfer(1, false, &[0], &mut []));
    // assert!(ap.transfer(1, false, &[0], &mut []));
    
    let mut out_data = [0u8; 8];
    assert!(ap.transfer(64, false, &[255; 8], &mut out_data));
    assert!(ap.transfer(64, false, &[255; 8], &mut out_data));
    assert!(ap.transfer(64, false, &[255; 8], &mut out_data));
    assert!(ap.transfer(64, false, &[255; 8], &mut out_data));
    assert!(ap.transfer(64, false, &[255; 8], &mut out_data));
    defmt::info!("done!");
}