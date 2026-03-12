// Copyright (c) 2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use embedded_storage_async::nor_flash::NorFlash;
use sequential_storage::map::{MapStorage, MapConfig};
use sequential_storage::cache::NoCache;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    StorageError,
    DoesntExist,
}

pub struct ConfigStorage<F: NorFlash, const S: usize> {
    inner: MapStorage<u8, F, NoCache>,
    buffer: [u8; S],
}

impl<F: NorFlash<Error: defmt::Format>, const S: usize> ConfigStorage<F, S> {
    pub fn new(flash: F, range: core::ops::Range<u32>) -> Self {
        Self {
            inner: MapStorage::new(flash, MapConfig::new(range), NoCache::new()),
            buffer: [0u8; S],
        }
    }

    pub async fn read(&mut self, id: u8) -> Result<&[u8], Error> {
        let buffer = &mut self.buffer;
        self.inner.fetch_item::<&[u8]>(buffer, &id).await.
            map_err(|err| {
                defmt::error!("storage error when fetching an item: {=?}", err);
                Error::StorageError
            })?.ok_or(Error::DoesntExist)
    }

    pub async fn write(&mut self, id: u8, data: &[u8]) -> Result<(), Error> {
        let buffer = &mut self.buffer;
        self.inner.store_item(buffer, &id, &data).await.
            map_err(|err| {
                defmt::error!("storage error when storing an item: {=?}", err);
                Error::StorageError
            })
    }
}