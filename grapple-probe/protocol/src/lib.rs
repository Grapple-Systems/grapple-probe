// Copyright (c) 2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

#![no_std]

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    NotEnoughBytes,
    InvalidPacket,
}

pub mod proto;
pub mod field;
