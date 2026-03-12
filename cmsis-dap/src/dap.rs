// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::*;
use crate::{JTAGAccessPort, SWDAccessPort, SWJAccessPort};

pub struct DebugAccessPort<P> {
    port: P,
    pub transfer_config: Transfer,
    state: State,
}

impl<P> DebugAccessPort<P> {
    pub fn new(port: P) -> Self {
        Self {
            port,
            transfer_config: Transfer::default(),
            state: State::Disconnected,
        }
    }
}

impl<P: JTAGAccessPort + SWJAccessPort + SWDAccessPort> DebugAccessPort<P> {
    pub fn connect(&mut self, mode: ConnectMode) -> Result<ConnectMode, ()> {
        match mode {
            ConnectMode::Default | ConnectMode::SWD => {
                if <P as SWDAccessPort>::SUPPORTED {
                    if <P as SWDAccessPort>::open(&mut self.port) {
                        self.state = State::SWD;
                        return Ok(ConnectMode::SWD);
                    }
                } else {
                    defmt::warn!("attempt to connect to SWD when not supported");
                }
            },
            ConnectMode::JTAG => {
                if <P as JTAGAccessPort>::SUPPORTED {
                    if <P as JTAGAccessPort>::open(&mut self.port) {
                        self.state = State::JTAG;
                        return Ok(ConnectMode::JTAG);
                    } else {
                        defmt::warn!("attempt to connect to JTAG when not supported");
                    }
                }
            }
            _ => ()
        };
        Err(())
    }

    pub fn disconnect(&mut self) -> bool {
        match self.state {
            State::SWD => {
                self.state = State::Disconnected;
                <P as SWDAccessPort>::close(&mut self.port)
            },
            State::JTAG => {
                self.state = State::Disconnected;
                <P as JTAGAccessPort>::close(&mut self.port)
            },
            State::Disconnected => true,
        }
    }

    pub fn swj_pins(&mut self, out: Pins, mask: Pins, wait_us: u32) -> Pins {
        <P as SWJAccessPort>::pins(&mut self.port, out, mask, wait_us)
    }

    pub fn swj_clock(&mut self, freq_hz: u32) -> bool {
        <P as SWJAccessPort>::set_frequency(&mut self.port, freq_hz)
    }

    pub fn swj_sequence(&mut self, num_bits: usize, data: &[u8]) -> bool {
        <P as SWJAccessPort>::write_sequence(&mut self.port, num_bits, data)
    }

    pub fn swd_read_sequence(&mut self, num_bits: usize, data: &mut [u8]) -> bool {
        if self.state == State::SWD {
            <P as SWDAccessPort>::read(&mut self.port, num_bits, data)
        } else {
            false
        }
    }

    pub fn swd_write_sequence(&mut self, num_bits: usize, data: &[u8]) -> bool {
        if self.state == State::SWD {
            <P as SWDAccessPort>::write(&mut self.port, num_bits, data)
        } else {
            false
        }
    }

    pub fn jtag_read_idcode(&mut self, index: u8) -> Option<u32> {
        let abort = AbortToken::new();
        if self.state == State::JTAG {
            if let Some(mut transfer) = self.transfer_config.try_operate_jtag(index as usize, &mut self.port, &abort) {
                let(id_code, status) = transfer.register_operator.read_idcode();
                if !status.has_errors() {
                    return Some(id_code);
                }
            }
        }
        None
    }

    pub fn jtag_sequence(&mut self, num_bits: usize, tms: bool, tdi: &[u8], tdo: &mut [u8]) -> bool {
        if self.state == State::JTAG {
            <P as JTAGAccessPort>::transfer(&mut self.port, num_bits, tms, tdi, tdo)
        } else {
            false
        }
    }

    pub fn write_abort(&mut self, index: u8, value: u32, abort: &AbortToken) -> TransferStatus {
        match self.state {
            State::SWD => self.transfer_config.operate_swd(&mut self.port, &abort).write(Register::debug_port(0), value),
            State::JTAG => {
                if let Some(mut transfer) = self.transfer_config.try_operate_jtag(index as usize, &mut self.port, &abort) {
                    transfer.register_operator.write_abort(value)
                } else {
                    TransferStatus::default()
                }
            },
            State::Disconnected => TransferStatus::default()
        }
    }

