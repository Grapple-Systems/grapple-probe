// Copyright (c) 2025-2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::protocol as proto;
use crate::{JTAGAccessPort, SWDAccessPort, SWJAccessPort};
use crate::dap::DebugAccessPort;
use crate::swo::SWOAccessBehavior;
use crate::types::{AbortToken, TransferStatus};
use crate::MAX_PACKET_SIZE;
use crate::packet_buffer::PacketBufferConsumer;

pub trait Responder {
    async fn write_packet(&mut self, packet: &[u8]);
}

pub struct Dispatcher<'a, P, R, SWO> {
    dap: DebugAccessPort<P>,
    reactor: &'a R,
    swo: Option<SWO>,
}

impl<'a, P: JTAGAccessPort + SWJAccessPort + SWDAccessPort, R: crate::Reactor, SWO: SWOAccessBehavior> Dispatcher<'a, P, R, SWO> {
    pub fn new(port: P, reactor: &'a R, swo: Option<SWO>) -> Self {
        Self { dap: DebugAccessPort::new(port), reactor, swo }
    } 

    pub async fn dispatch_all<'b, Res: Responder, const S: usize>(&mut self, mut commands: PacketBufferConsumer<'b, S>, mut responder: Res, abort: &AbortToken) {
        let mut response = [0u8; MAX_PACKET_SIZE];
        loop {
            let grant = commands.read().await;
            let response_size = self.dispatch_one(grant.buf(), &mut response, abort).await;
            if response_size > 0 {
                responder.write_packet(&response[..response_size]).await;
            }
            grant.release();
        }
    }

    pub async fn dispatch_one(&mut self, command: &[u8], response: &mut [u8], abort: &AbortToken) -> usize {
        match proto::Command::try_parse(command) {
            Ok(proto::Command::Info(command)) => self.handle_info(&command, response),
            Ok(proto::Command::HostStatus(command)) => self.handle_host_status(&command, response).await,
            Ok(proto::Command::Connect(command)) => self.handle_connect(&command, response),
            Ok(proto::Command::Disconnect(_)) => self.handle_disconnect(response),
            Ok(proto::Command::TransferConfigure(command)) => self.handle_transfer_configure(&command, response),
            Ok(proto::Command::Transfer(command)) => self.handle_transfer(&command, response, abort),
            Ok(proto::Command::TransferBlock(command)) => self.handle_transfer_block(&command, response, abort),
            Ok(proto::Command::WriteAbort(command)) => self.handle_write_abort(&command, response, abort),
            Ok(proto::Command::Delay(command)) => self.handle_delay(&command, response).await,
            Ok(proto::Command::ResetTarget(command)) => self.handle_reset_target(&command, response),
            Ok(proto::Command::SWJPins(command)) => self.handle_swj_pins(&command, response),
            Ok(proto::Command::SWJClock(command)) => self.handle_swj_clock(&command, response),
            Ok(proto::Command::SWJSequence(command)) => self.handle_swj_sequence(&command, response),
            Ok(proto::Command::SWDConfigure(command)) => self.handle_swd_configure(&command, response),
            Ok(proto::Command::SWDSequence(command)) => self.handle_swd_sequence(&command, response),
            Ok(proto::Command::SWOTransport(command)) => self.handle_swo_transport(&command, response),
            Ok(proto::Command::SWOMode(command)) => self.handle_swo_mode(&command, response),
            Ok(proto::Command::SWOBaudrate(command)) => self.handle_swo_baudrate(&command, response),
            Ok(proto::Command::SWOControl(command)) => self.handle_swo_control(&command, response),
            Ok(proto::Command::SWOStatus(_)) => self.handle_swo_status(response),
            Ok(proto::Command::SWOExtendedStatus(command)) => self.handle_swo_extended_status(&command, response),
            Ok(proto::Command::SWOData(command)) => self.handle_swo_data(&command, response),
            Ok(proto::Command::JTAGSequence(command)) => self.handle_jtag_sequence(&command, response),
            Ok(proto::Command::JTAGConfigure(command)) => self.handle_jtag_configure(&command, response),
            Ok(proto::Command::JTAGIDCode(command)) => self.handle_jtag_idcode(&command, response),
            Err(proto::Error::UnsupportedCommand(_)) => self.reactor.unrecognized_packet(command, response).await, 
            Err(_) => {
                defmt::error!("unexpected error processing dap command");
                proto::Error::respond(response).unwrap_or(0)
            }
        }
    }

    fn handle_info(&mut self, command: &proto::InfoCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::InfoResponse::try_alloc(response).expect("couldn't allocate info response");

        match command.get_id() {
            0x01..=0x03 => response.commit(0), // indicate that usb has the correct values
            0x04 => response.commit_bytes("2.1.1".as_bytes()),
            0x05..=0x08 => response.commit(0), // don't support on-board debugging
            0x09 => {
                if let Some(fw_version) = self.reactor.get_firmware_version() {
                    response.commit_bytes(fw_version.as_bytes())
                } else {
                    response.commit(0)
                }
            },
            0xF0 => {
                let capabilities = 
                    if <P as SWDAccessPort>::SUPPORTED { 0x01 } else { 0x00 } |
                    if <P as JTAGAccessPort>::SUPPORTED { 0x02 } else { 0x00 } |
                    if SWO::SUPPORTS_UART { 0x04 } else { 0x00 } |
                    if SWO::SUPPORTS_MANCHESTER { 0x08 } else { 0x00 } |
                    0x40; // swo streaming trace
                response.commit_bytes(&[capabilities])
            }, // capabilities
            0xFD => response.commit_bytes(&(SWO::BUFFER_SIZE as u32).to_le_bytes()),
            0xFE => response.commit_bytes(&[1u8]), // packet count
            0xFF => response.commit_bytes(&64u16.to_le_bytes()), // packet size
            id => {
                defmt::warn!("unrecognized info command: {}", id);
                response.commit_unrecognized()
            },
        }
    }

    async fn handle_host_status(&mut self, command: &proto::HostStatusCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::HostStatusResponse::try_alloc(response).expect("couldn't allocate host status response");
        self.reactor.host_status(command.get()).await;
        response.commit()
    }

    fn handle_connect(&mut self, command: &proto::ConnectCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::ConnectResponse::try_alloc(response).expect("couldn't allocate connect response");

        defmt::info!("dispatcher: handle connect");

        if let Ok(connected_mode) = self.dap.connect(command.get_mode()) {
            response.commit_ok(connected_mode)
        } else {
            response.commit_err()
        }
    }

    fn handle_disconnect(&mut self, response: &mut [u8]) -> usize {
        let response = proto::DisconnectResponse::try_alloc(response).expect("couldn't allocate disconnect response");

        defmt::info!("dispatcher: handle disconnect");

        if self.dap.disconnect() {
            if let Some(swo) = self.swo.as_mut() {
                swo.set_mode(crate::types::SWOMode::Off);
            }
            response.commit_ok()
        } else {
            response.commit_err()
        }
    }

    fn handle_transfer_configure(&mut self, command: &proto::TransferConfigureCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::TransferConfigureResponse::try_alloc(response).expect("couldn't allocate transfer configure response");

        defmt::info!("transfer configure; idle_cycles: {}, wait_retry: {}, match_retry: {}",
            command.get_idle_cycles(), command.get_wait_retry(), command.get_match_retry());

        self.dap.transfer_config.idle_cycles = command.get_idle_cycles() as usize;
        self.dap.transfer_config.wait_retry = command.get_wait_retry() as usize;
        self.dap.transfer_config.match_retry = command.get_match_retry() as usize;

        response.commit_ok()
    }

    fn handle_transfer(&mut self, command: &proto::TransferCommand<&[u8]>, response: &mut [u8], abort: &AbortToken) -> usize {
        let mut response = proto::TransferResponse::try_alloc(response).expect("couldn't allocate transfer response");

        let index = command.get_index();
        for (request, data) in command.get_transfers() {
            if let Some(response) = response.try_allocate_response(request) {
                self.reactor.activity();
                let timestamp = request.timestamp().then(|| {
                    let ticks = embassy_time::Instant::now().as_ticks();
                    (ticks & (u32::MAX as u64)) as u32
                });
                let register = request.get_register();
                if request.is_read() {
                    if request.is_value_match() {
                        let status = self.dap.transfer_read_match(index, register, data, abort);
                        response.commit(status, None, None);
                    } else {
                        let (data, status) = self.dap.transfer_read(index, register, abort);
                        if status.has_errors() {
                            defmt::warn!("dap transfer read error: {}", status.ack.get_inner());
                        }
                        response.commit(status, timestamp, Some(data));
                    }
                } else {
                    if request.is_match_mask() {
                        self.dap.transfer_config.match_mask = data;
                        response.commit(TransferStatus::ok(), None, None);
                    } else {
                        let status = self.dap.transfer_write(index, register, data, abort);
                        if status.has_errors() {
                            defmt::warn!("dap transfer write error: {}", status.ack.get_inner());
                        }
                        response.commit(status, timestamp, None);
                    }
                }
            } else {
                break;
            }

            if abort.is_set() {
                break;
            }
        }
        response.commit()
    }

    fn handle_transfer_block(&mut self, command: &proto::TransferBlockCommand<&[u8]>, response: &mut [u8], abort: &AbortToken) -> usize {
        let mut response = proto::TransferBlockResponse::try_alloc(response).expect("couldn't allocate transfer block response");

        self.reactor.activity();
        let index = command.get_index();
        let request = command.get_request();
        if request.is_read() {
            let block_size = response.get_data_mut().len().min(command.get_count() * 4);
            let (count, status) = self.dap.transfer_read_block(index, request.get_register(), &mut response.get_data_mut()[..block_size], abort);
            if status.has_errors() {
                defmt::warn!("dap transfer block read error: {}", status.ack.get_inner());
            }
            response.set_status(status);
            response.commit_with_data(count)
        } else {
            let block_size = command.get_data().len().min(command.get_count() * 4);
            let (count, status) = self.dap.transfer_write_block(index, request.get_register(), &command.get_data()[..block_size], abort);
            if status.has_errors() {
                defmt::warn!("dap transfer block write error: {}", status.ack.get_inner());
            }
            response.set_status(status);
            response.commit_without_data(count)
        }
    }

    fn handle_write_abort(&mut self, command: &proto::WriteAbortCommand<&[u8]>, response: &mut [u8], abort: &AbortToken) -> usize {
        let response = proto::WriteAbortResponse::try_alloc(response).expect("couldn't allocate write abort response");

        let index = command.get_dap_index();
        let data = command.get_abort_value();
        let status = self.dap.write_abort(index, data, abort);

        if status.has_errors() {
            response.commit_err()
        } else {
            response.commit_ok()
        }
    }

    async fn handle_delay(&mut self, command: &proto::DelayCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::DelayResponse::try_alloc(response).expect("couldn't allocate delay response");

        embassy_time::Timer::after_micros(command.get_delay_us() as u64).await;
        response.commit_ok()
    }

    fn handle_reset_target(&mut self, _command: &proto::ResetTargetCommand<&[u8]>, response: &mut [u8]) -> usize {
        let mut response = proto::ResetTargetResponse::try_alloc(response).expect("couldn't allocate reset target response");

        response.set_device_specific(false);
        response.commit_ok()
    }

    fn handle_swj_pins(&mut self, command: &proto::SWJPinsCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::SWJPinsResponse::try_alloc(response).expect("couldn't allocate swj pins response");

        self.reactor.activity();
        let input_pins = self.dap.swj_pins(
            command.get_output_pins(),
            command.get_selected_pins(),
            command.get_wait_us());
        response.commit(input_pins)
    }

    fn handle_swj_clock(&mut self, command: &proto::SWJClockCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::SWJClockResponse::try_alloc(response).expect("couldn't allocate swj clock response");

        defmt::info!("swj clock: {} Hz", command.get_freq_hz());

        if self.dap.swj_clock(command.get_freq_hz()) {
            response.commit_ok()
        } else {
            response.commit_err()
        }
    }

    fn handle_swj_sequence(&mut self, command: &proto::SWJSequenceCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::SWJSequenceResponse::try_alloc(response).expect("couldn't allocate swj sequence response");

        defmt::debug!("swj sequence started");

        self.reactor.activity();
        let (num_bits, data) = command.get_sequence();
        if self.dap.swj_sequence(num_bits, data) {
            defmt::debug!("swj sequence succeeded");
            response.commit_ok()
        } else {
            defmt::debug!("swj sequence failed");
            response.commit_err()
        }
    }

    fn handle_swd_configure(&mut self, command: &proto::SWDConfigureCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::SWDConfigureResponse::try_alloc(response).expect("couldn't allocate swd configure response");

        self.dap.transfer_config.swd.turnaround = command.get_turnaround();
        self.dap.transfer_config.swd.always_data_phase = command.get_always_data_phase();

        response.commit_ok()
    }

    fn handle_swd_sequence(&mut self, command: &proto::SWDSequenceCommand<&[u8]>, response: &mut [u8]) -> usize {
        let mut response = proto::SWDSequenceResponse::try_alloc(response).expect("couldn't allocate swd sequence response");

        self.reactor.activity();
        for (num_bits, data) in command.sequences() {
            if data.len() > 0 {
                if !self.dap.swd_write_sequence(num_bits, data) {
                    return response.commit_err();
                }
            } else {
                if let Some(mut response_data) = response.try_alloc_data(num_bits) {
                    if !self.dap.swd_read_sequence(num_bits, response_data.data()) {
                        return response.commit_err();
                    }
                    response_data.commit();
                }
            }
        }

        response.commit_ok()
    }

    fn handle_swo_transport(&mut self, command: &proto::SWOTransportCommand<&[u8]>, response: &mut [u8]) -> usize {
        if let Some(swo) = self.swo.as_mut() {
            let response = proto::SWOTransportResponse::try_alloc(response).expect("couldn't allocate swo transfer response");

            if command.try_get_transport().is_some_and(|transport| swo.set_transport(transport)) {
                response.commit_ok()
            } else {
                response.commit_err()
            }
        } else {
            proto::Error::respond(response).expect("unable to allocate unsupported response")
        }
    }

    fn handle_swo_mode(&mut self, command: &proto::SWOModeCommand<&[u8]>, response: &mut [u8]) -> usize {
        if let Some(swo) = self.swo.as_mut() {
            let response = proto::SWOModeResponse::try_alloc(response).expect("couldn't allocate swo mode response");

            defmt::info!("dispatcher: handle swo mode");

            if command.try_get_mode().is_some_and(|mode| swo.set_mode(mode)) {
                response.commit_ok()
            } else {
                response.commit_err()
            }
        } else {
            proto::Error::respond(response).expect("unable to allocate unsupported response")
        }
    }

    fn handle_swo_baudrate(&mut self, command: &proto::SWOBaudrateCommand<&[u8]>, response: &mut [u8]) -> usize {
        if let Some(swo) = self.swo.as_mut() {
            let response = proto::SWOBaudrateResponse::try_alloc(response).expect("couldn't allocate swo baudrate response");

            if swo.set_baudrate(command.get_baudrate()) {
                response.commit(command.get_baudrate())
            } else {
                response.commit(0)
            }
        } else {
            proto::Error::respond(response).expect("unable to allocate unsupported response")
        }
    }

    fn handle_swo_control(&mut self, command: &proto::SWOControlCommand<&[u8]>, response: &mut [u8]) -> usize {
        if let Some(swo) = self.swo.as_mut() {
            let response = proto::SWOControlResponse::try_alloc(response).expect("couldn't allocate swo control response");

            swo.set_active(command.is_start());
            response.commit_ok()
        } else {
            proto::Error::respond(response).expect("unable to allocate unsupported response")
        }
    }

    fn handle_swo_status(&mut self, response: &mut [u8]) -> usize {
        if let Some(swo) = self.swo.as_mut() {
            let response = proto::SWOStatusResponse::try_alloc(response).expect("couldn't allocate swo status response");
            response.commit(&swo.get_status())
        } else {
            proto::Error::respond(response).expect("unable to allocate unsupported response")
        }
    }

    fn handle_swo_extended_status(&mut self, command: &proto::SWOExtendedStatusCommand<&[u8]>, response: &mut [u8]) -> usize {
        if let Some(swo) = self.swo.as_mut() {
            let response = proto::SWOExtendedStatusResponse::try_alloc(response, command.get_control()).expect("couldn't allocate swo extended status response");
            response.commit(&swo.get_status())
        } else {
            proto::Error::respond(response).expect("unable to allocate unsupported response")
        }
    }

    fn handle_swo_data(&mut self, command: &proto::SWODataCommand<&[u8]>, response: &mut [u8]) -> usize {
        if let Some(swo) = self.swo.as_mut() {
            let mut response = proto::SWODataResponse::try_alloc(response).expect("couldn't allocate swo extended data response");
            let max_read_size = response.get_trace_data_mut().len().min(command.get_max_trace_count() as usize);
            let data_read = swo.read_trace_data(&mut response.get_trace_data_mut()[..max_read_size]).unwrap_or(0);
            response.commit(&swo.get_status(), data_read as u16)
        } else {
            proto::Error::respond(response).expect("unable to allocate unsupported response")
        }
    }

    fn handle_jtag_sequence(&mut self, command: &proto::JTAGSequenceCommand<&[u8]>, response: &mut [u8]) -> usize {
        let mut response = proto::JTAGSequenceResponse::try_alloc(response).expect("couldn't allocate jtag sequence response");
        
        defmt::debug!("jtag sequence started");

        self.reactor.activity();
        let mut tdo_idx = 0;
        let mut status = true;
        for sequence in command.get_sequences() {
            let tdo = if sequence.get_capture_tdo() {
                &mut response.get_tdo_data_mut()[tdo_idx..tdo_idx+sequence.get_tdi_data().len()]
            } else {
                &mut []
            };
            tdo_idx += tdo.len();
            status = status && self.dap.jtag_sequence(sequence.get_num_bits(), sequence.get_tms(), sequence.get_tdi_data(), tdo);
            if !status {
                break;
            }
        }

        if status {
            defmt::debug!("jtag sequence ok");
            response.commit_ok(tdo_idx)
        } else {
            defmt::debug!("jtag sequence err");
            response.commit_err()
        }
    }

    fn handle_jtag_configure(&mut self, command: &proto::JTAGConfigureCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::JTAGConfigureResponse::try_alloc(response).expect("couldn't allocate jtag configure response");

        let ir_lengths = command.get_ir_lengths();
        if self.dap.transfer_config.jtag.ir_lengths.len() >= ir_lengths.len() {
            self.dap.transfer_config.jtag.device_count = ir_lengths.len();
            self.dap.transfer_config.jtag.ir_lengths[..ir_lengths.len()].copy_from_slice(ir_lengths);
            self.dap.transfer_config.jtag.recalculate();
            response.commit_ok()
        } else {
            response.commit_err()
        }
    }

    fn handle_jtag_idcode(&mut self, command: &proto::JTAGIDCodeCommand<&[u8]>, response: &mut [u8]) -> usize {
        let response = proto::JTAGIDCodeResponse::try_alloc(response).expect("couldn't allocate jtag id code response");

        self.reactor.activity();
        if let Some(idcode) = self.dap.jtag_read_idcode(command.get_index()) {
            response.commit_ok(idcode)
        } else {
            response.commit_err()
        }
    }
}

