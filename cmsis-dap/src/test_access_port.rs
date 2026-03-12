// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use bitbuffer::{BitReadBuffer, BitReadStream, BitWriteStream, LittleEndian};

pub struct TestAccessPort<'a> {
    pub mock: MockSWDAccessPort,
    pub read_stream: BitReadStream<'a, LittleEndian>,
    pub write_stream: BitWriteStream<'a, LittleEndian>,
}

impl<'a> TestAccessPort<'a> {
    pub fn new(read_stream: BitReadStream<'a, LittleEndian>, write_stream: BitWriteStream<'a, LittleEndian>) -> Self {
        Self {
            mock: MockSWDAccessPort::new(),
            read_stream,
            write_stream,
        }
    }
}

impl<'a> crate::SWJAccessPort for TestAccessPort<'a> {
    fn set_frequency(&mut self, freq_hz: u32) -> bool {
        self.mock.set_frequency(freq_hz)
    }

    fn pins(&mut self, out: crate::Pins, mask: crate::Pins, wait_us: u32) -> crate::Pins {
        self.mock.pins(out, mask, wait_us)
    }

    fn write_sequence(&mut self, num_bits: usize, data: &[u8]) -> bool {
        <Self as crate::SWDAccessPort>::write(self, num_bits, data)
    }
}

impl<'a> crate::SWDAccessPort for TestAccessPort<'a> {
    const SUPPORTED: bool = true;

    fn open(&mut self) -> bool {
        self.mock.open()
    }

    fn close(&mut self) -> bool {
        self.mock.close()
    }

    fn read(&mut self, num_bits: usize, data: &mut [u8]) -> bool {
        if let Ok(bits) = self.read_stream.read_bits(num_bits) {
            let mut data_vec = Vec::<u8>::new();
            let mut writer = BitWriteStream::new(&mut data_vec, LittleEndian);
            writer.write_bits(&bits).expect("unable to write all bits");
            data.copy_from_slice(&data_vec);
            true
        } else {
            false
        }
    }

    fn write(&mut self, num_bits: usize, data: &[u8]) -> bool {
        let bits = BitReadStream::new(BitReadBuffer::new(data, LittleEndian)).read_bits(num_bits).
            expect("data not big enough for given bit count");
        self.write_stream.write_bits(&bits).expect("failed to write bits to the stream");
        true
    }
}

impl<'a> crate::JTAGAccessPort for TestAccessPort<'a> {
    const SUPPORTED: bool = true;

    fn open(&mut self) -> bool {
        self.mock.open()
    }

    fn close(&mut self) -> bool {
        self.mock.close()
    }

    fn clock(&mut self, cycles: usize, _tms: bool, tdi: bool) -> bool {
        let bytes = (cycles + 7) / 8;
        let data = if tdi {
            std::vec![0xFFu8; bytes]
        } else {
            std::vec![0x00u8; bytes]
        };
        let bits = BitReadStream::new(BitReadBuffer::new(&data, LittleEndian)).read_bits(cycles).unwrap();
        self.write_stream.write_bits(&bits).expect("failed to write bits to the stream");
        true
    }

    fn transfer(&mut self, num_bits: usize, _tms: bool, tdi: &[u8], tdo: &mut [u8]) -> bool {
        let bits = BitReadStream::new(BitReadBuffer::new(tdi, LittleEndian)).read_bits(num_bits).
            expect("data not big enough for given bit count");
        self.write_stream.write_bits(&bits).expect("failed to write bits to the stream");
        if tdo.len() > 0 {
            if let Ok(bits) = self.read_stream.read_bits(num_bits) {
                let mut data_vec = Vec::<u8>::new();
                let mut writer = BitWriteStream::new(&mut data_vec, LittleEndian);
                writer.write_bits(&bits).expect("unable to write all bits");
                tdo[..data_vec.len()].copy_from_slice(&data_vec);
                true
            } else {
                false
            }
        } else {
            true
        }
    }
}

#[mockall::automock]
pub trait SWDAccessPort {
    fn set_frequency(&mut self, freq_hz: u32) -> bool;

    fn pins(&mut self, out: crate::Pins, mask: crate::Pins, wait_us: u32) -> crate::Pins;

    fn open(&mut self) -> bool;

    fn close(&mut self) -> bool;
}