    pub fn transfer_write(&mut self, index: u8, register: Register, data: u32, abort: &AbortToken) -> TransferStatus {
        match self.state {
            State::SWD => self.transfer_config.operate_swd(&mut self.port, abort).write(register, data),
            State::JTAG => {
                if let Some(mut transfer) = self.transfer_config.try_operate_jtag(index as usize, &mut self.port, abort) {
                    transfer.write(register, data)
                } else {
                    TransferStatus::default()
                }
            },
            State::Disconnected => TransferStatus::default()
        }
    }

    pub fn transfer_read(&mut self, index: u8, register: Register, abort: &AbortToken) -> (u32, TransferStatus) {
        match self.state {
            State::SWD => self.transfer_config.operate_swd(&mut self.port, abort).read(register),
            State::JTAG => {
                if let Some(mut transfer) = self.transfer_config.try_operate_jtag(index as usize, &mut self.port, abort) {
                    transfer.read(register)
                } else {
                    (0, TransferStatus::default())
                }
            },
            State::Disconnected => (0, TransferStatus::default())
        }
    }

    pub fn transfer_read_match(&mut self, index: u8, register: Register, match_value: u32, abort: &AbortToken) -> TransferStatus {
        match self.state {
            State::SWD => self.transfer_config.operate_swd(&mut self.port, abort).read_match(register, match_value),
            State::JTAG => {
                if let Some(mut transfer) = self.transfer_config.try_operate_jtag(index as usize, &mut self.port, abort) {
                    transfer.read_match(register, match_value)
                } else {
                    TransferStatus::default()
                }
            },
            State::Disconnected => TransferStatus::default()
        }
    }

    pub fn transfer_read_block(&mut self, index: u8, register: Register, block: &mut [u8], abort: &AbortToken) -> (usize, TransferStatus) {
        match self.state {
            State::SWD => self.transfer_config.operate_swd(&mut self.port, abort).read_block(register, block),
            State::JTAG => {
                if let Some(mut transfer) = self.transfer_config.try_operate_jtag(index as usize, &mut self.port, abort) {
                    transfer.read_block(register, block)
                } else {
                    (0, TransferStatus::default())
                }
            },
            State::Disconnected => (0, TransferStatus::default())
        }
    }

    pub fn transfer_write_block(&mut self, index: u8, register: Register, block: &[u8], abort: &AbortToken) -> (usize, TransferStatus) {
        match self.state {
            State::SWD => self.transfer_config.operate_swd(&mut self.port, abort).write_block(register, block),
            State::JTAG => {
                if let Some(mut transfer) = self.transfer_config.try_operate_jtag(index as usize, &mut self.port, abort) {
                    transfer.write_block(register, block)
                } else {
                    (0, TransferStatus::default())
                }
            },
            State::Disconnected => (0, TransferStatus::default())
        }
    }
}

#[derive(Clone, Default)]
pub struct Transfer {
    pub idle_cycles: usize,
    pub wait_retry: usize,
    pub match_retry: usize,
    pub match_mask: u32,
    pub swd: SWD,
    pub jtag: JTAG<64>,
}

impl Transfer {
    fn operate_swd<'a, 'p, P: SWDAccessPort>(&self, port: &'p mut P, abort: &'a AbortToken) -> TransferOperator<'a, SWDOperator<'p, P>> {
        TransferOperator {
            config: self.clone(),
            register_operator: SWDOperator {
                config: self.swd.clone(),
                port
            },
            abort
        }
    }

    fn try_operate_jtag<'a, 'p, P: SWDAccessPort>(&self, index: usize, port: &'p mut P, abort: &'a AbortToken) -> Option<TransferOperator<'a, JTAGOperator<'p, P>>> {
        self.jtag.try_make_operator(index, port).map(|operator| {
            TransferOperator {
                config: self.clone(),
                register_operator: operator,
                abort,
            }
        })
    }
}

struct TransferOperator<'a, R> {
    config: Transfer,
    register_operator: R,
    abort: &'a AbortToken,
}

impl<'a, R: RegisterOperator> TransferOperator<'a, R> {
    pub fn write(&mut self, reg: Register, value: u32) -> TransferStatus {
        let mut status = self.register_operator.write_register(reg, value);
        if status.ack.is_wait() && !self.abort.is_set() {
            for _ in 0..self.config.wait_retry {
                self.register_operator.wait_idle_cycles(self.config.idle_cycles);
                status = self.register_operator.write_register(reg, value);
                if !status.ack.is_wait() || self.abort.is_set() {
                    break;
                }
                defmt::warn!("transfer write retry");
            }
        }
        status
    }

