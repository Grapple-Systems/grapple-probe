// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::SWOPort;
use embassy_rp::uart;

impl SWOPort for uart::BufferedUart {
    const SUPPORTS_UART: bool = true;
    const SUPPORTS_MANCHESTER: bool = false;

    fn set_mode_uart(&mut self, baudrate: u32) -> bool {
        self.set_baudrate(baudrate);
        true
    }

    fn close(&mut self) {}

    async fn read_trace_data(&mut self, data: &mut [u8]) -> usize {
        use embedded_io_async::Read;
        self.read(data).await.unwrap_or(0)
    }
}