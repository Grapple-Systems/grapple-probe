// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

#![cfg_attr(not(test), no_std)]

#[derive(Debug, PartialEq)]
pub enum Error {
    NotEnoughBytes,
    InvalidChecksum,
    UnrecognizedId,
}

pub enum DeviceInfoField<'a> {
    DeviceId(DeviceId<'a>)
}

impl<'a> TryFrom<Field<'a>> for DeviceInfoField<'a> {
    type Error = Error;

    fn try_from(value: Field<'a>) -> Result<Self, Self::Error> {
        match value.get_id() {
            0 => DeviceId::try_from(value).map(|field| Self::DeviceId(field)),
            _ => Err(Error::UnrecognizedId),
        }
    }
}

pub struct DeviceInfo<'a> {
    payload_len: usize,
    inner: &'a [u8],
}

impl<'a> DeviceInfo<'a> {
    pub fn try_parse(inner: &'a [u8]) -> Result<Self, Error> {
        if inner.as_ref().len() >= 4 {
            let payload_len = u16::from_le_bytes(inner.as_ref()[2..4].try_into().unwrap()) as usize;
            if inner.as_ref().len() >= 4 + payload_len {
                let data_sum = u16::from_le_bytes(inner.as_ref()[..2].try_into().unwrap());
                let calc_sum = checksum(&inner.as_ref()[2..4+payload_len]);
                if data_sum == calc_sum {
                    Ok(Self { payload_len, inner})
                } else {
                    Err(Error::InvalidChecksum)
                }
            } else {
                Err(Error::NotEnoughBytes)
            }
        } else {
            Err(Error::NotEnoughBytes)
        }
    }

    pub fn get_payload(&self) -> &'a [u8] {
        &self.inner.as_ref()[4..4+self.payload_len]
    }

    pub fn get_fields(&self) -> FieldIter<'a> {
        FieldIter { inner: self.get_payload() }
    }
}

pub struct FieldIter<'a> {
    inner: &'a [u8]
}

impl<'a> Iterator for FieldIter<'a> {
    type Item = DeviceInfoField<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let field = Field::try_parse(self.inner).ok()?;
            self.inner = &self.inner[field.get_len()..];
            if let Ok(field) = field.try_into() {
                return Some(field)
            }
        }
    }
}

pub struct DeviceInfoBuilder<B> {
    payload_len: usize,
    inner: B,
}

impl<B: AsMut<[u8]>> DeviceInfoBuilder<B> {
    pub fn try_alloc(mut inner: B) -> Result<Self, Error> {
        if inner.as_mut().len() >= 4 {
            Ok(Self { payload_len: 0, inner })
        } else {
            Err(Error::NotEnoughBytes)
        }
    }

    pub fn add_device_id(&mut self, value: &str) -> bool {
        if self.get_remaining_bytes().len() >= 4 + value.len() {
            self.write_field_header(DeviceInfoFieldId::DeviceId, value.len());
            self.get_remaining_bytes()[4..4+value.len()].copy_from_slice(value.as_bytes());
            self.payload_len += 4 + value.len();
            true
        } else {
            false
        }
    }

    pub fn commit(mut self) -> usize {
        self.inner.as_mut()[2..4].copy_from_slice(&(self.payload_len as u16).to_le_bytes());
        let sum = checksum(&self.inner.as_mut()[2..4 + self.payload_len]);
        self.inner.as_mut()[..2].copy_from_slice(&sum.to_le_bytes());
        4 + self.payload_len
    }

    fn get_remaining_bytes(&mut self) -> &mut [u8] {
        &mut self.inner.as_mut()[4+self.payload_len..]
    }

    fn write_field_header(&mut self, id: DeviceInfoFieldId, len: usize) {
        self.get_remaining_bytes()[..2].copy_from_slice(&(id as u16).to_le_bytes());
        self.get_remaining_bytes()[2..4].copy_from_slice(&(len as u16).to_le_bytes());
    }
}

pub struct Field<'a> {
    len: usize,
    inner: &'a [u8],
}

impl<'a> Field<'a> {
    pub fn try_parse(inner: &'a [u8]) -> Result<Self, Error> {
        if inner.len() >= 4 {
            let len = u16::from_le_bytes(inner[2..4].try_into().unwrap()) as usize;
            if inner.len() >= 4 + len {
                Ok(Self { len, inner })
            } else {
                Err(Error::NotEnoughBytes)
            }
        } else {
            Err(Error::NotEnoughBytes)
        }
    }

    pub fn get_id(&self) -> u16 {
        u16::from_le_bytes(self.inner[..2].try_into().unwrap())
    }

    pub fn get_payload(&self) -> &'a [u8] {
        &self.inner[4..4+self.len]
    }

    pub fn get_len(&self) -> usize {
        4 + self.len
    }
}

pub struct DeviceId<'a> {
    inner: Field<'a>
}

impl<'a> TryFrom<Field<'a>> for DeviceId<'a> {
    type Error = Error;
    fn try_from(inner: Field<'a>) -> Result<Self, Self::Error> {
        assert_eq!(inner.get_id(), DeviceInfoFieldId::DeviceId as u16);
        Ok(Self { inner })
    }
}

impl<'a> DeviceId<'a> {
    pub fn try_get(&self) -> Option<&'a str> {
        core::str::from_utf8(self.inner.get_payload()).ok()
    }
}

#[repr(u16)]
#[derive(Debug, PartialEq)]
enum DeviceInfoFieldId {
    DeviceId = 0
}

fn checksum(data: &[u8]) -> u16 {
    fletcher::calc_fletcher16(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_parse() {
        let mut buf = [0u8; 4096];
        let mut builder = DeviceInfoBuilder::try_alloc(&mut buf).
            expect("should have enough space in buffer to allocate builder");
        builder.add_device_id("grpl-probe-r1-0324");
        let size = builder.commit();

        let device_info = DeviceInfo::try_parse(&buf[..size]).expect("should produce valid device info");
        let field = device_info.get_fields().next().expect("should have a field");
        match field {
            DeviceInfoField::DeviceId(id) => assert_eq!(id.try_get(), Some("grpl-probe-r1-0324"))
        }
    }

    #[test]
    fn devicd_id_has_buffer_scope() {
        let mut buf = [0u8; 4096];
        let mut builder = DeviceInfoBuilder::try_alloc(&mut buf).
            expect("should have enough space in buffer to allocate builder");
        builder.add_device_id("grpl-probe-r1-0324");
        let size = builder.commit();

        let mut device_id: Option<&str> = None;
        let device_info = DeviceInfo::try_parse(&buf[..size]).expect("should produce valid device info");
        for field in device_info.get_fields() {
            match field {
                DeviceInfoField::DeviceId(devid_field) => device_id = devid_field.try_get()
            }
        }
        assert_eq!(device_id, Some("grpl-probe-r1-0324"));
    }
}