    pub fn write_block(&mut self, reg: Register, block: &[u8]) -> (usize, TransferStatus) {
        let count = block.len() / 4;
        assert!(count > 0);

        for i in 0..count {
            let value = u32::from_le_bytes(block[i * 4..(i + 1) * 4].try_into().unwrap());
            let status = self.write(reg, value);
            if status.has_errors() {
                return (i + 1, status);
            }
        }
        let (_, final_status) = self.read_inner(Register::rdbuff());
        (count, final_status)
    }

    pub fn read(&mut self, reg: Register) -> (u32, TransferStatus) {
        let (value, status) = self.read_inner(reg);
        if !status.has_errors() && (R::IS_JTAG || reg.ap) {
            self.read_inner(Register::rdbuff())
        } else {
            (value, status)
        }
    }

    pub fn read_block(&mut self, reg: Register, block: &mut [u8]) -> (usize, TransferStatus) {
        let count = block.len() / 4;
        assert!(count > 0);

        let mut status = TransferStatus::default();
        let post_read = R::IS_JTAG || reg.ap;
        if post_read {
            // post the first read if jtag or if this is an access port read
            (_, status) = self.read_inner(reg);
            if status.has_errors() {
                return (0, status);
            }
        }

        for i in 0..count {
            // read the last value through rdbuff
            let (value, new_status) = if post_read && i >= count - 1 {
                self.read_inner(Register::rdbuff())
            } else {
                self.read_inner(reg)
            };
            status = new_status;
            if status.has_errors() {
                return (i + 1, status);
            }
            block[i * 4..(i + 1) * 4].copy_from_slice(&value.to_le_bytes());
        }

        (count, status)
    }

    pub fn read_match(&mut self, reg: Register, match_value: u32) -> TransferStatus {
        let mut status = TransferStatus::default();
        for _ in 0..self.config.match_retry + 1 {
            let (value, new_status) = self.read(reg);
            status = new_status;
            if self.abort.is_set() || status.has_errors() {
                break;
            }
            status.value_mismatch = (value & self.config.match_mask) != match_value;
            if !status.value_mismatch {
                break;
            }
            defmt::warn!("transfer read_match retry");
        }
        status
    }

    fn read_inner(&mut self, reg: Register) -> (u32, TransferStatus) {
        let (mut value, mut status) = self.register_operator.read_register(reg);
        if status.ack.is_wait() && !self.abort.is_set() {
            for _ in 0..self.config.wait_retry {
                self.register_operator.wait_idle_cycles(self.config.idle_cycles);
                (value, status) = self.register_operator.read_register(reg);
                if !status.ack.is_wait() || self.abort.is_set() {
                    break;
                }
                defmt::warn!("transfer read_inner retry");
            }
        }
        (value, status)
    }
}

trait RegisterOperator {
    const IS_JTAG: bool;

    fn wait_idle_cycles(&mut self, cycles: usize) -> bool;

    fn read_register(&mut self, reg: Register) -> (u32, TransferStatus);

    fn write_register(&mut self, reg: Register, data: u32) -> TransferStatus;
}

#[derive(Clone)]
pub struct SWD {
    pub turnaround: usize,
    pub always_data_phase: bool
}

impl Default for SWD {
    fn default() -> Self {
        Self {
            turnaround: 1,
            always_data_phase: false,
        }
    }
}

pub struct SWDOperator<'a, P> {
    config: SWD,
    port: &'a mut P,
}

impl<'a, P: SWDAccessPort> RegisterOperator for SWDOperator<'a, P> {
    const IS_JTAG: bool = false;

    fn wait_idle_cycles(&mut self, mut cycles: usize) -> bool {
        let mut ok = true;
        for _ in 0..((cycles + 31) / 32) {
            let num_bits = cycles.min(32);
            ok &= self.port.write(num_bits, &[0u8, 0, 0, 0]);
            cycles -= num_bits;
        }
        ok
    }

