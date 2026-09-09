// Copyright (c) 2025-2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![no_main]

use cmsis_dap::PacketBuffer;
use embedded_io_async::{Read, Write};
use grapple_probe_proto::{field, proto};

#[derive(Clone)]
struct State {
    host_status: cmsis_dap::HostStatus,
    gnd_detect: bool,
    trans_mv: u32,
    tvcc_mv: u32,
    uart_packets: u32,
    debug_commands: u32,
    i2c_commands: u32,
    power_config: field::OwnedPowerControl,
}

impl State {
    pub const fn new() -> Self {
        Self {
            host_status: cmsis_dap::HostStatus::Connected(false),
            gnd_detect: false,
            trans_mv: 0,
            tvcc_mv: 0,
            uart_packets: 0,
            debug_commands: 0,
            i2c_commands: 0,
            power_config: field::OwnedPowerControl::default(),
        }
    }
}

type InterTaskMutex = embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
static STATE: embassy_sync::watch::Watch<InterTaskMutex, State, 1> = embassy_sync::watch::Watch::new_with(State::new());

static STORAGE: embassy_sync::mutex::Mutex<InterTaskMutex, Option<grapple_probe::Storage<'static>>> = 
    embassy_sync::mutex::Mutex::new(None);

const BUFFER_SIZE: usize = 8*1024;
static PACKET_BUFFER: PacketBuffer<BUFFER_SIZE> = PacketBuffer::new();

static FIRMWARE_VERSION: &str = core::env!("CARGO_PKG_VERSION");

struct CMSISDapReactor {}

impl CMSISDapReactor {
    fn get_status(&self, req: &proto::GetStatusRequest<&[u8]>, res: &mut [u8]) -> usize {
        let mut res = req.try_alloc_response(res).expect("response buffer not big enough");
        let status = STATE.try_get().expect("state is not set");
        let mut flags = proto::StatusFlags::default();
        flags.set_gnd_detect(status.gnd_detect);
        res.set_flags(flags.inner);
        res.set_target_voltage(status.tvcc_mv as u16);
        res.set_signal_voltage(status.trans_mv as u16);
        res.commit()
    }

    async fn read_config(&self, req: &proto::ReadFieldRequest<&[u8]>, res: &mut [u8]) -> usize {
        let mut res = req.try_alloc_response(res).expect("response buffer not big enough");
        let mut storage_guard = STORAGE.lock().await;
        let storage = storage_guard.as_mut().expect("storage is none");
        if let Ok(value) = storage.read(req.get_field_id()).await {
            let len = value.len().min(res.get_data_mut().len());
            if len < value.len() {
                defmt::warn!("read a field that can't fit in a packet");
            }
            res.set_status(0);
            res.set_field_id(req.get_field_id());
            res.get_data_mut()[..len].copy_from_slice(&value[..len]);
            res.commit(len)
        } else {
            res.set_status(0xFF);
            res.commit(0)
        }
    }

    async fn write_config(&self, req: &proto::WriteFieldRequest<&[u8]>, res: &mut [u8]) -> usize {
        use field::Field;

        let mut res = req.try_alloc_response(res).expect("response buffer not big enough");
        let field_stored = match req.get_field_id() {
            field::OwnedPowerControl::ID => {
                if let Ok(field) = field::PowerControl::try_parse(req.get_data()) {
                    Self::store_power_control(field).await
                } else {
                    false
                }
            },
            _ => false,
        };
        if field_stored {
            res.set_status(0);
            res.set_field_id(req.get_field_id());
        } else {
            res.set_status(0xFF);
        }
        res.commit()
    }

    fn reset(&self, req: &proto::ResetRequest<&[u8]>) -> usize {
        match proto::ResetType::try_from(req.get_reset_type()) {
            Ok(proto::ResetType::Panic) => panic!("intentional crash!!!"),
            Err(e) => defmt::warn!("invalid reset type: {}", e.number),
        };
        0
    }

    async fn store_field(id: u8, data: &[u8]) -> bool {
        let mut storage_guard = STORAGE.lock().await;
        let storage = storage_guard.as_mut().expect("storage is none");
        storage.write(id, data).await.is_ok()
    }

    async fn store_power_control(field: field::PowerControl<&[u8]>) -> bool {
        use field::Field;
        if Self::store_field(field::OwnedPowerControl::ID, field.data()).await {
            STATE.sender().send_modify(|state| {
                let state = state.as_mut().expect("state doesn't exist");
                state.power_config.clone_from(&field);
            });
            true
        } else {
            false
        }
    }
}

impl cmsis_dap::Reactor for CMSISDapReactor {
    fn get_firmware_version(&self) -> Option<&str> {
        Some(FIRMWARE_VERSION)
    }

