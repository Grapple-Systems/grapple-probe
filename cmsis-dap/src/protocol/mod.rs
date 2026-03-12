// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

mod common;
mod general;
mod jtag;
mod swd;
mod swj;
mod swo;
mod transfer;

pub use common::{CommandType, Error};
pub use general::*;
pub use jtag::*;
pub use swd::*;
pub use swj::*;
pub use swo::*;
pub use transfer::*;

use common::Packet;

pub enum Command<Buf> {
    Info(InfoCommand<Buf>),
    HostStatus(HostStatusCommand<Buf>),
    Connect(ConnectCommand<Buf>),
    Disconnect(DisconnectCommand<Buf>),
    TransferConfigure(TransferConfigureCommand<Buf>),
    Transfer(TransferCommand<Buf>),
    TransferBlock(TransferBlockCommand<Buf>),
    WriteAbort(WriteAbortCommand<Buf>),
    Delay(DelayCommand<Buf>),
    ResetTarget(ResetTargetCommand<Buf>),
    SWJPins(SWJPinsCommand<Buf>),
    SWJClock(SWJClockCommand<Buf>),
    SWJSequence(SWJSequenceCommand<Buf>),
    SWDConfigure(SWDConfigureCommand<Buf>),
    SWDSequence(SWDSequenceCommand<Buf>),
    SWOTransport(SWOTransportCommand<Buf>),
    SWOMode(SWOModeCommand<Buf>),
    SWOBaudrate(SWOBaudrateCommand<Buf>),
    SWOControl(SWOControlCommand<Buf>),
    SWOStatus(SWOStatusCommand),
    SWOExtendedStatus(SWOExtendedStatusCommand<Buf>),
    SWOData(SWODataCommand<Buf>),
    JTAGSequence(JTAGSequenceCommand<Buf>),
    JTAGConfigure(JTAGConfigureCommand<Buf>),
    JTAGIDCode(JTAGIDCodeCommand<Buf>),
}

