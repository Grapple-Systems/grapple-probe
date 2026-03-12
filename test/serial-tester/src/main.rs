// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use rand::RngCore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::SerialPortBuilderExt;

/// Test a serial port by writing blocks of data to it and validate them on another connected serial port.
#[derive(Parser)]
struct Args {
    /// path of tty to write blocks to
    tty_in: std::path::PathBuf,
    /// path of tty to read blocks from
    tty_out: std::path::PathBuf,
    /// baud rate to use when writing blocks, in bits per second.
    baud_rate: u32,
    /// size of blocks to write and read from.
    block_size: usize,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let mut rng = rand::thread_rng();
    
    let mut port1 = tokio_serial::new(args.tty_in.to_str().unwrap(), args.baud_rate).
        open_native_async().expect("failed to open tty1");
    let mut port2 = tokio_serial::new(args.tty_out.to_str().unwrap(), args.baud_rate).
        open_native_async().expect("failed to open tty2");

    // dump all data from a previous session
    dumpall_timeout(&mut port1).await;
    dumpall_timeout(&mut port2).await;

    let (mut p1_read, mut p1_write) = tokio::io::split(port1);
    let (mut p2_read, mut p2_write) = tokio::io::split(port2);

    let mut port1_send = vec![0u8; args.block_size];
    let mut port1_recv = vec![0u8; args.block_size];
    let mut port2_send = vec![0u8; args.block_size];
    let mut port2_recv = vec![0u8; args.block_size];

    let mut total_bytes = 0usize;
    let start_time = std::time::Instant::now();

    loop {
        rng.fill_bytes(&mut port1_send);
        rng.fill_bytes(&mut port2_send);

        let serial_activity = async {
            let p1_write = p1_write.write_all(&port1_send);
            let p2_write = p2_write.write_all(&port2_send);
            let p1_read = p1_read.read_exact(&mut port1_recv);
            let p2_read = p2_read.read_exact(&mut port2_recv);
            tokio::join!(p1_write, p2_write, p1_read, p2_read)
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            (p1w_res, p2w_res, p1r_res, p2r_res) = serial_activity => {
                p1w_res.expect("failed to write to port 1");
                p2w_res.expect("failed to write to port 2");
                p1r_res.expect("failed to read from port 1");
                p2r_res.expect("failed to read from port 2");
            }
        }

        assert_eq!(port1_send, port2_recv);
        assert_eq!(port2_send, port1_recv);
        total_bytes += args.block_size;
    }

    let elapsed_time = start_time.elapsed();
    let measured_baud = ((total_bytes as f64 / elapsed_time.as_secs_f64()) * 8.0) as usize;
    println!("Send {} bytes over {} seconds -> {} bps", total_bytes, elapsed_time.as_secs(), measured_baud);
}

async fn dumpall_timeout(serial: &mut tokio_serial::SerialStream) {
    let mut dumper = [0u8; 4096];
    while tokio::time::timeout(std::time::Duration::from_millis(100), serial.read(&mut dumper)).await.is_ok() {}
}