    async fn host_status(&self, status: cmsis_dap::HostStatus) {
        STATE.sender().send_modify(|state| {
            if let Some(state) = state.as_mut() {
                state.host_status = status.clone();
            }
        });
    }

    async fn unrecognized_packet(&self, command: &[u8], response: &mut [u8]) -> usize {
        match proto::Packet::try_parse_request(command) {
            Ok(proto::Packet::GetStatusRequest(req)) => self.get_status(&req, response),
            Ok(proto::Packet::ReadFieldRequest(req)) => self.read_config(&req, response).await,
            Ok(proto::Packet::WriteFieldRequest(req)) => self.write_config(&req, response).await,
            Ok(proto::Packet::ResetRequest(req)) => self.reset(&req),
            _ => {
                defmt::warn!("unsupported packet");
                response[0] = 0xFF;
                1
            },
        }
    }

    fn activity(&self) {
        STATE.sender().send_modify(|state| {
            if let Some(state) = state.as_mut() {
                state.debug_commands = state.debug_commands.wrapping_add(1);
            }
        });
    }
}

struct WrappedI2C<'a, D: embassy_rp::i2c::Instance> {
    inner: usb_i2c::rpi::I2CDevice<'a, D>,
    state: embassy_sync::watch::DynSender<'a, State>,
}

impl<'a, D: embassy_rp::i2c::Instance> usb_i2c::I2CDevice for WrappedI2C<'a, D> {
    fn configure(&mut self, freq_hz: u32) -> Result<(), Self::Error> {
        self.inner.configure(freq_hz)
    }
}

impl<'a, D: embassy_rp::i2c::Instance> embedded_hal_async::i2c::ErrorType for WrappedI2C<'a, D> {
    type Error = <usb_i2c::rpi::I2CDevice<'a, D> as embedded_hal_async::i2c::ErrorType>::Error;
}

