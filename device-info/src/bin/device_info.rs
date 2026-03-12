// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{self, Write, Read};

fn generate(device_id: &str) -> Vec<u8> {
    let mut buf = vec![0u8; 4096];
    let mut builder = device_info::DeviceInfoBuilder::try_alloc(buf.as_mut_slice()).
        expect("should be able to allocate into the buffer");
    builder.add_device_id(device_id);
    let len = builder.commit();
    buf[..len].to_vec()
}

fn main() {
    let command = std::env::args().nth(1).expect("usage: device_info [gen|parse]");
    match command.as_str() {
        "gen" => {
            let device_id = std::env::args().nth(2).expect("usage: device_info gen <id>");
            let device_info = generate(device_id.as_str());
            io::stdout().write_all(&device_info).expect("couldn't write to stdout");
        }
        "parse" => {
            let mut buf = Vec::new();
            let len = io::stdin().read_to_end(&mut buf).expect("couldn't read stdin");
            let info = device_info::DeviceInfo::try_parse(&buf[..len]).expect("invalid device info");
            for field in info.get_fields() {
                match field {
                    device_info::DeviceInfoField::DeviceId(device_id) => {
                        println!("device id: {}", device_id.try_get().unwrap());
                    }
                }
            }
        }
        _ => ()
    }
}