#[cfg(test)]
mod test {
    use bitbuffer::{BitWriteStream, BitReadBuffer, BitReadStream, LittleEndian};
    use crate::test_access_port::TestAccessPort;
    use super::*;

    struct FakeReactor {}
    impl crate::Reactor for FakeReactor {
        async fn unrecognized_packet(&self, _command: &[u8], response: &mut [u8]) -> usize {
            let data = "unsup";
            response[..data.len()].copy_from_slice(data.as_bytes());
            data.len()
        }
    }

    #[tokio::test]
    async fn transfer_read_command() {
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

        let reactor = FakeReactor {};
        let abort = AbortToken::new();
        let mut dispatcher = Dispatcher::<'_, _, _, ()>::new(port, &reactor, None);

        dispatcher.dap.connect(crate::types::ConnectMode::SWD).unwrap();
        let mut response = [0u8; 64];
        let size = dispatcher.dispatch_one(&[5, 0, 1, 2], &mut response, &abort).await;
        assert_eq!(size, 7);
        assert_eq!(&response[..size], &[5, 1, 1, 0x67, 0x45, 0x23, 0x01]);
    }

    #[tokio::test]
    async fn jtag_sequence() {
        let data_vec = [0u8;40];
        let reader = BitReadStream::new(BitReadBuffer::new(&data_vec, LittleEndian)).read_bits(data_vec.len() * 8).unwrap();
        let mut writer_vec = Vec::<u8>::new();
        let writer = BitWriteStream::new(&mut writer_vec, LittleEndian);

        let mut port = TestAccessPort::new(reader, writer);
        port.mock.expect_open().once().return_const(true);

        let reactor = FakeReactor {};
        let abort = AbortToken::new();
        let mut dispatcher = Dispatcher::<'_, _, _, ()>::new(port, &reactor, None);

        dispatcher.dap.connect(crate::types::ConnectMode::JTAG).unwrap();
        let mut response = [0u8; 64];
        let request_buf = [20u8, 12, 65, 0, 65, 0, 65, 0, 1, 0, 65, 0, 1, 0, 1, 0, 128, 255, 255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255];
        let size = dispatcher.dispatch_one(&request_buf, &mut response, &abort).await;
        assert_eq!(size, 42);
        assert_eq!(&response[..size], &[20u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn unrecognized_sequence() {
        let data_vec = [0u8;40];
        let reader = BitReadStream::new(BitReadBuffer::new(&data_vec, LittleEndian)).read_bits(data_vec.len() * 8).unwrap();
        let mut writer_vec = Vec::<u8>::new();
        let writer = BitWriteStream::new(&mut writer_vec, LittleEndian);

        let port = TestAccessPort::new(reader, writer);

        let reactor = FakeReactor {};
        let abort = AbortToken::new();
        let mut dispatcher = Dispatcher::<'_, _, _, ()>::new(port, &reactor, None);

        let mut response = [0u8; 64];
        let size = dispatcher.dispatch_one(&[0xE0], &mut response, &abort).await;
        assert_eq!(size, 5);
        assert_eq!(&response[..5], "unsup".as_bytes());
    }
}