    fn read_register(&mut self, register: Register) -> (u32, TransferStatus) {
        let command_byte = Command::read(register);
        let mut status = TransferStatus::default();
        let mut ack = [0u8; 1];
        let mut data = [0u8; 5];
        status.protocol_error = !self.port.write(8, &[command_byte]);
        status.protocol_error |= !self.port.read(self.config.turnaround + 3, &mut ack);
        if !status.protocol_error {
            status.ack = TransferAck::swd(ack[0] >> self.config.turnaround);
            if self.config.always_data_phase || status.ack.is_ok() {
                status.protocol_error |= !self.port.read(33 + self.config.turnaround, &mut data);
            } else {
                status.protocol_error |= !self.port.read(self.config.turnaround, &mut ack);
            }
        }

        let mut reg_value = 0u32;
        if !status.protocol_error && status.ack.is_ok() {
            reg_value = u32::from_le_bytes(data[..4].try_into().unwrap());
            let parity = (reg_value.count_ones() & 0x01) as u8;
            status.protocol_error = parity != data[4] & 0x01;
        }
        (reg_value, status)
    }

    fn write_register(&mut self, register: Register, data: u32) -> TransferStatus {
        let command_byte = Command::write(register);
        let mut status = TransferStatus::default();
        let mut ack = [0u8; 1];
        let parity = (data.count_ones() & 0x01) as u8;
        status.protocol_error = !self.port.write(8, &[command_byte]);
        status.protocol_error |= !self.port.read(self.config.turnaround + 3, &mut ack);
        if !status.protocol_error {
            status.ack = TransferAck::swd(ack[0] >> self.config.turnaround);
            status.protocol_error |= !self.port.read(self.config.turnaround, &mut ack);
            if self.config.always_data_phase || status.ack.is_ok() {
                status.protocol_error |= !self.port.write(32, &data.to_le_bytes());
                status.protocol_error |= !self.port.write(1, &[parity]);
            }
        }

        status
    }
}

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
enum JTAGInstruction {
    Abort = 0xF8,
    DebugPort = 0xFA,
    AccessPort = 0xFB,
    IDCode = 0xFE,
    Bypass = 0xFF,
}

#[derive(Clone)]
pub struct JTAG<const N: usize> {
    pub ir_lengths: [u8; N],
    pub device_count: usize,
    ir_before_bits: [u16; N],
    ir_total_bits: usize,
}

impl<const N: usize> Default for JTAG<N> {
    fn default() -> Self {
        Self {
            ir_lengths: [0u8; N],
            device_count: 0,
            ir_before_bits: [0u16; N],
            ir_total_bits: 0,
        }
    }
}

impl<const N: usize> JTAG<N> {
    pub fn recalculate(&mut self) {
        for i in 1..self.device_count {
            self.ir_before_bits[i] = self.ir_before_bits[i - 1] + (self.ir_lengths[i - 1] as u16);
        }
        self.ir_total_bits = (self.ir_before_bits[self.device_count - 1] as usize) + (self.ir_lengths[self.device_count - 1] as usize);
    }

    pub fn try_make_operator<'a, P>(&self, index: usize, port: &'a mut P) -> Option<JTAGOperator<'a, P>> {
        if index < self.device_count {
            let ir_size_bits = self.ir_lengths[index] as usize;
            let ir_before_bits = self.ir_before_bits[index] as usize;
            Some(JTAGOperator {
                ir_size_bits, ir_before_bits, index, port,
                ir_after_bits: self.ir_total_bits - ir_size_bits - ir_before_bits,
                count: self.device_count,
                instruction: JTAGInstruction::Bypass,
            })
        } else {
            None
        }
    }
}

pub struct JTAGOperator<'a, P> {
    ir_size_bits: usize,
    ir_before_bits: usize,
    ir_after_bits: usize,
    index: usize,
    count: usize,
    instruction: JTAGInstruction,
    port: &'a mut P,
}

impl<'a, P: JTAGAccessPort> RegisterOperator for JTAGOperator<'a, P> {
    const IS_JTAG: bool = true;

    fn wait_idle_cycles(&mut self, cycles: usize) -> bool {
        self.port.clock(cycles, false, false);
        true
    }

    fn read_register(&mut self, reg: Register) -> (u32, TransferStatus) {
        let instruction = if reg.ap { JTAGInstruction::AccessPort } else { JTAGInstruction::DebugPort };
        if instruction != self.instruction {
            self.write_ir(instruction);
        }

        let addr = 0x01 | reg.address << 1;
        match self.transfer_reg(addr, 0) {
            Ok(value) => (value, TransferStatus::ok()),
            Err(ack) => (0, TransferStatus::from_ack(ack)),
        }
    }

    fn write_register(&mut self, reg: Register, data: u32) -> TransferStatus {
        let instruction = if reg.ap { JTAGInstruction::AccessPort } else { JTAGInstruction::DebugPort };
        if instruction != self.instruction {
            self.write_ir(instruction);
        }

        let addr = reg.address << 1;
        match self.transfer_reg(addr, data) {
            Ok(_) => TransferStatus::ok(),
            Err(ack) => TransferStatus::from_ack(ack),
        }
    }
}

