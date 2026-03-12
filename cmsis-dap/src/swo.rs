// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::MAX_PACKET_SIZE;
use crate::SWOPort;
use crate::types::{SWOMode, SWOStatus, SWOTransport};
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_usb::driver::EndpointIn as USBEndpointIn;

const TRACE_BLOCK_SIZE: usize = 64;

pub struct SWO<'a, P: SWOPort, M: RawMutex, const N: usize> {
    control: embassy_sync::watch::Watch<M, SWOTaskControl, 1>,
    status: embassy_sync::watch::Watch<M, SWOStatus, 2>,
    transport: embassy_sync::watch::Watch<M, SWOTransport, 1>,
    buffer_consumer: embassy_sync::mutex::Mutex<M, bbqueue::Consumer<'a, N>>,
    buffer_producer: Option<bbqueue::Producer<'a, N>>,
    _port: core::marker::PhantomData<P>,
}

impl<'a, P: SWOPort, M: RawMutex, const N: usize> SWO<'a, P, M, N> {
    pub fn new(buffer: &'a bbqueue::BBBuffer<N>) -> Self {
        let (producer, consumer) = buffer.try_split().expect("buffer couldn't be split");
        let control = embassy_sync::watch::Watch::new();
        control.sender().send(SWOTaskControl::default());
        let status = embassy_sync::watch::Watch::new();
        status.sender().send(SWOStatus::default());
        let transport = embassy_sync::watch::Watch::new();
        transport.sender().send(SWOTransport::None);
        Self {
            control,
            status,
            transport,
            buffer_consumer: embassy_sync::mutex::Mutex::new(consumer),
            buffer_producer: Some(producer),
            _port: core::marker::PhantomData,
        }
    }

    pub fn try_split<R: crate::Reactor>(&'a mut self, port: P, reactor: &'a R) -> Option<(SWOAccess<'a, P, M, N>, SWOTask<'a, P, R, N>)> {
        let buffer = self.buffer_producer.take()?;
        let task_status_receiver = self.status.dyn_receiver()?;
        let control = self.control.dyn_receiver()?;

        Some((
            SWOAccess {
                parent: self,
            },
            SWOTask {
                port,
                control,
                status_sender: self.status.dyn_sender(),
                status_receiver: task_status_receiver,
                reactor,
                buffer,
            },
        ))
    }
}

pub trait SWOBulkTaskBehavior {
    fn run(&mut self) -> impl core::future::Future<Output = ()>;
}

impl SWOBulkTaskBehavior for () {
    async fn run(&mut self) -> () {
        unimplemented!();
    }
}

pub trait SWOAccessBehavior {
    type BulkTask<E: USBEndpointIn>: SWOBulkTaskBehavior;

    const SUPPORTS_UART: bool;
    const SUPPORTS_MANCHESTER: bool;
    const BUFFER_SIZE: usize;

    fn set_transport(&mut self, transport: SWOTransport) -> bool;

    fn set_mode(&mut self, mode: SWOMode) -> bool;

    fn set_baudrate(&mut self, baudrate: u32) -> bool;

    fn set_active(&mut self, active: bool);

    fn get_status(&mut self) -> SWOStatus;

    fn read_trace_data(&mut self, data: &mut [u8]) -> Option<usize>;

    fn try_make_bulk_task<E: USBEndpointIn>(&self, endpoint: E) -> Option<Self::BulkTask<E>>;
}

impl SWOAccessBehavior for () {
    type BulkTask<E: USBEndpointIn> = ();

    const SUPPORTS_UART: bool = false;
    const SUPPORTS_MANCHESTER: bool = false;
    const BUFFER_SIZE: usize = 0;

    fn set_transport(&mut self, _transport: SWOTransport) -> bool {
        unimplemented!();
    }

    fn set_mode(&mut self, _mode: SWOMode) -> bool {
        unimplemented!();
    }

    fn set_baudrate(&mut self, _baudrate: u32) -> bool {
        unimplemented!();
    }

    fn set_active(&mut self, _active: bool) {
        unimplemented!();
    }

    fn get_status(&mut self) -> SWOStatus {
        unimplemented!();
    }

    fn read_trace_data(&mut self, _data: &mut [u8]) -> Option<usize> {
        unimplemented!();
    }

