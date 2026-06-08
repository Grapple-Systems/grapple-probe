// Copyright (c) 2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A library for interfacing with Grapple Probes.
//! 
//! ``` rust
//! // Print out the serial number, measured target voltage, and commanded signal voltage for each detected Grapple Probe.
//! for probe in GrappleProbe.all() {
//!     println!("grapple probe: {}", probe.get_serial_number());
//! 
//!     let mut dap = probe.open_cmsis_dap().unwrap();
//!     let status = dap.get_status().unwrap();
//!     println!("\ttvcc: {} mv, signal: {} mv", status.target_mv, status.signal_mv);
//! }
//! ```

use grapple_probe_proto::{field, proto};
use nusb::MaybeFuture;

pub use proto::ResetType;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    PermissionDenied,
    DeviceDoesntHaveCMSIS,
    ConfigValueError,
    USBError,
    Timeout,
    ValueError,
}

impl From<nusb::Error> for Error {
    fn from(value: nusb::Error) -> Self {
        match value.kind() {
            nusb::ErrorKind::PermissionDenied => Error::PermissionDenied,
            _ => Error::USBError,
        }
    }
}

pub struct GrappleProbe {
    dev: nusb::DeviceInfo,
}

impl GrappleProbe {
    /// Enumerate all the Grapple Probes.
    pub fn all() -> impl Iterator<Item = GrappleProbe> {
        nusb::list_devices().wait().expect("couldn't list usb devices").
            filter(|dev| dev.product_string().is_some_and(|prod_str| prod_str.starts_with("Grapple Probe"))).
            map(|dev| GrappleProbe { dev })
    }

    /// Get the serial number.
    pub fn get_serial_number(&self) -> &str {
        self.dev.serial_number().expect("device doesn't have serial number")
    }

    /// Open the cmsis dap interface.  When this is open the probe can't be used as a debugger.
    pub fn open_cmsis_dap(&self) -> Result<GrappleProbeDebug, Error> {
        let if_info = self.dev.interfaces().find(|iface| iface.interface_string().is_some_and(|if_str| if_str.starts_with("CMSIS-DAP")));
        if let Some(if_info) = if_info {
            let dev = self.dev.open().wait().map_err(|err| Error::from(err))?;
            let iface = dev.claim_interface(if_info.interface_number()).wait().map_err(|err| Error::from(err))?;
            let mut eps = iface.descriptor().ok_or(Error::DeviceDoesntHaveCMSIS)?.endpoints();
            let tx_addr = eps.next().ok_or(Error::DeviceDoesntHaveCMSIS)?.address();
            let rx_addr = eps.next().ok_or(Error::DeviceDoesntHaveCMSIS)?.address();
            let tx = iface.endpoint::<nusb::transfer::Bulk, nusb::transfer::Out>(tx_addr).unwrap().writer(64);
            let mut rx = iface.endpoint::<nusb::transfer::Bulk, nusb::transfer::In>(rx_addr).unwrap().reader(64);
            rx.set_read_timeout(std::time::Duration::from_secs(1));

            Ok(GrappleProbeDebug { rx, tx, power_config: None })
        } else {
            Err(Error::DeviceDoesntHaveCMSIS)
        }
    }
}

impl GrappleProbeDebug {
    /// Get the firmware version.
    pub fn get_firmware_version(&mut self) -> Result<String, Error> {
        use std::io::{Read, Write};
        let req = [0x00u8, 0x09];

        self.tx.write_all(&req).or(Err(Error::USBError))?;
        self.tx.flush().or(Err(Error::USBError))?;
        let mut buf = [0u8; 64];
        let read_size = self.rx.read(&mut buf).or(Err(Error::Timeout))?;
        if read_size >= 2 && buf[0] == 0 {
            let fw_ver_len = (buf[1] as usize).min(read_size - 2);
            String::from_utf8(buf[2..2+fw_ver_len].to_vec()).map_err(|_| Error::ValueError)
        } else {
            Err(Error::Timeout)
        }
    }

    /// Get the current status.
    pub fn get_status(&mut self) -> Result<Status, Error> {
        let mut buf = [0u8; 64];
        let len = proto::GetStatusRequest::try_alloc(&mut buf).expect("buffer not big enough").commit();
        let res = self.request(&mut buf, len)?;
        if let proto::Packet::GetStatusResponse(res) = res {
            let flags = proto::StatusFlags { inner: res.get_flags() };
            Ok(Status {
                gnddet: flags.gnd_detect(),
                target_mv: res.get_target_voltage(),
                signal_mv: res.get_signal_voltage(),
            })
        } else {
            Err(Error::Timeout)
        }
    }

    /// Get the power configuration.  This lazy loads from the device.
    pub fn get_power_config(&mut self) -> Result<&PowerControl, Error> {
        use field::Field;
        if self.power_config.is_none() {
            if let Ok(data) = self.read_field(field::OwnedPowerControl::ID) {
                if let Ok(control) = field::PowerControl::try_parse(data) {
                    self.power_config = Some(PowerControl::from_field(&control))
                } else {
                    self.power_config = Some(PowerControl::from_field(&field::OwnedPowerControl::default()));
                }
            } else {
                self.power_config = Some(PowerControl::from_field(&field::OwnedPowerControl::default()));
            }
        }
        Ok(self.power_config.as_ref().expect("power_config not loaded"))
    }

