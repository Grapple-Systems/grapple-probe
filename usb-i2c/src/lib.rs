// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

#![cfg_attr(not(test), no_std)]

pub mod rpi;

mod protocol;

use embedded_hal_async::i2c;

const MAX_PACKET_SIZE: usize = 64;

pub trait I2CDevice: i2c::I2c {
    fn configure(&mut self, freq_hz: u32) -> Result<(), Self::Error>;
}

pub struct I2CClass {
    control: core::mem::MaybeUninit<Control>,
}

impl I2CClass {
    pub fn new() -> Self {
        Self {
            control: core::mem::MaybeUninit::uninit(),
        }
    }

    pub fn build<'a, USB: embassy_usb::driver::Driver<'a>, D>(
        &'a mut self, device: D, builder: &mut embassy_usb::Builder<'a, USB>
    ) -> I2CUSBDevice<'a, USB, D>
    {
        let mut function = builder.function(0xFF, 0, 0);
        let mut interface = function.interface();
        let iface_string_id = interface.string();
        let mut alt = interface.alt_setting(0xFF, 0, 0, Some(iface_string_id));
        let read_ep = alt.endpoint_bulk_out(None, MAX_PACKET_SIZE as u16);
        let write_ep = alt.endpoint_bulk_in(None, MAX_PACKET_SIZE as u16);
        drop(function);

        let control = self.control.write(Control { iface_string_id });
        builder.handler(control);

        I2CUSBDevice { device, read_ep, write_ep }
    }
}

pub struct I2CUSBDevice<'a, USB: embassy_usb::driver::Driver<'a>, D> {
    device: D,
    read_ep: USB::EndpointOut,
    write_ep: USB::EndpointIn,
}

impl<'a, USB: embassy_usb::driver::Driver<'a>, D: I2CDevice> I2CUSBDevice<'a, USB, D> {
    pub async fn run(&mut self) {
        use embassy_usb::driver::{Endpoint, EndpointIn, EndpointOut};
        let mut read_buf = [0u8; MAX_PACKET_SIZE];
        let mut res_buf = [0u8; MAX_PACKET_SIZE];
        loop {
            self.read_ep.wait_enabled().await;

            while let Ok(read_size) = self.read_ep.read(&mut read_buf).await {
                let res_size = match protocol::Packet::try_parse_request(&read_buf[..read_size]) {
                    Ok(protocol::Packet::ConfigureRequest(req)) => self.handle_configure(req, &mut res_buf),
                    Ok(protocol::Packet::WriteRequest(req)) => self.handle_write(req, &mut res_buf).await,
                    Ok(protocol::Packet::ReadRequest(req)) => self.handle_read(req, &mut res_buf).await,
                    Ok(protocol::Packet::WriteReadRequest(req)) => self.handle_write_read(req, &mut res_buf).await,
                    Ok(_) => {
                        defmt::error!("i2c_usb: got a response packet");
                        protocol::GeneralError::try_alloc(&mut res_buf).expect("couldn't allocate response").commit()
                    }
                    Err(e) => {
                        defmt::warn!("i2c_usb: parsing error: {}", e);
                        protocol::GeneralError::try_alloc(&mut res_buf).expect("couldn't allocate response").commit()
                    }
                };

                let _ = self.write_ep.write(&mut res_buf[..res_size]).await;
            }
        }
    }

    fn handle_configure(&mut self, req: protocol::ConfigureRequest<&[u8]>, res_buf: &mut [u8]) -> usize {
        let mut res = req.try_alloc_response(res_buf).expect("couldn't allocate response");
        
        if self.device.configure(req.get_freq_hz()).is_ok() {
            res.set_result(0);
            res.commit()
        } else {
            res.set_result(255);
            res.commit()
        }
    }

    async fn handle_write(&mut self, req: protocol::WriteRequest<&[u8]>, res_buf: &mut [u8]) -> usize {
        let mut res = req.try_alloc_response(res_buf).expect("couldn't allocate response");

        let address = i2c_address_from_protocol(req.get_address());
        let status = match self.device.write(address, req.get_data()).await {
            Ok(()) => 0,
            Err(_) => 1,
        };
        res.set_result(status);
        res.commit()
    }

    async fn handle_read(&mut self, req: protocol::ReadRequest<&[u8]>, res_buf: &mut [u8]) -> usize {
        let mut res = req.try_alloc_response(res_buf).expect("couldn't allocate response");

        let address = i2c_address_from_protocol(req.get_address());
        let data = &mut res.get_data_mut()[..req.get_len() as usize];
        let status = match self.device.read(address, data).await {
            Ok(()) => 0,
            Err(_) => 1,
        };
        res.set_result(status);
        res.commit(req.get_len() as usize)
    }

    async fn handle_write_read(&mut self, req: protocol::WriteReadRequest<&[u8]>, res_buf: &mut [u8]) -> usize {
        let mut res = req.try_alloc_response(res_buf).expect("couldn't allocate response");

        let address = i2c_address_from_protocol(req.get_address());
        let write_data = &mut res.get_read_data_mut()[..req.get_read_len() as usize];
        let status = match self.device.write_read(address, req.get_write_data(), write_data).await {
            Ok(()) => 0,
            Err(_) => 1,
        };
        res.set_result(status);
        res.commit(req.get_read_len() as usize)
    }
}

fn i2c_address_from_protocol(proto: u16) -> i2c::SevenBitAddress {
    (proto & 0x7F) as i2c::SevenBitAddress
}

struct Control {
    iface_string_id: embassy_usb::types::StringIndex,
}

impl embassy_usb::Handler for Control {
    fn get_string(&mut self, index: embassy_usb::types::StringIndex, _lang_id: u16) -> Option<&str> {
        if index == self.iface_string_id {
            Some("Grapple I2C")
        } else {
            None
        }
    }
}