impl<Buf: AsRef<[u8]>> Command<Buf> {
    pub fn try_parse(inner: Buf) -> Result<Self, Error> {
        let packet = Packet::try_parse(inner)?;
        match packet.get_command() {
            CommandType::Info => Ok(Self::Info(InfoCommand::try_from(packet)?)),
            CommandType::HostStatus => Ok(Self::HostStatus(HostStatusCommand::try_from(packet)?)),
            CommandType::Connect => Ok(Self::Connect(ConnectCommand::try_from(packet)?)),
            CommandType::Disconnect => Ok(Self::Disconnect(DisconnectCommand::try_from(packet)?)),
            CommandType::TransferConfigure => Ok(Self::TransferConfigure(TransferConfigureCommand::try_from(packet)?)),
            CommandType::Transfer => Ok(Self::Transfer(TransferCommand::try_from(packet)?)),
            CommandType::TransferBlock => Ok(Self::TransferBlock(TransferBlockCommand::try_from(packet)?)),
            CommandType::WriteAbort => Ok(Self::WriteAbort(WriteAbortCommand::try_from(packet)?)),
            CommandType::Delay => Ok(Self::Delay(DelayCommand::try_from(packet)?)),
            CommandType::ResetTarget => Ok(Self::ResetTarget(ResetTargetCommand::try_from(packet)?)),
            CommandType::SWJPins => Ok(Self::SWJPins(SWJPinsCommand::try_from(packet)?)),
            CommandType::SWJClock => Ok(Self::SWJClock(SWJClockCommand::try_from(packet)?)),
            CommandType::SWJSequence => Ok(Self::SWJSequence(SWJSequenceCommand::try_from(packet)?)),
            CommandType::SWDConfigure => Ok(Self::SWDConfigure(SWDConfigureCommand::try_from(packet)?)),
            CommandType::SWDSequence => Ok(Self::SWDSequence(SWDSequenceCommand::try_from(packet)?)),
            CommandType::SWOTransport => Ok(Self::SWOTransport(SWOTransportCommand::try_from(packet)?)),
            CommandType::SWOMode => Ok(Self::SWOMode(SWOModeCommand::try_from(packet)?)),
            CommandType::SWOBaudrate => Ok(Self::SWOBaudrate(SWOBaudrateCommand::try_from(packet)?)),
            CommandType::SWOControl => Ok(Self::SWOControl(SWOControlCommand::try_from(packet)?)),
            CommandType::SWOStatus => Ok(Self::SWOStatus(SWOStatusCommand::try_from(packet)?)),
            CommandType::SWOExtendedStatus => Ok(Self::SWOExtendedStatus(SWOExtendedStatusCommand::try_from(packet)?)),
            CommandType::SWOData => Ok(Self::SWOData(SWODataCommand::try_from(packet)?)),
            CommandType::JTAGSequence => Ok(Self::JTAGSequence(JTAGSequenceCommand::try_from(packet)?)),
            CommandType::JTAGConfigure => Ok(Self::JTAGConfigure(JTAGConfigureCommand::try_from(packet)?)),
            CommandType::JTAGIDCode => Ok(Self::JTAGIDCode(JTAGIDCodeCommand::try_from(packet)?)),
            cmd => Err(Error::UnsupportedCommand(cmd as u8)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn swd_configure() {
        let packet = [0x13u8, 0];
        match Command::try_parse(&packet).expect("failed to parse valid packet") {
            Command::SWDConfigure(command) => {
                assert_eq!(command.get_turnaround(), 1);
                assert_eq!(command.get_always_data_phase(), false);
            },
            _ => assert!(false, "unexpected command response"),
        }

        let packet = [0x13u8, 0x07];
        match Command::try_parse(&packet).expect("failed to parse valid packet") {
            Command::SWDConfigure(command) => {
                assert_eq!(command.get_turnaround(), 4);
                assert_eq!(command.get_always_data_phase(), true);
            },
            _ => assert!(false, "unexpected command response"),
        }
    }

    #[test]
    fn transfer_read_one_packet() {
        use crate::types::TransferRequest;

        let packet = [5u8, 0, 1, 2];
        let command = Command::try_parse(packet).expect("failed to parse valid packet");
        match command {
            Command::Transfer(command) => {
                assert_eq!(command.get_index(), 0);
                let expected_transfers = [(TransferRequest(2), 0u32)];
                let transfers: Vec<(TransferRequest, u32)> = command.get_transfers().collect();
                assert_eq!(expected_transfers, transfers.as_slice(), "unexpected transfers");
            },
            _ => assert!(false, "unexpected command response"),
        }
    }

    #[test]
    fn transfer_response() {
        use crate::types::{TransferAck, TransferRequest, TransferStatus};

        let mut buffer = [0u8; 64];
        let mut response = TransferResponse::try_alloc(&mut buffer).expect("buffer should be big enough");
        
        let request = TransferRequest(0b00000010); // normal read
        let one_response = response.try_allocate_response(request).expect("there should be enough buffer for the response");
        one_response.commit(TransferStatus::ok(), None, Some(45));

        let request = TransferRequest(0b00000000); // normal write
        let one_response = response.try_allocate_response(request).expect("there should be enough buffer for the response");
        one_response.commit(TransferStatus::ok(), None, None);

        let request = TransferRequest(0b00100000); // write match mask
        let one_response = response.try_allocate_response(request).expect("there should be enough buffer for the response");
        let status = TransferStatus {
            ack: TransferAck::swd(7),
            protocol_error: true,
            value_mismatch: false,
        };
        one_response.commit(status, None, None);

        let request = TransferRequest(0b00010010); // read with value match
        let one_response = response.try_allocate_response(request).expect("there should be enough buffer for the response");
        one_response.commit(TransferStatus::ok(), None, None);

        let request = TransferRequest(0b10000010); // read with timestamp
        let one_response = response.try_allocate_response(request).expect("there should be enough buffer for the response");
        one_response.commit(TransferStatus::ok(), Some(44), Some(43));

        let packet_size = response.commit();

        let expected_packet = [0x05u8, 5, 0x01, 45, 0, 0, 0, 44, 0, 0, 0, 43, 0, 0, 0];
        assert_eq!(packet_size, expected_packet.len());
        assert_eq!(buffer[..packet_size], expected_packet);
    }
    
    #[test]
    fn transfer_block_response() {
        use crate::types::TransferStatus;
        
        let mut buffer = [0u8; 64];
        let mut response = TransferBlockResponse::try_alloc(&mut buffer).expect("there should be enough buffer for the response");
        response.set_status(TransferStatus::ok());
        let packet_size = response.commit_with_data(10);

        assert_eq!(packet_size, 4 + 10 * 4);
        assert_eq!(&buffer[..4], &[0x06, 0x0A, 0x00, 0x01]);

        let mut buffer = [0u8; 64];
        let mut response = TransferBlockResponse::try_alloc(&mut buffer).expect("there should be enough buffer for the response");
        response.set_status(TransferStatus::ok());
        let packet_size = response.commit_without_data(10);

        assert_eq!(packet_size, 4);
        assert_eq!(&buffer[..4], &[0x06, 0x0A, 0x00, 0x01]);
    }

    #[test]
    fn swd_sequence_targetsel() {
        let packet = [29u8, 3, 8, 153, 133, 33, 39, 41, 0, 1, 0];
        let command = Command::try_parse(packet).expect("failed to parse valid packet");
        let sequences = match command {
            Command::SWDSequence(command) => {
                command.sequences().map(|(num_bits, data)|(num_bits, data.to_vec())).collect::<Vec<(usize, Vec<u8>)>>()
            },
            _ => panic!("should be a swd sequence command"),
        };

        let expected_sequences = vec![(8usize, vec![153u8]), (5, Vec::<u8>::new()), (33, vec![39u8, 41, 0, 1, 0])];
        assert_eq!(expected_sequences, sequences);
    }

    #[test]
    fn write_abort_command() {
        let packet = [0x08u8, 0, 0x01, 0x23, 0x45, 0x67];
        let command = Command::try_parse(packet).expect("failed to parse valid packet");
        match command {
            Command::WriteAbort(command) => {
                assert_eq!(command.get_dap_index(), 0);
                assert_eq!(command.get_abort_value(), 0x67452301);
            },
            _ => panic!("should be a write abort command"),
        };
    }

    #[test]
    fn write_abort_response() {
        let mut packet = [0u8; 16];

        let response = WriteAbortResponse::try_alloc(&mut packet).expect("should be able to alloc write abort response");
        assert_eq!(response.commit_ok(), 2);
        assert_eq!(&packet[..2], &[0x08, 0x00]);

        let response = WriteAbortResponse::try_alloc(&mut packet).expect("should be able to alloc write abort response");
        assert_eq!(response.commit_err(), 2);
        assert_eq!(&packet[..2], &[0x08, 0xFF]);
    }

    #[test]
    fn delay_command() {
        let packet = [0x09u8, 0x01, 0x23];
        let command = Command::try_parse(packet).expect("failed to parse valid packet");
        match command {
            Command::Delay(command) => {
                assert_eq!(command.get_delay_us(), 0x2301);
            },
            _ => panic!("should be a delay command"),
        };
    }

    #[test]
    fn delay_response() {
        let mut packet = [0u8; 16];
        
        let response = DelayResponse::try_alloc(&mut packet).expect("should be able to alloc delay response");
        assert_eq!(response.commit_ok(), 2);
        assert_eq!(&packet[..2], &[0x09, 0x00]);
    }

    #[test]
    fn reset_target_command() {
        let packet = [0x0Au8];
        let command = Command::try_parse(packet).expect("failed to parse valid packet");
        match command {
            Command::ResetTarget(_) => (),
            _ => panic!("should be a delay command"),
        };
    }

    #[test]
    fn reset_target_response() {
        let mut packet = [0u8; 16];

        let mut response = ResetTargetResponse::try_alloc(&mut packet).expect("should be able to alloc reset target response");
        response.set_device_specific(false);
        assert_eq!(response.commit_ok(), 3);
        assert_eq!(&packet[..3], &[0x0A, 0x00, 0x00]);

        let mut response = ResetTargetResponse::try_alloc(&mut packet).expect("should be able to alloc reset target response");
        response.set_device_specific(true);
        assert_eq!(response.commit_ok(), 3);
        assert_eq!(&packet[..3], &[0x0A, 0x00, 0x01]);

        let response = ResetTargetResponse::try_alloc(&mut packet).expect("should be able to alloc reset target response");
        assert_eq!(response.commit_err(), 3);
        assert_eq!(&packet[..3], &[0x0A, 0xFF, 0x00]);
    }
}