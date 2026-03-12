// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::{TransferRequest, TransferStatus};
use super::common::*;

pub struct TransferConfigureProps {}
impl CommandProperties for TransferConfigureProps {
    const TYPE: CommandType = CommandType::TransferConfigure;
}

pub struct TransferConfigureCommand<Buf> {
    inner: Packet<Buf>
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for TransferConfigureCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::TransferConfigure);
        if inner.get_payload().len() >= 5 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> TransferConfigureCommand<Buf> {
    pub fn get_idle_cycles(&self) -> u8 {
        self.inner.get_payload()[0]
    }

    pub fn get_wait_retry(&self) -> u16 {
        u16::from_le_bytes(self.inner.get_payload()[1..3].try_into().unwrap())
    }

    pub fn get_match_retry(&self) -> u16 {
        u16::from_le_bytes(self.inner.get_payload()[3..5].try_into().unwrap())
    }
}

pub type TransferConfigureResponse<Buf> = StandardResponse<TransferConfigureProps, Buf>;

pub struct TransferIterator<'a, Buf> {
    inner: &'a TransferCommand<Buf>,
    packet_index: usize,
    requests: usize,
}

impl<'a, Buf: AsRef<[u8]>> Iterator for TransferIterator<'a, Buf> {
    type Item = (TransferRequest, u32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.requests > 0 {
            let request = TransferRequest(*self.inner.inner.get_payload().get(self.packet_index)?);
            // there's only data if the request is a write or a read match.
            if !request.is_read() || request.is_match_mask() {
                let data = self.inner.inner.get_payload().get(self.packet_index + 1..self.packet_index + 5)?;
                let data = u32::from_le_bytes(data.try_into().unwrap());
                self.packet_index += 5;
                self.requests -= 1;
                Some((request, data))
            } else {
                self.packet_index += 1;
                self.requests -= 1;
                Some((request, 0))
            }
        } else {
            None
        }
    }
}

pub struct TransferCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for TransferCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::Transfer);
        if inner.get_payload().len() >= 2 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> TransferCommand<Buf> {
    pub fn get_index(&self) -> u8 {
        self.inner.get_payload()[0]
    }

    pub fn get_transfers<'a>(&'a self) -> TransferIterator<'a, Buf> {
        TransferIterator {
            inner: self,
            packet_index: 2,
            requests: self.inner.get_payload()[1] as usize,
        }
    }
}

pub struct OneTransferResponse<'a, Buf> {
    parent: &'a mut TransferResponse<Buf>,
    response_len: usize,
}

impl<'a, Buf: AsMut<[u8]>> OneTransferResponse<'a, Buf> {
    pub fn commit(self, status: TransferStatus, timestamp: Option<u32>, data: Option<u32>) {
        let payload = &mut self.parent.get_remaining_payload_mut()[..self.response_len];
        
        if let Some(timestamp) = timestamp {
            payload[..4].copy_from_slice(&timestamp.to_le_bytes());
        }
        if let Some(data) = data {
            if timestamp.is_some() {
                payload[4..8].copy_from_slice(&data.to_le_bytes());
            } else {
                payload[..4].copy_from_slice(&data.to_le_bytes());
            }
        }

        self.parent.set_status(status);
        self.parent.payload_index += self.response_len;
        self.parent.response_count += 1;
    }
}

pub struct TransferResponse<Buf> {
    inner: Packet<Buf>,
    payload_index: usize,
    response_count: usize,
}

impl<Buf: AsMut<[u8]>> TransferResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::Transfer)?;
        if inner.get_payload_mut().len() >= 2 {
            Ok(Self {
                inner,
                payload_index: 2,
                response_count: 0,
            })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn set_status(&mut self, status: TransferStatus) {

        self.inner.get_payload_mut()[1] = 
            if status.ack.is_ok() { 0x01 } else { 0 } |
            if status.ack.is_wait() { 0x02 } else { 0 } |
            if status.ack.is_fault() { 0x04 } else { 0 } |
            if status.ack.is_no_response() { 0x07 } else { 0 } |
            if status.protocol_error { 0x08 } else { 0 } |
            if status.value_mismatch { 0x10 } else { 0 };
    }

    pub fn try_allocate_response<'a>(&'a mut self, request: TransferRequest) -> Option<OneTransferResponse<'a, Buf>> {
        let timestamp_len = if request.timestamp() { 4 } else { 0 };
        // data is only in the response if the request is a normal read
        let data_len = if request.is_read() && !request.is_value_match() { 4 } else { 0 };
        let response_len = timestamp_len + data_len;
        if self.get_remaining_payload_mut().len() >= response_len {
            Some(OneTransferResponse { parent: self, response_len })
        } else {
            None
        }
    }

    pub fn commit(mut self) -> usize {
        self.inner.get_payload_mut()[0] = self.response_count as u8;
        self.inner.commit(self.payload_index)
    }

    fn get_remaining_payload_mut(&mut self) -> &mut [u8] {
        let i = self.payload_index;
        &mut self.inner.get_payload_mut()[i..]
    }
}

pub struct TransferBlockCommand<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsRef<[u8]>> TryFrom<Packet<Buf>> for TransferBlockCommand<Buf> {
    type Error = Error;

    fn try_from(inner: Packet<Buf>) -> Result<Self, Self::Error> {
        assert_eq!(inner.command, CommandType::TransferBlock);
        if inner.get_payload().len() >= 4 {
            Ok(Self { inner })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }
}

impl<Buf: AsRef<[u8]>> TransferBlockCommand<Buf> {
    pub fn get_index(&self) -> u8 {
        self.inner.get_payload()[0]
    }

    pub fn get_count(&self) -> usize {
        u16::from_le_bytes(self.inner.get_payload()[1..3].try_into().unwrap()) as usize
    }

    pub fn get_request(&self) -> TransferRequest {
        TransferRequest(self.inner.get_payload()[3])
    }

    pub fn get_data(&self) -> &[u8] {
        &self.inner.get_payload()[4..]
    }
}

pub struct TransferBlockResponse<Buf> {
    inner: Packet<Buf>,
}

impl<Buf: AsMut<[u8]>> TransferBlockResponse<Buf> {
    pub fn try_alloc(inner: Buf) -> Result<Self, Error> {
        let mut inner = Packet::try_alloc(inner, CommandType::TransferBlock)?;
        if inner.get_payload_mut().len() >= 1 {
            Ok(Self {
                inner,
            })
        } else {
            Err(Error::BufferNotBigEnough)
        }
    }

    pub fn set_status(&mut self, status: TransferStatus) {
        let response =
            if status.ack.is_ok() { 0x01 } else { 0 } |
            if status.ack.is_wait() { 0x02 } else { 0 } |
            if status.ack.is_fault() { 0x04 } else { 0 } |
            if status.ack.is_no_response() { 0x07 } else { 0 } |
            if status.protocol_error { 0x08 } else { 0 };
        self.inner.get_payload_mut()[2] = response;
    }

    pub fn get_data_mut(&mut self) -> &mut[u8] {
        &mut self.inner.get_payload_mut()[3..]
    }

    pub fn commit_with_data(mut self, count: usize) -> usize {
        self.inner.get_payload_mut()[..2].copy_from_slice(&(count as u16).to_le_bytes());
        let data_size = count * 4;
        self.inner.commit(3 + data_size)
    }

    pub fn commit_without_data(mut self, count: usize) -> usize {
        self.inner.get_payload_mut()[..2].copy_from_slice(&(count as u16).to_le_bytes());
        self.inner.commit(3)
    }
}