impl<'a, P: JTAGAccessPort> JTAGOperator<'a, P> {
    pub fn read_idcode(&mut self) -> (u32, TransferStatus) {
        self.write_ir(JTAGInstruction::IDCode);
        match self.transfer_reg(0, 0) {
            Ok(value) => (value, TransferStatus::ok()),
            Err(ack) => (0, TransferStatus::from_ack(ack))
        }
    }

    pub fn write_abort(&mut self, value: u32) -> TransferStatus {
        self.write_ir(JTAGInstruction::Abort);
        match self.transfer_reg(0, value) {
            Ok(_) => TransferStatus::ok(),
            Err(ack) => TransferStatus::from_ack(ack),
        }
    }

    fn write_ir(&mut self, instruction: JTAGInstruction) {
        self.port.clock(2, true, false); // dr-scan, ir-scan
        self.port.clock(self.ir_before_bits + 2, false, true); // capture-ir, shift-ir, ir_before_bits x shift-ir

        let data = [instruction as u8];
        if self.ir_after_bits > 0 {
            self.port.transfer(self.ir_size_bits, false, &data, &mut []); // ir_size_bits x shift-ir
            self.port.clock(self.ir_after_bits - 1, false, true); // ir_after_bits - 1 x shift-ir
            self.port.clock(2, true, true); // exit1-ir, update-ir
        } else {
            self.port.transfer(self.ir_size_bits - 1, false, &data, &mut []); // ir_size_bits - 1 x shift-ir
            let last_bit = ((instruction as u8) & (1 << (self.ir_size_bits - 1))) != 0;
            self.port.clock(2, true, last_bit); // exit1-ir, update-ir
        }
        self.port.clock(1, false, true); // idle
        self.instruction = instruction;
    }

    fn transfer_reg(&mut self, addr: u8, data: u32) -> Result<u32, TransferAck> {
        // setup for dr
        self.port.clock(1, true, true); // dr-scan
        self.port.clock(2, false, true); // capture-dr, shift-dr

        // bypass everything up to the device we're interested in
        self.port.clock(self.index, false, true); // index x shift-dr

        // id code is the only register that doesn't take a 3 bit address before the value.
        if self.instruction != JTAGInstruction::IDCode {
            // write the instruction
            let mut data_in = [0u8; 1];
            self.port.transfer(3, false, &[addr], &mut data_in); // 3x shift-dr
            let ack = TransferAck::jtag(data_in[0]);
            if !ack.is_ok() {
                self.port.clock(2, true, true); // exit1-dr, update-dr
                self.port.clock(1, false, true); // idle
                return Err(ack);
            }
        }

        // transfer the data
        if self.index < self.count - 1 {
            let mut reg_data = [0u8; 4];
            self.port.transfer(32, false, &data.to_le_bytes(), &mut reg_data); // 32x shift-dr
            self.port.clock(self.count - self.index - 2, false, true); // shift-dr to count-1
            self.port.clock(2, true, true); // exit1-dr, update-dr
            self.port.clock(1, false, true); // idle
            Ok(u32::from_le_bytes(reg_data.try_into().unwrap()))
        } else {
            let mut reg_data = [0u8; 4];
            self.port.transfer(31, false, &data.to_le_bytes(), &mut reg_data); // 31x shift-dr
            let mut last_bit = [0u8; 1];
            self.port.transfer(1, true, &[(data >> 31) as u8], &mut last_bit); // exit1-dr
            self.port.clock(1, true, true); // update-dr
            self.port.clock(1, false, true); // idle
            Ok(u32::from_le_bytes(reg_data.try_into().unwrap()) | ((last_bit[0] as u32) << 31))
        }
    }
}

struct Command {}

impl Command {
    pub fn write(register: Register) -> u8 {
        let mut command_value = 0b10000001u8;
        if register.ap {
            command_value |= 0b10;
        }
        command_value |= (register.address & 0b11) << 3;
        let parity = (command_value.count_ones() & 0x01) as u8;
        command_value | parity << 5
    }

    pub fn read(register: Register) -> u8 {
        let mut command_value = 0b10000101u8;
        if register.ap {
            command_value |= 0b10;
        }
        command_value |= (register.address & 0b11) << 3;
        let parity = (command_value.count_ones() & 0x01) as u8;
        command_value | parity << 5
    }
}

