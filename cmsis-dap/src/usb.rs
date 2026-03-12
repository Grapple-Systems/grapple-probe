// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::dispatcher::{Dispatcher, Responder};
use crate::{MAX_PACKET_SIZE, JTAGAccessPort, PacketBuffer, SWDAccessPort, SWJAccessPort};
use crate::packet_buffer::PacketBufferProducer;
use crate::swo::{SWOAccessBehavior, SWOBulkTaskBehavior};
use crate::types::AbortToken;
use embassy_usb::driver::{Endpoint, EndpointIn, EndpointOut};

pub struct CMSISDapClass<'a, D: embassy_usb::driver::Driver<'a>, P: SWJAccessPort + SWDAccessPort, R: crate::Reactor, SWO: SWOAccessBehavior> {
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
    dispatcher: Dispatcher<'a, P, R, SWO>,
    swo_bulk_task: Option<SWO::BulkTask<D::EndpointIn>>,
}

impl<'a, D: embassy_usb::driver::Driver<'a>, P: JTAGAccessPort + SWJAccessPort + SWDAccessPort, R: crate::Reactor, SWO: SWOAccessBehavior> CMSISDapClass<'a, D, P, R, SWO> {
    pub fn new(builder: &mut embassy_usb::Builder<'a, D>, port: P, state: &'a mut State, reactor: &'a R, swo: Option<SWO>) -> Self {
        let mut function = builder.function(0xFF, 0, 0);
        let mut interface = function.interface();
        let iface_string_id = interface.string();
        let mut alt = interface.alt_setting(0xFF, 0, 0, Some(iface_string_id));
        let read_ep = alt.endpoint_bulk_out(None, MAX_PACKET_SIZE as u16);
        let write_ep = alt.endpoint_bulk_in(None, MAX_PACKET_SIZE as u16);
        let swo_bulk_task = swo.as_ref().map(|swo| {
            let endpoint = alt.endpoint_bulk_in(None, MAX_PACKET_SIZE as u16);
            swo.try_make_bulk_task(endpoint).expect("couldn't make the swo bulk task")
        });
        drop(function);

        let control = state.control.write(Control { iface_string_id });
        builder.handler(control);

        Self { read_ep, write_ep, dispatcher: Dispatcher::new(port, reactor, swo), swo_bulk_task }
    }

    pub async fn run<const SIZE: usize>(&mut self, buffer: &PacketBuffer<SIZE>) {
        let (prod, cons) = buffer.try_split().unwrap();

        let mut reader = Reader::<'_, '_, 'a, D, SIZE> {
            endpoint: &mut self.read_ep,
            buffer_prod: prod,
        };
        let writer = Writer::<'_, 'a, D> {
            endpoint: &mut self.write_ep,
        };
        let abort = AbortToken::new();

        let read_fut = reader.read_all(&abort);
        let disp_fut = self.dispatcher.dispatch_all(cons, writer, &abort);
        let swo_fut = async { if let Some(task) = self.swo_bulk_task.as_mut() { task.run().await } };

        embassy_futures::join::join3(read_fut, disp_fut, swo_fut).await;
    }
}

struct Reader<'a, 'b, 'd, D: embassy_usb::driver::Driver<'d>, const S: usize> {
    endpoint: &'a mut D::EndpointOut,
    buffer_prod: PacketBufferProducer<'b, S>,
}

impl<'a, 'b, 'd, D: embassy_usb::driver::Driver<'d>, const S: usize> Reader<'a, 'b, 'd, D, S> {
    async fn read_all(&mut self, abort_token: &AbortToken) {
        loop {
            self.endpoint.wait_enabled().await;
            defmt::info!("dap connected");

            loop {
                let mut grant = self.buffer_prod.grant().await;
                if let Ok(packet_size) = self.endpoint.read(grant.buf()).await {
                    if packet_size == 1 && grant.buf()[0] == 0x07 {
                        defmt::info!("received transfer abort command");
                        abort_token.abort();
                    } else {
                        defmt::trace!("dap request: {}", &grant.buf()[..packet_size]);
                        grant.commit(packet_size);
                    }
                } else {
                    break;
                }
            }

            defmt::info!("dap disconnected");
        }
    }
}

struct Writer<'a, 'b, D: embassy_usb::driver::Driver<'b>> {
    endpoint: &'a mut D::EndpointIn
}

impl<'a, 'd, D: embassy_usb::driver::Driver<'d>> Responder for Writer<'a, 'd, D> {
    async fn write_packet(&mut self, packet: &[u8]) {
        defmt::trace!("dap response: {}", packet);
        self.endpoint.write(packet).await.ok();
    }
}

pub struct State {
    control: core::mem::MaybeUninit<Control>,
}

impl State {
    pub fn new() -> Self {
        Self { control: core::mem::MaybeUninit::uninit() }
    }
}

struct Control {
    iface_string_id: embassy_usb::types::StringIndex,
}

impl embassy_usb::Handler for Control {
    fn get_string(&mut self, index: embassy_usb::types::StringIndex, _lang_id: u16) -> Option<&str> {
        if index == self.iface_string_id {
            Some("CMSIS-DAP v2 Interface")
        } else {
            None
        }
    }
}