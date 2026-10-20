// Copyright (c) 2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;

pub trait Field {
    const ID: u8;
}

pub struct PowerControl<B> {
    inner: B,
}
pub type OwnedPowerControl = PowerControl<[u8;6]>;

impl<B> Field for PowerControl<B> {
    const ID: u8 = 0;
}

impl<B: Clone> Clone for PowerControl<B> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl<B> PowerControl<B> {
    pub const fn default() -> OwnedPowerControl {
        // transvcc default 3.3 V and following tvcc
        // tvcc is an input
        // 5v key is enabled
        let inner = [0x00, 0b11000000, 0xE4, 0x0C, 0x00, 0x00];
        PowerControl { inner }
    }
}

impl<B: AsRef<[u8]>> PowerControl<B> {
    pub fn try_parse(inner: B) -> Result<Self, Error> {
        if inner.as_ref().len() >= 6 && inner.as_ref()[0] == 0 {
            Ok(Self { inner })
        } else {
            Err(Error::InvalidPacket)
        }
    }

    pub fn as_owned(&self) -> OwnedPowerControl {
        let mut inner = [0u8;6];
        inner.copy_from_slice(&self.inner.as_ref()[..6]);
        PowerControl { inner }
    }

    pub fn data(&self) -> &[u8] {
        &self.inner.as_ref()[..6]
    }

    pub fn key5v_enable(&self) -> bool {
        self.get_flag(PowerControlFlagMask::Key5VEnable)
    }

    pub fn transvcc_follows_tvcc(&self) -> bool {
        self.get_flag(PowerControlFlagMask::SignalFollowTVCC)
    }

    pub fn transvcc_gated_by_gnddet(&self) -> bool {
        self.get_flag(PowerControlFlagMask::SignalGatedGndDet)
    }

    pub fn transvcc_gated_by_tvcc(&self) -> bool {
        self.get_flag(PowerControlFlagMask::SignalGatedTVCC)
    }

    pub fn tvcc_output(&self) -> bool {
        self.get_flag(PowerControlFlagMask::TVCCOutput)
    }

    pub fn tvcc_gated_by_gnddet(&self) -> bool {
        self.get_flag(PowerControlFlagMask::TVCCGatedGndDet)
    }

    pub fn transvcc_default_mv(&self) -> u16 {
        u16::from_le_bytes(self.inner.as_ref()[2..4].try_into().unwrap())
    }

    pub fn tvcc_output_mv(&self) -> u16 {
        u16::from_le_bytes(self.inner.as_ref()[4..6].try_into().unwrap())
    }

    fn get_flag(&self, mask: PowerControlFlagMask) -> bool {
        (self.inner.as_ref()[1] & mask as u8) != 0
    }
}

impl<B: AsMut<[u8]>> PowerControl<B> {
    pub fn try_alloc(mut inner: B) -> Result<Self, Error> {
        if inner.as_mut().len() >= 6 {
            Ok(Self { inner })
        } else {
            Err(Error::NotEnoughBytes)
        }
    }

    pub fn commit(mut self) -> usize {
        self.inner.as_mut()[0] = 0;
        6
    }

    pub fn clone_from<O: AsRef<[u8]>>(&mut self, other: &PowerControl<O>) {
        let len = self.inner.as_mut().len().min(other.inner.as_ref().len());
        self.inner.as_mut()[..len].copy_from_slice(&other.inner.as_ref()[..len]);
    }

    pub fn set_key5v_enable(&mut self, value: bool) {
        self.set_flag(PowerControlFlagMask::Key5VEnable, value);
    }

    pub fn set_transvcc_follows_tvcc(&mut self, value: bool) {
        self.set_flag(PowerControlFlagMask::SignalFollowTVCC, value);
    }

    pub fn set_transvcc_gated_by_gnddet(&mut self, value: bool) {
        self.set_flag(PowerControlFlagMask::SignalGatedGndDet, value);
    }
    
    pub fn set_transvcc_gated_by_tvcc(&mut self, value: bool) {
        self.set_flag(PowerControlFlagMask::SignalGatedTVCC, value);
    }

    pub fn set_tvcc_output(&mut self, value: bool) {
        self.set_flag(PowerControlFlagMask::TVCCOutput, value);
    }

    pub fn set_tvcc_gated_by_gnddet(&mut self, value: bool) {
        self.set_flag(PowerControlFlagMask::TVCCGatedGndDet, value);
    }

    pub fn set_transvcc_default_mv(&mut self, value: u16) {
        self.inner.as_mut()[2..4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn set_tvcc_output_mv(&mut self, value: u16) {
        self.inner.as_mut()[4..6].copy_from_slice(&value.to_le_bytes());
    }

    fn set_flag(&mut self, mask: PowerControlFlagMask, value: bool) {
        let value = if value { mask as u8 } else { 0 };
        self.inner.as_mut()[1] = self.inner.as_mut()[1] & !(mask as u8) | value;
    }
}

#[repr(u8)]
#[derive(Clone, Copy)]
enum PowerControlFlagMask {
    Key5VEnable = 0x80,
    SignalFollowTVCC = 0x40,
    SignalGatedGndDet = 0x20,
    SignalGatedTVCC = 0x10,
    TVCCOutput = 0x08,
    TVCCGatedGndDet = 0x04,
}