impl<'a, D> embedded_hal_async::i2c::I2c<embedded_hal_async::i2c::SevenBitAddress> for WrappedI2C<'a, D> where 
D: embassy_rp::i2c::Instance
{
    async fn transaction(
            &mut self,
            address: embedded_hal_async::i2c::SevenBitAddress,
            operations: &mut [embedded_hal_async::i2c::Operation<'_>],
        ) -> Result<(), Self::Error> {
        
        self.state.send_modify(|s|{
            let state = s.as_mut().expect("no state");
            state.i2c_commands = state.i2c_commands.wrapping_add(1);
        });
        self.inner.transaction(address, operations).await
    }
}

const SWO_BUFFER_SIZE: usize = 256;
type Mutex = embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    use embassy_usb::msos;
    use field::Field;

    let mut board = grapple_probe::Board::open();

    //spawner.spawn(watchdog_task(board.take_watchdog()).expect("failed to spawn watchdog task"));

    let mut storage = board.take_storage();
    let maybe_power_config = storage.read(field::OwnedPowerControl::ID).await.ok().
        and_then(|data| field::PowerControl::try_parse(data).ok());
    if let Some(power_config) = maybe_power_config {
        STATE.sender().send_modify(|field| {
            let field = field.as_mut().expect("no state!");
            field.power_config.clone_from(&power_config);
        });
    }
    
    _ = STORAGE.try_lock().expect("storage locked on boot").insert(storage);

    let target_power = board.take_target_power();

    let debug_access_port = board.take_access_port();
    let uart = board.take_uart();
    let usb_device = board.take_usb();

    let mut usb_config = embassy_usb::Config::new(0x1209, 0x4853);
    usb_config.manufacturer = Some("Grapple Systems");
    usb_config.product = Some("Grapple Probe (CMSIS-DAP v2)");
    usb_config.serial_number = Some("0123456789");
    usb_config.max_power = 500;
    usb_config.max_packet_size_0 = 64;

    if let Ok(device_info) = board.get_device_info() {
        for field in device_info.get_fields() {
            match field {
                device_info::DeviceInfoField::DeviceId(device_id) => {
                    usb_config.serial_number = device_id.try_get()
                }
            }
        }
    }

    defmt::info!("serial number: {:?}", usb_config.serial_number);
    defmt::info!("firmeare version: {:?}", FIRMWARE_VERSION);

    // // Required for windows compatibility.
    // // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    usb_config.device_class = 0xEF;
    usb_config.device_sub_class = 0x02;
    usb_config.device_protocol = 0x01;
    usb_config.composite_with_iads = true;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut msos_descriptor = [0; 256];
    let mut control_buf = [0; 256];

    let reactor = CMSISDapReactor{};
    let mut cmsis_dap_state = cmsis_dap::State::new();
    let mut vcom_state = embassy_usb::class::cdc_acm::State::new();
    let mut i2c_usb = usb_i2c::I2CClass::new();

    let mut usb_builder = embassy_usb::Builder::new(
        usb_device,
        usb_config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    usb_builder.msos_descriptor(msos::windows_version::WIN8_1, 0);
    usb_builder.msos_feature(msos::CompatibleIdFeatureDescriptor::new("WINUSB", ""));
    let guid_data = msos::PropertyData::RegMultiSz(&["{CDB3B5AD-293B-4663-AA36-1AAE46463776}"]);
    usb_builder.msos_feature(msos::RegistryPropertyFeatureDescriptor::new("DeviceInterfaceGUIDs", guid_data));

    let access_port = grapple_probe::PioAccessPort::new(&debug_access_port);
    let swo_port = grapple_probe::PioSWO::new(&debug_access_port);

    let mut swo_buffer = bbqueue::BBBuffer::<SWO_BUFFER_SIZE>::new();

    let mut swo = cmsis_dap::SWO::<'_, _, Mutex, SWO_BUFFER_SIZE>::new(&mut swo_buffer);
    let (swo_access, mut swo_task) = swo.try_split(swo_port, &reactor).expect("couldn't split swo");

    let mut cmsis_dap = cmsis_dap::CMSISDapClass::new(&mut usb_builder, access_port, &mut cmsis_dap_state, &reactor, Some(swo_access));
    let vcom = embassy_usb::class::cdc_acm::CdcAcmClass::new(&mut usb_builder, &mut vcom_state, 64);
    let i2c = WrappedI2C { inner: board.take_i2c(), state: STATE.dyn_sender() };
    let mut i2c = i2c_usb.build(i2c, &mut usb_builder);
    let mut usb = usb_builder.build();

    let usb_fut = usb.run();
    let cmsis_dap_fut = cmsis_dap.run(&PACKET_BUFFER);
    let swo_task_fut = swo_task.run();

    let vcom_fut = usb_uart(vcom, uart);
    let i2c_fut = i2c.run();

    spawner.spawn(ui_task(board.take_led()).expect("couldn't make ui task"));

    let power_task_config = PowerTaskConfig {
        adc: board.take_adc(),
        gnd_detect: board.take_ground_detect(),
        vreg: target_power,
        key5v: board.take_key5v(),
    };
    spawner.spawn(power_task(power_task_config).expect("couldn't make power task"));

    embassy_futures::join::join5(usb_fut, cmsis_dap_fut, vcom_fut, swo_task_fut, i2c_fut).await;
}

enum USBUartCommand {
    Write(usize),
    LineEncoding,
    Reconnect,
    Retry,
}

async fn usb_uart<'a>(mut vcom: embassy_usb::class::cdc_acm::CdcAcmClass<'a, grapple_probe::USBDriver<'a>>, almost_uart: grapple_probe::AlmostUart) {
    let mut uart_tx_buffer = [0u8; 256];
    let mut uart_rx_buffer = [0u8; 256];
    let mut uart = almost_uart.into_buffered(&mut uart_tx_buffer, &mut uart_rx_buffer);
    let state = STATE.sender();

    vcom.wait_connection().await;
    let (mut usb_sender, mut usb_receiver, control_changed) = vcom.split_with_control();
    let mut rx_buffer = [0u8; 64];
    let mut tx_buffer = [0u8; 64];
    let mut line_coding = usb_receiver.line_coding();
    loop {
        let usb_fut = usb_receiver.read_packet(&mut rx_buffer);
        let uart_fut = uart.read(&mut tx_buffer);
        let control_fut = control_changed.control_changed();

        let command = match embassy_futures::select::select3(usb_fut, uart_fut, control_fut).await {
            embassy_futures::select::Either3::First(usb_res) => {
                state.send_modify(|state| {
                    if let Some(state) = state.as_mut() {
                        state.uart_packets = state.uart_packets.wrapping_add(1);
                    }
                });
                usb_res.map(|num_bytes| USBUartCommand::Write(num_bytes)).unwrap_or(USBUartCommand::Reconnect)
            },
            embassy_futures::select::Either3::Second(uart_res) => {
                if let Ok(num_bytes) = uart_res {
                    state.send_modify(|state| {
                        if let Some(state) = state.as_mut() {
                            state.uart_packets = state.uart_packets.wrapping_add(1);
                        }
                    });
                    usb_sender.write_packet(&tx_buffer[..num_bytes]).await.ok();
                } else {
                    defmt::warn!("uart read error");
                }
                USBUartCommand::Retry
            },
            embassy_futures::select::Either3::Third(()) => USBUartCommand::LineEncoding
        };

        match command {
            USBUartCommand::Write(num_bytes) => uart.write_all(&rx_buffer[..num_bytes]).await.expect("failed to write to uart"),
            USBUartCommand::LineEncoding => {
                if line_coding != usb_receiver.line_coding() {
                    line_coding = usb_receiver.line_coding();
                    defmt::info!("usb uart line coding change: {} baud, stop bits: {}, parity: {}", line_coding.data_rate(), line_coding.stop_bits(), line_coding.parity_type());
                    let mut config = embassy_rp::uart::Config::default();
                    config.baudrate = line_coding.data_rate();
                    config.parity = match line_coding.parity_type() {
                        embassy_usb::class::cdc_acm::ParityType::Even => embassy_rp::uart::Parity::ParityEven,
                        embassy_usb::class::cdc_acm::ParityType::Odd => embassy_rp::uart::Parity::ParityOdd,
                        _ => embassy_rp::uart::Parity::ParityNone,
                    };
                    config.stop_bits = match line_coding.stop_bits() {
                        embassy_usb::class::cdc_acm::StopBits::Two => embassy_rp::uart::StopBits::STOP2,
                        _ => embassy_rp::uart::StopBits::STOP1,
                    };
                    uart.set_config(config)
                }
            },
            USBUartCommand::Reconnect => usb_receiver.wait_connection().await,
            USBUartCommand::Retry => (),
        }
    }
}

