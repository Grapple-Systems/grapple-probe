// Copyright (c) 2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{field, proto};
use futures::io::{AsyncReadExt, AsyncWriteExt};

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
    pub async fn all() -> impl Iterator<Item = GrappleProbe> {
        nusb::list_devices()
            .await
            .expect("couldn't list usb devices")
            .filter(|dev| dev.product_string().is_some_and(|prod_str| prod_str.starts_with("Grapple Probe")))
            .map(|dev| GrappleProbe { dev })
    }

    /// Enumerate all the Grapple Probes (blocking).
    pub fn all_blocking() -> impl Iterator<Item = GrappleProbe> {
        smol::block_on(Self::all())
    }

    /// Get the serial number.
    pub fn get_serial_number(&self) -> &str {
        self.dev.serial_number().expect("device doesn't have serial number")
    }

    /// Open the cmsis dap interface.  When this is open the probe can't be used as a debugger.
    pub async fn open_cmsis_dap(&self) -> Result<GrappleProbeSession, Error> {
        let if_info = self.dev.interfaces().find(|iface| iface.interface_string().is_some_and(|if_str| if_str.starts_with("CMSIS-DAP")));
        if let Some(if_info) = if_info {
            let dev = self.dev.open().await.map_err(Error::from)?;
            let iface = dev.claim_interface(if_info.interface_number()).await.map_err(Error::from)?;
            let mut eps = iface.descriptor().ok_or(Error::DeviceDoesntHaveCMSIS)?.endpoints();
            let tx_addr = eps.next().ok_or(Error::DeviceDoesntHaveCMSIS)?.address();
            let rx_addr = eps.next().ok_or(Error::DeviceDoesntHaveCMSIS)?.address();
            let tx = iface.endpoint::<nusb::transfer::Bulk, nusb::transfer::Out>(tx_addr).unwrap().writer(64);
            let mut rx = iface.endpoint::<nusb::transfer::Bulk, nusb::transfer::In>(rx_addr).unwrap().reader(64);
			rx.set_read_timeout(std::time::Duration::from_secs(1));

            Ok(GrappleProbeSession { rx, tx, power_config: None })
        } else {
            Err(Error::DeviceDoesntHaveCMSIS)
        }
    }

    /// Open the cmsis dap interface (blocking).  When this is open the probe can't be used as a debugger.
    pub fn open_cmsis_dap_blocking(&self) -> Result<GrappleProbeSession, Error> {
        smol::block_on(self.open_cmsis_dap())
    }
}

impl GrappleProbeSession {
    /// Get the firmware version.
    pub async fn get_firmware_version(&mut self) -> Result<String, Error> {
        let req = [0x00u8, 0x09];

        self.tx.write_all(&req).await.or(Err(Error::USBError))?;
        self.tx.flush().await.or(Err(Error::USBError))?;
        let mut buf = [0u8; 64];
        let read_size = self.rx.read(&mut buf).await.or(Err(Error::Timeout))?;
        if read_size >= 2 && buf[0] == 0 {
            let fw_ver_len = (buf[1] as usize).min(read_size - 2);
            String::from_utf8(buf[2..2+fw_ver_len].to_vec()).or(Err(Error::ValueError))
        } else {
            Err(Error::Timeout)
        }
    }

    /// Get the firmware version (blocking).
    pub fn get_firmware_version_blocking(&mut self) -> Result<String, Error> {
        smol::block_on(self.get_firmware_version())
    }