    fn try_make_bulk_task<E: USBEndpointIn>(&self, _endpoint: E) -> Option<Self::BulkTask<E>> {
        unimplemented!();
    }
}

pub struct SWOAccess<'a, P: SWOPort, M: RawMutex, const N: usize> {
    parent: &'a SWO<'a, P, M, N>,
}

impl<'a, P: SWOPort, M: RawMutex, const N: usize> SWOAccessBehavior for SWOAccess<'a, P, M, N> {
    type BulkTask<E: USBEndpointIn> = SWOBulkTask<'a, M, E, N>;

    const SUPPORTS_UART: bool = P::SUPPORTS_UART;
    const SUPPORTS_MANCHESTER: bool = P::SUPPORTS_MANCHESTER;
    const BUFFER_SIZE: usize = N;

    fn set_transport(&mut self, transport: SWOTransport) -> bool {
        defmt::info!("swo transport {}", transport as u8);
        self.parent.transport.sender().send(transport);
        true
    }

    fn set_mode(&mut self, mode: SWOMode) -> bool {
        if mode == SWOMode::Uart && !P::SUPPORTS_UART {
            return false;
        } else if mode == SWOMode::Manchester && !P::SUPPORTS_MANCHESTER {
            return false;
        }

        self.parent.control.sender().send_if_modified(|control| {
            let control = control.as_mut().expect("should have a control value");
            let changed = control.mode != mode;
            control.mode = mode;
            changed
        });
        true
    }

    fn set_baudrate(&mut self, baudrate: u32) -> bool {
        self.parent.control.sender().send_if_modified(|control| {
            let control = control.as_mut().expect("should have a control value");
            let changed = control.baudrate != baudrate;
            control.baudrate = baudrate;
            changed
        });
        true
    }

    fn set_active(&mut self, active: bool) {
        self.parent.control.sender().send_if_modified(|control| {
            let control = control.as_mut().expect("should have a control value");
            let changed = control.active != active;
            control.active = active;
            changed
        });
    }

    fn get_status(&mut self) -> SWOStatus {
        let status = self.parent.status.try_get().unwrap_or(SWOStatus::default());
        status
    }

    fn read_trace_data(&mut self, data: &mut [u8]) -> Option<usize> {
        if self.parent.transport.try_get() != Some(SWOTransport::SWOData) {
            return None;
        }
        let mut buffer = self.parent.buffer_consumer.try_lock().ok()?;

        let grant = buffer.split_read().ok()?;
        let first_len = grant.bufs().0.len().min(data.len());
        let second_len = grant.bufs().1.len().min(data.len() - first_len);
        data[..first_len].copy_from_slice(&grant.bufs().0[..first_len]);
        data[first_len..first_len+second_len].copy_from_slice(&grant.bufs().1[..second_len]);
        grant.release(first_len + second_len);
        self.parent.status.sender().send_modify(|status| {
            let status = status.as_mut().expect("should have a status");
            status.buffered_traces -= (first_len + second_len) as u32;
        });
        Some(first_len + second_len)
    }

    fn try_make_bulk_task<E: USBEndpointIn>(&self, endpoint: E) -> Option<SWOBulkTask<'a, M, E, N>> {
        let transport = self.parent.transport.dyn_receiver()?;
        let status_receiver = self.parent.status.dyn_receiver()?;
        Some(SWOBulkTask {
            transport,
            endpoint,
            status_sender: self.parent.status.dyn_sender(),
            status_receiver,
            buffer: &self.parent.buffer_consumer,
        })
    }
}

pub struct SWOBulkTask<'a, M: RawMutex, E: USBEndpointIn, const N: usize> {
    transport: embassy_sync::watch::DynReceiver<'a, SWOTransport>,
    endpoint: E,
    status_sender: embassy_sync::watch::DynSender<'a, SWOStatus>,
    status_receiver: embassy_sync::watch::DynReceiver<'a, SWOStatus>,
    buffer: &'a embassy_sync::mutex::Mutex<M, bbqueue::Consumer<'a, N>>,
}