#[derive(PartialEq)]
enum State {
    Disconnected,
    SWD,
    JTAG,
}

#[cfg(test)]
mod test {
    use bitbuffer::{BitWriteStream, BitReadBuffer, BitReadStream, LittleEndian};
    use crate::test_access_port::TestAccessPort;
    use super::*;

    #[tokio::test]
    async fn read_register() {
        let mut data_vec = Vec::<u8>::new();
        let mut data_builder = BitWriteStream::new(&mut data_vec, LittleEndian);
        data_builder.write_bits(&BitReadStream::new(BitReadBuffer::new(&[0x2u8], LittleEndian)).read_bits(4).unwrap()).unwrap();
        data_builder.write_int(0x01234567, 32).unwrap();
        data_builder.write_bool(false).unwrap();
        data_builder.write_bool(false).unwrap();
        let num_bits = data_builder.bit_len();

        let reader = BitReadStream::new(BitReadBuffer::new(&data_vec, LittleEndian)).read_bits(num_bits).unwrap();
        let mut writer_vec = Vec::<u8>::new();
        let writer = BitWriteStream::new(&mut writer_vec, LittleEndian);

        let mut port = TestAccessPort::new(reader, writer);
        port.mock.expect_open().once().return_const(true);
        let mut dap = DebugAccessPort::new(port);

        let abort = AbortToken::new();
        assert_eq!(dap.connect(ConnectMode::SWD), Ok(ConnectMode::SWD));
        let (value, status) = dap.transfer_read(0, Register::debug_port(1), &abort);
        assert!(!status.has_errors());
        assert_eq!(value, 0x01234567);
        assert_eq!(&writer_vec, &[0b10001101u8]);
    }

    #[tokio::test]
    async fn write_register() {
        let mut data_vec = Vec::<u8>::new();
        let mut data_builder = BitWriteStream::new(&mut data_vec, LittleEndian);
        data_builder.write_bits(&BitReadStream::new(BitReadBuffer::new(&[0x2u8], LittleEndian)).read_bits(5).unwrap()).unwrap();
        let num_bits = data_builder.bit_len();

        let reader = BitReadStream::new(BitReadBuffer::new(&data_vec, LittleEndian)).read_bits(num_bits).unwrap();
        let mut writer_vec = Vec::<u8>::new();
        let writer = BitWriteStream::new(&mut writer_vec, LittleEndian);

        let mut port = TestAccessPort::new(reader, writer);
        port.mock.expect_open().once().return_const(true);
        let mut dap = DebugAccessPort::new(port);

        let abort = AbortToken::new();
        assert_eq!(dap.connect(ConnectMode::SWD), Ok(ConnectMode::SWD));
        let status = dap.transfer_write(0, Register::access_port(1), 0x01234567, &abort);
        assert!(!status.has_errors());
        assert_eq!(&writer_vec, &[0b10001011u8, 0x67, 0x45, 0x23, 0x01, 0x00]);
    }

    #[tokio::test]
    async fn read_mask_register() {
        let mut data_vec = Vec::<u8>::new();
        let mut data_builder = BitWriteStream::new(&mut data_vec, LittleEndian);
        data_builder.write_bits(&BitReadStream::new(BitReadBuffer::new(&[0x2u8], LittleEndian)).read_bits(4).unwrap()).unwrap();
        data_builder.write_int(0x01234567, 32).unwrap();
        data_builder.write_bool(false).unwrap();
        data_builder.write_bool(false).unwrap();
        let num_bits = data_builder.bit_len();

        let reader = BitReadStream::new(BitReadBuffer::new(&data_vec, LittleEndian)).read_bits(num_bits).unwrap();
        let mut writer_vec = Vec::<u8>::new();
        let writer = BitWriteStream::new(&mut writer_vec, LittleEndian);

        let mut port = TestAccessPort::new(reader, writer);
        port.mock.expect_open().once().return_const(true);
        let mut dap = DebugAccessPort::new(port);

        let abort = AbortToken::new();
        assert_eq!(dap.connect(ConnectMode::SWD), Ok(ConnectMode::SWD));
        dap.transfer_config.match_mask = 0x00FFFFFF;
        let status = dap.transfer_read_match(0, Register::debug_port(1), 0x00234567, &abort);
        assert!(!status.has_errors());
        assert_eq!(&writer_vec, &[0b10001101u8]);
    }
}