    /// Modify the power control parameters from the 
    pub fn modify_power_config<F: Fn(&mut PowerControl) -> bool>(&mut self, func: F) -> Result<(), Error> {
        use field::Field;
        let mut power_control = self.get_power_config()?.clone();
        if func(&mut power_control) {
            let mut power_control_field = field::OwnedPowerControl::default();
            power_control.fill_field(&mut power_control_field);
            self.write_field(field::OwnedPowerControl::ID, power_control_field.data())?;
            _ = self.power_config.replace(power_control);
        }
        Ok(())
    }

    /// Read a raw configuration field.
    pub fn read_field(&mut self, field: u8) -> Result<Vec<u8>, Error> {
        let mut buf = [0u8; 64];
        let mut req = proto::ReadFieldRequest::try_alloc(&mut buf).expect("buffer not big enough");
        req.set_field_id(field);
        let len = req.commit();

        let res = self.request(&mut buf, len)?;
        if let proto::Packet::ReadFieldResponse(res) = res {
            if res.get_status() == 0 && res.get_field_id() == field {
                Ok(res.get_data().to_vec())
            } else {
                Err(Error::Timeout)
            }
        } else {
            Err(Error::Timeout)
        }
    }

    /// Write a raw configuration field.
    pub fn write_field(&mut self, id: u8, data: &[u8]) -> Result<(), Error> {
        let mut buf = [0u8; 64];
        let mut req = proto::WriteFieldRequest::try_alloc(&mut buf).expect("buffer not big enough");
        req.set_field_id(id);
        if data.len() <= req.get_data_mut().len() {
            req.get_data_mut()[..data.len()].copy_from_slice(data);
        } else {
            return Err(Error::ConfigValueError);
        }
        let len = req.commit(data.len());

        let res = self.request(&mut buf, len)?;
        if let proto::Packet::WriteFieldResponse(res) = res {
            if res.get_status() == 0 && res.get_field_id() == id {
                Ok(())
            } else {
                Err(Error::ConfigValueError)
            }
        } else {
            Err(Error::Timeout)
        }
    }

    pub fn reset(&mut self, reset_type: ResetType) -> Result<(), Error> {
        let mut buf = [0u8; 64];
        let mut req = proto::ResetRequest::try_alloc(&mut buf).expect("buffer not big enough");
        req.set_reset_type(reset_type as u8);
        let len = req.commit();

        self.request(&mut buf, len).and(Ok(()))
    }

    fn request<'a>(&mut self, buf: &'a mut [u8], req_len: usize) -> Result<proto::Packet<&'a [u8]>, Error> {
        use std::io::{Read, Write};

        self.tx.write_all(&buf[..req_len]).or(Err(Error::USBError))?;
        self.tx.flush().or(Err(Error::USBError))?;
        let read_size = self.rx.read(buf).or(Err(Error::Timeout))?;
        proto::Packet::try_parse_response(&buf[..read_size]).or(Err(Error::Timeout))
    }
}

pub struct GrappleProbeDebug {
    rx: nusb::io::EndpointRead<nusb::transfer::Bulk>,
    tx: nusb::io::EndpointWrite<nusb::transfer::Bulk>,
    power_config: Option<PowerControl>,
}

pub struct Status {
    /// True if ground is detected.
    pub gnddet: bool,
    /// Measured tvcc in millivolts.
    pub target_mv: u16,
    /// Commanded signal level in millivolts.
    pub signal_mv: u16,
}

#[derive(Clone)]
pub struct PowerControl {
    /// True to enable the 5V Key power output.
    pub key5v_enable: bool,
    /// True if the signal voltage follows the measured tvcc.
    pub signal_follows_tvcc: bool,
    /// True if the signal voltage is gated by the ground detect signal.
    pub signal_gated_by_gnddet: bool,
    /// True if the signal voltage is gated by tvcc >= 1.6 V.
    pub signal_gated_by_tvcc: bool,
    /// Millivolts of signals when not gated and not following tvcc or tvcc is < 1.6 V.
    pub signal_default_mv: u16,
    /// True if tvcc is generated by the Grapple Probe.
    pub tvcc_output: bool,
    /// True if tvcc output is gated by the ground detect signal.
    pub tvcc_gated_by_gnddet: bool,
    /// Millivolts output on tvcc when output enabled (tvcc_output).
    pub tvcc_output_mv: u16,
}

impl PowerControl {
    pub fn default() -> Self {
        Self::from_field(&field::OwnedPowerControl::default())
    }

    fn from_field<B: AsRef<[u8]>>(f: &field::PowerControl<B>) -> Self {
        Self {
            key5v_enable: f.key5v_enable(),
            signal_follows_tvcc: f.transvcc_follows_tvcc(),
            signal_gated_by_gnddet: f.transvcc_gated_by_gnddet(),
            signal_gated_by_tvcc: f.transvcc_gated_by_tvcc(),
            signal_default_mv: f.transvcc_default_mv(),
            tvcc_output: f.tvcc_output(),
            tvcc_gated_by_gnddet: f.tvcc_gated_by_gnddet(),
            tvcc_output_mv: f.tvcc_output_mv(),
        }
    }

    fn fill_field<B: AsMut<[u8]>>(&self, f: &mut field::PowerControl<B>) {
        f.set_key5v_enable(self.key5v_enable);
        f.set_transvcc_follows_tvcc(self.signal_follows_tvcc);
        f.set_transvcc_gated_by_gnddet(self.signal_gated_by_gnddet);
        f.set_transvcc_gated_by_tvcc(self.signal_gated_by_tvcc);
        f.set_transvcc_default_mv(self.signal_default_mv);
        f.set_tvcc_output(self.tvcc_output);
        f.set_tvcc_gated_by_gnddet(self.tvcc_gated_by_gnddet);
        f.set_tvcc_output_mv(self.tvcc_output_mv);
    }
}