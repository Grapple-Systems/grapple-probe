// Copyright (c) 2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

#![cfg_attr(not(feature = "std"), no_std)]

//! A library for interfacing with Grapple Probes.
//! 
//! ```no_run
//! // Print out the serial number, measured target voltage, and commanded signal voltage for each detected Grapple Probe.
//! smol::block_on(async {
//!     for probe in grapple_probe::GrappleProbe::all().await {
//!         println!("grapple probe: {}", probe.get_serial_number());
//! 
//!         let mut dap = probe.open_cmsis_dap().await.unwrap();
//!         let status = dap.get_status().await.unwrap();
//!         println!("\ttvcc: {} mv, signal: {} mv", status.target_mv, status.signal_mv);
//!     }
//! });
//! ```

/// Protocol packet definitions.
pub mod proto;

/// Configuration field definitions.
pub mod field;

/// Host-side USB communication (requires `std` feature).
#[cfg(feature = "std")]
mod host;

#[cfg(feature = "std")]
pub use host::{Error, GrappleProbe, GrappleProbeDebug, PowerControl, Status};