#[embassy_executor::task]
async fn ui_task(mut led: grapple_probe::LED<'static>) {
    led.set(100, 0, 0);
    let mut led_blink = false;

    let mut state_receiver = STATE.anon_receiver();
    let mut last_state = state_receiver.try_get().expect("should have a state");

    let mut ticker = embassy_time::Ticker::every(embassy_time::Duration::from_millis(50));
    loop {
        let next_state = state_receiver.try_get().expect("should have a state");

        let (red, green, blue) = if next_state.trans_mv == 0 {
            (100, 0, 0)
        } else {
            let green = (next_state.trans_mv - 1800) * 100 / 1500;
            let blue = 100 - green;
            (0, green as u8, blue as u8)
        };

        defmt::trace!("led color: {}, {}, {}", red, green, blue);

        let activity = last_state.host_status != next_state.host_status ||
            last_state.debug_commands != next_state.debug_commands ||
            last_state.uart_packets != next_state.uart_packets ||
            last_state.i2c_commands != next_state.i2c_commands;

        if activity && !led_blink {
            led.set(0, 0, 0);
            led_blink = true;
        } else {
            led.set(red, green, blue);
            led_blink = false;
        }

        last_state = next_state;
        ticker.next().await;
    }
}

struct PowerTaskConfig<'a> {
    pub adc: grapple_probe::ADC<'a>,
    pub gnd_detect: embassy_rp::gpio::Input<'a>,
    pub vreg: grapple_probe::PWMVoltageRegulator<'a>,
    pub key5v: embassy_rp::gpio::Output<'a>,
}

#[embassy_executor::task]
async fn power_task(mut cfg: PowerTaskConfig<'static>) {
    static MIN_TVCC: u32 = 1600;

    let state_sender = STATE.sender();
    loop {
        let power_config = STATE.try_get().expect("no state").power_config;
        let gnddet = cfg.gnd_detect.is_low();

        if power_config.tvcc_output() {
            let tvcc_gated = power_config.tvcc_gated_by_gnddet() && !gnddet;
            let tvcc_mv = if tvcc_gated { 0 } else { power_config.tvcc_output_mv() as u32 };
            cfg.vreg.set_voltage_mv_b(tvcc_mv).await;
        } else {
            cfg.vreg.set_voltage_mv_b(0).await;
        }

        let sweep = cfg.adc.read().await;

        let trans_vcc_gated = 
            power_config.transvcc_gated_by_gnddet() && !gnddet ||
            power_config.transvcc_gated_by_tvcc() && sweep.tvcc_mv < MIN_TVCC;
        let trans_vcc = if trans_vcc_gated {
            0
        } else {
            if power_config.transvcc_follows_tvcc() && sweep.tvcc_mv >= MIN_TVCC {
                sweep.tvcc_mv
            } else {
                power_config.transvcc_default_mv() as u32
            }
        };
        let trans_vcc_actual_mv = cfg.vreg.set_voltage_mv_a(trans_vcc).await;

        cfg.key5v.set_level(power_config.key5v_enable().into());

        state_sender.send_modify(|state| {
            if let Some(state) = state.as_mut() {
                state.gnd_detect = gnddet;
                state.trans_mv = trans_vcc_actual_mv;
                state.tvcc_mv = sweep.tvcc_mv;
            }
        });
    }
}

#[embassy_executor::task]
async fn watchdog_task(mut watchdog: embassy_rp::watchdog::Watchdog) {
    let mut timer = embassy_time::Ticker::every(embassy_time::Duration::from_millis(100));
    loop {
        //timer.next().await;
        defmt::debug!("feed watchdog");
        watchdog.feed(embassy_time::Duration::from_millis(150));
        defmt::debug!("watchdog fed");
        embassy_time::Timer::after_millis(50).await;
    }
}