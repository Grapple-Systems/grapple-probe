// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::MAX_PACKET_SIZE;
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

pub struct PacketBuffer<const SIZE: usize> {
    inner: bbqueue::BBBuffer<SIZE>,
    produce_signal: Signal<CriticalSectionRawMutex, bool>,
    consume_signal: Signal<CriticalSectionRawMutex, bool>,
}

impl<const SIZE: usize> PacketBuffer<SIZE> {
    pub const fn new() -> Self {
        Self {
            inner: bbqueue::BBBuffer::new(),
            produce_signal: Signal::new(),
            consume_signal: Signal::new(),
        }
    }

    pub fn try_split<'a>(&'a self) -> Option<(PacketBufferProducer<'a, SIZE>, PacketBufferConsumer<'a, SIZE>)> {
        self.inner.try_split().ok().map(|(prod, cons)| {
            (PacketBufferProducer { parent: self, inner: prod },
                PacketBufferConsumer { parent: self, inner: cons })
        })
    }
}

pub struct PacketBufferProducer<'a, const SIZE: usize> {
    parent: &'a PacketBuffer<SIZE>,
    inner: bbqueue::Producer<'a, SIZE>,
}

impl<'a, const SIZE: usize> PacketBufferProducer<'a, SIZE> {
    pub fn try_grant(&mut self) -> Option<PacketBufferWGrant<'a, SIZE>> {
        self.inner.grant_exact(MAX_PACKET_SIZE + 4).ok().map(|grant| {
            PacketBufferWGrant { parent: &self.parent, inner: grant }
        })
    }

    pub async fn grant(&mut self) -> PacketBufferWGrant<'a, SIZE> {
        loop {
            if let Some(grant) = self.try_grant() {
                return grant;
            }
            self.parent.consume_signal.wait().await;
        }
    }
}

pub struct PacketBufferWGrant<'a, const SIZE: usize> {
    parent: &'a PacketBuffer<SIZE>,
    inner: bbqueue::GrantW<'a, SIZE>,
}

impl<'a, const SIZE: usize> PacketBufferWGrant<'a, SIZE> {
    pub fn buf(&mut self) -> &mut [u8] {
        &mut self.inner.buf()[4..]
    }

    pub fn commit(mut self, packet_size: usize) {
        self.inner.buf()[..4].copy_from_slice(&(packet_size as u32).to_ne_bytes());
        self.inner.commit(packet_size + 4);
        self.parent.produce_signal.signal(true);
    }
}

pub struct PacketBufferConsumer<'a, const SIZE: usize> {
    parent: &'a PacketBuffer<SIZE>,
    inner: bbqueue::Consumer<'a, SIZE>,
}

impl<'a, const SIZE: usize> PacketBufferConsumer<'a, SIZE> {
    pub fn try_read(&mut self) -> Option<PacketBufferRGrant<'a, SIZE>> {
        if let Ok(grant) = self.inner.read() {
            if grant.buf().len() > 4 {
                let packet_size = u32::from_ne_bytes(grant.buf()[..4].try_into().unwrap()) as usize;
                if grant.buf().len() >= 4 + packet_size {
                    return Some(PacketBufferRGrant { parent: &self.parent, inner: grant, packet_size })
                }
            }
        }
        None
    }

    pub async fn read(&mut self) -> PacketBufferRGrant<'a, SIZE> {
        loop {
            if let Some(grant) = self.try_read() {
                return grant;
            }
            self.parent.produce_signal.wait().await;
        }
    }
}

pub struct PacketBufferRGrant<'a, const SIZE: usize> {
    parent: &'a PacketBuffer<SIZE>,
    inner: bbqueue::GrantR<'a, SIZE>,
    packet_size: usize,
}

impl<'a, const SIZE: usize> PacketBufferRGrant<'a, SIZE> {
    pub fn buf(&self) -> &[u8] {
        let packet_size = u32::from_ne_bytes(self.inner.buf()[..4].try_into().unwrap()) as usize;
        &self.inner.buf()[4..4+packet_size]
    }

    pub fn release(self) {
        self.inner.release(self.packet_size + 4);
        self.parent.consume_signal.signal(true);
    }
}