impl<'a, M: RawMutex, E: USBEndpointIn, const N: usize> SWOBulkTaskBehavior for SWOBulkTask<'a, M, E, N> {
    async fn run(&mut self) {
        let mut transport = self.transport.get().await;
        loop {
            let transport_fut = self.transport.changed();
            if transport == SWOTransport::USBBulkEndpoint {
                let transfer_fut = async {
                    self.endpoint.wait_enabled().await;
                    let mut buffer = self.buffer.lock().await;
                    buffer.split_read().and_then(|grant| {
                        let len = grant.combined_len();
                        grant.release(len);
                        Ok(())
                    }).ok();
                    loop {
                        if let Ok(grant) = buffer.read() {
                            let used = grant.len().min(MAX_PACKET_SIZE);
                            if used > 0 {
                                if !self.endpoint.write(&grant.buf()[..used]).await.is_ok() {
                                    break;
                                }
                                grant.release(used);
                                self.status_sender.send_modify(|status| {
                                    let status = status.as_mut().expect("should have a status");
                                    status.buffered_traces -= used as u32;
                                });
                            } else {
                                self.status_receiver.changed().await;
                            }
                        } else {
                            self.status_receiver.changed().await;
                        }
                    }
                };
                let result = embassy_futures::select::select(transport_fut, transfer_fut).await;
                if let embassy_futures::select::Either::First(next_transport) = result {
                    transport = next_transport;
                }
            } else {
                transport = transport_fut.await;
            }
        }
    }
}

#[derive(Clone, Default)]
struct SWOTaskControl {
    mode: SWOMode,
    baudrate: u32,
    active: bool,
}

pub struct SWOTask<'a, P: SWOPort, R: crate::Reactor, const N: usize> {
    port: P,
    control: embassy_sync::watch::DynReceiver<'a, SWOTaskControl>,
    status_sender: embassy_sync::watch::DynSender<'a, SWOStatus>,
    status_receiver: embassy_sync::watch::DynReceiver<'a, SWOStatus>,
    buffer: bbqueue::Producer<'a, N>,
    reactor: &'a R,
}

impl<'a, P: SWOPort, R: crate::Reactor, const N: usize> SWOTask<'a, P, R, N> {
    pub async fn run(&mut self) {
        let mut control = self.control.get().await;

        loop {
            self.status_sender.send_modify(|status| {
                let status = status.as_mut().expect("should have a status");
                status.flags.set_active(control.active && control.mode != SWOMode::Off);
            });

            let mut new_control: Option<SWOTaskControl> = None;
            let control_fut = self.control.changed();
            if control.active && control.mode != SWOMode::Off {
                let data_fut = async {
                    loop {
                        if let Ok(mut grant) = self.buffer.grant_exact(TRACE_BLOCK_SIZE) {
                            let bytes_used = self.port.read_trace_data(&mut grant).await;
                            if bytes_used > 0 {
                                self.reactor.activity();
                                grant.commit(bytes_used);
                                self.status_sender.send_modify(|status| {
                                    let status = status.as_mut().expect("should have a status");
                                    status.flags.set_buffer_overrun(false);
                                    status.buffered_traces += bytes_used as u32;
                                    status.next_trace_index = status.next_trace_index.wrapping_add(bytes_used as u32);
                                    status.next_trace_timestamp = embassy_time::Instant::now().as_ticks() as u32;
                                });
                            }
                        } else {
                            defmt::warn!("swo buffer overrun detected");
                            self.status_sender.send_if_modified(|status| {
                                let status = status.as_mut().expect("status should already exist");
                                let changed = !status.flags.is_buffer_overrun();
                                status.flags.set_buffer_overrun(true);
                                changed
                            });
                            self.status_receiver.changed().await;
                        }
                    }
                };

                if let embassy_futures::select::Either::First(control) =
                embassy_futures::select::select(control_fut, data_fut).await {
                    new_control = Some(control);
                }
            } else {
                new_control = Some(control_fut.await);
            }

            if let Some(new_control) = new_control {
                let control_applied = match (new_control.active, new_control.mode) {
                    (true, SWOMode::Uart) => self.port.set_mode_uart(new_control.baudrate),
                    (true, SWOMode::Manchester) => self.port.set_mode_manchester(new_control.baudrate),
                    (false, _) | (true, SWOMode::Off) =>  {
                        self.port.close();
                        true
                    },
                };
                
                if control_applied {
                    defmt::info!("swo new control; active: {}, mode: {}, baud: {}", new_control.active, new_control.mode as u8, new_control.baudrate);
                    control = new_control;
                }
            }
        }
    }
}