    /// Get the current status.
    pub async fn get_status(&mut self) -> Result<Status, Error> {
        let mut buf = [0u8; 64];
        let len = proto::GetStatusRequest::try_new(&mut buf).expect("buffer not big enough").commit();
        let res = self.request(&mut buf, len).await?;
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

    /// Get the current status (blocking).
    pub fn get_status_blocking(&mut self) -> Result<Status, Error> {
        smol::block_on(self.get_status())
    }

    /// Get the power configuration.  This lazy loads from the device.
    pub async fn get_power_config(&mut self) -> Result<&PowerControl, Error> {
        use field::Field;
        if self.power_config.is_none() {
            if let Ok(data) = self.read_field(field::OwnedPowerControl::ID).await {
                if let Ok(control) = field::PowerControl::try_parse(data) {
                    self.power_config = Some(PowerControl::from_field(&control));
                } else {
                    self.power_config = Some(PowerControl::from_field(&field::OwnedPowerControl::default()));
                }
            } else {
                self.power_config = Some(PowerControl::from_field(&field::OwnedPowerControl::default()));
            }
        }
        Ok(self.power_config.as_ref().expect("power_config not loaded"))
    }

    /// Get the power configuration (blocking).  This lazy loads from the device.
    pub fn get_power_config_blocking(&mut self) -> Result<&PowerControl, Error> {
        smol::block_on(self.get_power_config())
    }

    /// Modify the power control parameters from the probe.
    pub async fn modify_power_config<F: Fn(&mut PowerControl) -> bool>(&mut self, func: F) -> Result<(), Error> {
        use field::Field;
        let mut power_control = self.get_power_config().await?.clone();
        if func(&mut power_control) {
            let mut power_control_field = field::OwnedPowerControl::default();
            power_control.fill_field(&mut power_control_field);
            self.write_field(field::OwnedPowerControl::ID, power_control_field.data()).await?;
            _ = self.power_config.replace(power_control);
        }
        Ok(())
    }

    /// Modify the power control parameters from the probe (blocking).
    pub fn modify_power_config_blocking<F: Fn(&mut PowerControl) -> bool>(&mut self, func: F) -> Result<(), Error> {
        smol::block_on(self.modify_power_config(func))
    }

    /// Read a raw configuration field.
    pub async fn read_field(&mut self, field: u8) -> Result<Vec<u8>, Error> {
        let mut buf = [0u8; 64];
        let mut req = proto::ReadFieldRequest::try_new(&mut buf).expect("buffer not big enough");
        req.set_field_id(field);
        let len = req.commit();

        let res = self.request(&mut buf, len).await?;
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

    /// Read a raw configuration field (blocking).
    pub fn read_field_blocking(&mut self, field: u8) -> Result<Vec<u8>, Error> {
        smol::block_on(self.read_field(field))
    }

    /// Write a raw configuration field.
    pub async fn write_field(&mut self, id: u8, data: &[u8]) -> Result<(), Error> {
        let mut buf = [0u8; 64];
        let mut req = proto::WriteFieldRequest::try_new(&mut buf).expect("buffer not big enough");
        req.set_field_id(id);
        if data.len() <= req.get_data_mut().len() {
            req.get_data_mut()[..data.len()].copy_from_slice(data);
        } else {
            return Err(Error::ConfigValueError);
        }
        let len = req.commit(data.len());

        let res = self.request(&mut buf, len).await?;
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

    /// Write a raw configuration field (blocking).
    pub fn write_field_blocking(&mut self, id: u8, data: &[u8]) -> Result<(), Error> {
        smol::block_on(self.write_field(id, data))
    }

	/// Reset the target.
    pub async fn reset(&mut self, reset_type: ResetType) -> Result<(), Error> {
        let mut buf = [0u8; 64];
        let mut req = proto::ResetRequest::try_new(&mut buf).expect("buffer not big enough");
        req.set_reset_type(reset_type as u8);
        let len = req.commit();

        self.request(&mut buf, len).await.and(Ok(()))
    }

    /// Reset the target (blocking).
    pub fn reset_blocking(&mut self, reset_type: ResetType) -> Result<(), Error> {
        smol::block_on(self.reset(reset_type))
    }

    async fn request<'a>(&mut self, buf: &'a mut [u8], req_len: usize) -> Result<proto::Packet<&'a [u8]>, Error> {
        self.tx.write_all(&buf[..req_len]).await.or(Err(Error::USBError))?;
        self.tx.flush().await.or(Err(Error::USBError))?;
        let read_size = self.rx.read(buf).await.or(Err(Error::Timeout))?;
        proto::Packet::try_parse_response(&buf[..read_size]).or(Err(Error::Timeout))
    }
}

pub struct GrappleProbeSession {
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
