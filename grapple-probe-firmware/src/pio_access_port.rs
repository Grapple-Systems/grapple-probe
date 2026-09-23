// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: GPL-3.0-or-later

use cmsis_dap::{JTAGAccessPort, SWDAccessPort, SWJAccessPort, SWOPort};
use embassy_futures::block_on;
use embassy_rp::{gpio, pio, Peri, uart};
use embassy_rp::clocks::clk_sys_freq;
use embassy_rp::interrupt::typelevel::Binding;
use embassy_rp::pio::{InterruptHandler, PioPin};
use embassy_sync::mutex;

use core::cell::UnsafeCell;

type PioDivider = fixed::FixedU32::<fixed::types::extra::U8>;

type TokenMutex = embassy_sync::blocking_mutex::raw::NoopRawMutex;
type Token<'t> = mutex::MutexGuard<'t, TokenMutex, ()>;

fn get_pio_divider(min_decimation: u32, target_freq_hz: u32) -> PioDivider {
    use fixed::traits::ToFixed;

    let pio_freq_fixed = fixed::FixedU64::<fixed::types::extra::U8>::from_num(clk_sys_freq() as u64);
    let divider: PioDivider = (pio_freq_fixed / (target_freq_hz * min_decimation) as u64).to_fixed();
    defmt::debug!("pio divider: {} / {} -> {}", clk_sys_freq(), target_freq_hz, divider.to_bits());
    divider
}

pub struct PioAccessPortPeriphs<'a, APPio, SWO, Irqs, Clk, DioTms, SwoTdo, Tdi, TmsDir, Reset, TReset, const BUF_SIZE: usize> where
APPio: pio::Instance,
SWO: uart::Instance,
Irqs: Binding<APPio::Interrupt, pio::InterruptHandler<APPio>> +
    Binding<SWO::Interrupt, uart::BufferedInterruptHandler<SWO>> +
    Clone,
Clk: pio::PioPin,
DioTms: pio::PioPin,
TmsDir: pio::PioPin,
SwoTdo: pio::PioPin + uart::RxPin<SWO>,
Tdi: pio::PioPin + uart::TxPin<SWO>,
Reset: gpio::Pin,
TReset: gpio::Pin,
{
    pub swo_token: mutex::Mutex<TokenMutex, ()>,
    pub swd_token: mutex::Mutex<TokenMutex, ()>,
    pub pio_swd_jtag: Peri<'a, APPio>,
    pub swo: Peri<'a, SWO>,
    pub irqs: Irqs,
    pub swclk_tck: Peri<'a, Clk>,
    pub swdio_tms: Peri<'a, DioTms>,
    pub swo_tdo: Peri<'a, SwoTdo>,
    pub tdi: Peri<'a, Tdi>,
    pub swdio_tms_dir: Peri<'a, TmsDir>,
    pub resetn: Option<Peri<'a, Reset>>,
    pub tresetn: Option<Peri<'a, TReset>>,
    pub swo_buffer: UnsafeCell<[u8; BUF_SIZE]>,
}

impl<'a, APPio, SWO, Irqs, Clk, DioTms, SwoTdo, Tdi, TmsDir, Reset, TReset, const BUF_SIZE: usize> PioAccessPortOpener for
PioAccessPortPeriphs<'a, APPio, SWO, Irqs, Clk, DioTms, SwoTdo, Tdi, TmsDir, Reset, TReset, BUF_SIZE> where
APPio: pio::Instance,
SWO: uart::Instance,
Irqs: Binding<APPio::Interrupt, pio::InterruptHandler<APPio>> +
    Binding<SWO::Interrupt, uart::BufferedInterruptHandler<SWO>> +
    Clone,
Clk: pio::PioPin,
DioTms: pio::PioPin,
TmsDir: pio::PioPin,
SwoTdo: pio::PioPin + uart::RxPin<SWO>,
Tdi: pio::PioPin + uart::TxPin<SWO>,
Reset: gpio::Pin,
TReset: gpio::Pin,
{
    type SWDJTAG = APPio;
    type SWO = uart::BufferedUart;

    fn try_open_jtag(&self, freq_hz: u32) -> Option<JTAG<'_, APPio>> {
        if let (Ok(swd_token), Ok(swo_token)) = (self.swd_token.try_lock(), self.swo_token.try_lock()) {
            let mut jtag = unsafe {
                let resetn = self.resetn.as_ref().map(|pin| pin.clone_unchecked());
                let tresetn = self.tresetn.as_ref().map(|pin| pin.clone_unchecked());
                JTAG::new(
                    (swd_token, swo_token),
                    self.pio_swd_jtag.clone_unchecked(),
                    self.irqs.clone(),
                    self.swclk_tck.clone_unchecked(),
                    self.swdio_tms.clone_unchecked(),
                    self.tdi.clone_unchecked(),
                    self.swo_tdo.clone_unchecked(),
                    self.swdio_tms_dir.clone_unchecked(),
                    resetn, tresetn)
            };
            jtag.set_frequency(freq_hz);
            defmt::info!("pio-ap-opener: successfully opened jtag");
            Some(jtag)
        } else {
            defmt::warn!("pio-ap-opener: failed to open jtag");
            None
        }
    }

    fn try_open_swd(&self, freq_hz: u32) -> Option<SWD<'_, APPio>> {
        if let Ok(swd_token) = self.swd_token.try_lock() {
            let mut swd = unsafe {
                let resetn = self.resetn.as_ref().map(|pin| pin.clone_unchecked());
                SWD::new(
                    swd_token,
                    self.pio_swd_jtag.clone_unchecked(),
                    self.irqs.clone(),
                    self.swdio_tms.clone_unchecked(),
                    self.swclk_tck.clone_unchecked(),
                    self.swdio_tms_dir.clone_unchecked(),
                    resetn
                )
            };
            swd.set_frequency(freq_hz);
            defmt::info!("pio-ap-opener: successfully opened swd");
            Some(swd)
        } else {
            defmt::warn!("pio-ap-opener: failed to open swd");
            None
        }
    }

    fn try_open_swo(&self) -> Option<WrappedUartSWO<'_, uart::BufferedUart>> {
        if let Ok(swo_token) = self.swo_token.try_lock() {
            defmt::info!("pio-ap-opener: successfully opened swo");
            unsafe {
                // protected by the token
                let rx_buffer = &mut *self.swo_buffer.get();
                let config = uart::Config::default();
                Some(WrappedUartSWO {
                    _token: swo_token,
                    inner: uart::BufferedUart::new(
                        self.swo.clone_unchecked(),
                        self.tdi.clone_unchecked(),
                        self.swo_tdo.clone_unchecked(),
                        self.irqs.clone(),
                        &mut [], rx_buffer, config
                    )
                })
            }
        } else {
            defmt::warn!("pio-ap-opener: failed to open swo");
            None
        }
    }

    fn open_pins(&self) -> Pins<'_> {
        defmt::info!("pio-ap-opener: opened pins");
        let (swd_token, swo_token) = (self.swd_token.try_lock().ok(), self.swo_token.try_lock().ok());

        unsafe {
            let tck = swd_token.as_ref().map(|_| self.swclk_tck.clone_unchecked());
            let tms_dir = swd_token.as_ref().map(|_| self.swdio_tms_dir.clone_unchecked());
            let tms = swd_token.as_ref().map(|_| self.swdio_tms.clone_unchecked());
            let tdi = swo_token.as_ref().map(|_| self.tdi.clone_unchecked());
            let tdo = swo_token.as_ref().map(|_| self.swo_tdo.clone_unchecked());
            let resetn = swd_token.as_ref().and_then(|_| self.resetn.as_ref().map(|pin| pin.clone_unchecked()));
            let tresetn = swd_token.as_ref().and_then(|_| self.tresetn.as_ref().map(|pin| pin.clone_unchecked()));

            Pins::new((swd_token, swo_token), tck, tms_dir, tms, tdi, tdo, resetn, tresetn)
        }
    }
}

pub trait PioAccessPortOpener {
    type SWDJTAG: pio::Instance;
    type SWO: cmsis_dap::SWOPort;

    fn try_open_jtag(&self, freq_hz: u32) -> Option<JTAG<'_, Self::SWDJTAG>>;
    fn try_open_swd(&self, freq_hz: u32) -> Option<SWD<'_, Self::SWDJTAG>>;
    fn try_open_swo(&self) -> Option<WrappedUartSWO<'_, Self::SWO>>;
    fn open_pins(&self) -> Pins<'_>;
}

enum PioAccessPortType<'a, O: PioAccessPortOpener> {
    JTAG(JTAG<'a, O::SWDJTAG>),
    SWD(SWD<'a, O::SWDJTAG>),
    Pins(Pins<'a>),
}

enum SWDOrJTAG<'a, 'b, O: PioAccessPortOpener> {
    JTAG(&'a mut JTAG<'b, O::SWDJTAG>),
    SWD(&'a mut SWD<'b, O::SWDJTAG>),
}

pub struct PioAccessPort<'a, O: PioAccessPortOpener> {
    opener: &'a O,
    inner: PioAccessPortType<'a, O>,
    swj_freq_hz: u32,
}

impl<'a, O: PioAccessPortOpener> PioAccessPort<'a, O> {
    pub fn new(opener: &'a O) -> Self {
        let inner = PioAccessPortType::Pins(opener.open_pins());
        let swj_freq_hz = clk_sys_freq().min(1_000_000);
        Self { opener, inner, swj_freq_hz }
    }

    fn try_assert_jtag(&mut self) -> Option<&mut JTAG<'a, O::SWDJTAG>> {
        let open_jtag_or_pins = |inner| {
            match inner {
                PioAccessPortType::JTAG(jtag) => PioAccessPortType::JTAG(jtag),
                PioAccessPortType::SWD(swd) => {
                    drop(swd);
                    if let Some(jtag) = self.opener.try_open_jtag(self.swj_freq_hz) {
                        PioAccessPortType::JTAG(jtag)
                    } else {
                        PioAccessPortType::Pins(self.opener.open_pins())
                    }
                },
                PioAccessPortType::Pins(pins) => {
                    drop(pins);
                    if let Some(jtag) = self.opener.try_open_jtag(self.swj_freq_hz) {
                        PioAccessPortType::JTAG(jtag)
                    } else {
                        PioAccessPortType::Pins(self.opener.open_pins())
                    }
                }
            }
        };
        unsafe { replace_with::replace_with_or_abort_unchecked(&mut self.inner, open_jtag_or_pins) };

        match &mut self.inner {
            PioAccessPortType::JTAG(jtag) => Some(jtag),
            _ => None,
        }
    }

    fn assert_swd(&mut self) -> &mut SWD<'a, O::SWDJTAG> {
        let open_swd = |inner| {
            match inner {
                PioAccessPortType::JTAG(jtag) => {
                    drop(jtag);
                    PioAccessPortType::SWD(self.opener.try_open_swd(self.swj_freq_hz).
                        expect("should always be able to open swd"))
                },
                PioAccessPortType::SWD(swd) => PioAccessPortType::SWD(swd),
                PioAccessPortType::Pins(pins) => {
                    drop(pins);
                    PioAccessPortType::SWD(self.opener.try_open_swd(self.swj_freq_hz).
                        expect("should always be able to open swd"))
                }
            }
        };
        unsafe { replace_with::replace_with_or_abort_unchecked(&mut self.inner, open_swd) };

        match &mut self.inner {
            PioAccessPortType::SWD(swd) => swd,
            _ => panic!("couldn't assert swd"),
        }
    }

    fn assert_swj(&mut self) -> SWDOrJTAG<'_, 'a, O> {
        let open_swj = |inner| {
            match inner {
                PioAccessPortType::JTAG(jtag) => PioAccessPortType::JTAG(jtag),
                PioAccessPortType::SWD(swd) => PioAccessPortType::SWD(swd),
                PioAccessPortType::Pins(pins) => {
                    drop(pins);
                    PioAccessPortType::SWD(self.opener.try_open_swd(self.swj_freq_hz).
                        expect("should always be able to open swd"))
                }
            }
        };
        unsafe { replace_with::replace_with_or_abort_unchecked(&mut self.inner, open_swj) };

        match &mut self.inner {
            PioAccessPortType::JTAG(jtag) => SWDOrJTAG::JTAG(jtag),
            PioAccessPortType::SWD(swd) => SWDOrJTAG::SWD(swd),
            _ => panic!("couldn't assert swj"),
        }
    }

    fn assert_pins(&mut self) -> &mut Pins<'a> {
        let open_pins = |inner| {
            match inner {
                PioAccessPortType::JTAG(jtag) => {
                    drop(jtag);
                    PioAccessPortType::Pins(self.opener.open_pins())
                },
                PioAccessPortType::SWD(swd) => {
                    drop(swd);
                    PioAccessPortType::Pins(self.opener.open_pins())
                },
                PioAccessPortType::Pins(pins) => PioAccessPortType::Pins(pins),
            }
        };
        unsafe { replace_with::replace_with_or_abort_unchecked(&mut self.inner, open_pins) };

        match &mut self.inner {
            PioAccessPortType::Pins(pins) => pins,
            _ => panic!("should always be able to open pins"),
        }
    }
}

impl<'a, O: PioAccessPortOpener> SWJAccessPort for PioAccessPort<'a, O> {
    fn set_frequency(&mut self, freq_hz: u32) -> bool {
        if freq_hz <= clk_sys_freq() / 4 {
            self.swj_freq_hz = freq_hz;
            match &mut self.inner {
                PioAccessPortType::JTAG(jtag) => jtag.set_frequency(freq_hz),
                PioAccessPortType::SWD(swd) => swd.set_frequency(freq_hz),
                PioAccessPortType::Pins(_) => true,
            }
        } else {
            false
        }
    }

    fn pins(&mut self, out: cmsis_dap::Pins, mask: cmsis_dap::Pins, wait_us: u32) -> cmsis_dap::Pins {
        match &mut self.inner {
            PioAccessPortType::Pins(pins) => pins.swj_pins(out, mask, wait_us),
            _ => cmsis_dap::Pins(0)
        }
        
    }

    fn write_sequence(&mut self, num_bits: usize, data: &[u8]) -> bool {
        match self.assert_swj() {
            SWDOrJTAG::JTAG(jtag) => jtag.swj_sequence(num_bits as u32, data),
            SWDOrJTAG::SWD(swd) => swd.write_bits(num_bits as u32, data),
        }
        true
    }
}

impl<'a, O: PioAccessPortOpener> SWDAccessPort for PioAccessPort<'a, O> {
    const SUPPORTED: bool = true;

    fn open(&mut self) -> bool {
        self.assert_swd();
        true
    }

    fn close(&mut self) -> bool {
        self.assert_pins();
        true
    }

    #[inline(always)]
    fn read(&mut self, num_bits: usize, data: &mut [u8]) -> bool {
        if let PioAccessPortType::SWD(swd) = &mut self.inner {
            swd.read_bits(num_bits as u32, data);
            true
        } else {
            false
        }
    }

    #[inline(always)]
    fn write(&mut self, num_bits: usize, data: &[u8]) -> bool {
        if let PioAccessPortType::SWD(swd) = &mut self.inner {
            swd.write_bits(num_bits as u32, data);
            true
        } else {
            false
        }
    }
}

impl<'a, O: PioAccessPortOpener> JTAGAccessPort for PioAccessPort<'a, O> {
    const SUPPORTED: bool = true;

    fn open(&mut self) -> bool {
        self.try_assert_jtag().is_some()
    }

    fn close(&mut self) -> bool {
        self.assert_pins();
        true
    }

    fn clock(&mut self, cycles: usize, tms: bool, tdi: bool) -> bool {
        if let PioAccessPortType::JTAG(jtag) = &mut self.inner {
            let tdi = if tdi { [0xFF; 4] } else { [0; 4] };
            // transfer 32 bits at a time
            for i in (0..cycles).step_by(32) {
                let num_bits = (cycles - i).min(32) as u32;
                jtag.transfer(num_bits, tms, &tdi, &mut []);
            }
            true
        } else {
            false
        }
    }

    fn transfer(&mut self, num_bits: usize, tms: bool, tdi: &[u8], tdo: &mut [u8]) -> bool {
        if let PioAccessPortType::JTAG(jtag) = &mut self.inner {
            jtag.transfer(num_bits as u32, tms, tdi, tdo);
            true
        } else {
            false
        }
    }
}

pub enum PioSWOState<SWO: cmsis_dap::SWOPort> {
    SWO(SWO),
    Disabled,
}

pub struct PioSWO<'a, O: PioAccessPortOpener> {
    opener: &'a O,
    state: PioSWOState<WrappedUartSWO<'a, O::SWO>>
}

impl<'a, O: PioAccessPortOpener> PioSWO<'a, O> {
    pub fn new(opener: &'a O) -> Self {
        Self { opener, state: PioSWOState::Disabled }
    }
}

impl<'a, O: PioAccessPortOpener> SWOPort for PioSWO<'a, O> {
    const SUPPORTS_UART: bool = true;
    const SUPPORTS_MANCHESTER: bool = false;

    fn set_mode_uart(&mut self, baudrate: u32) -> bool {
        if matches!(self.state, PioSWOState::Disabled) {
            if let Some(swo) = self.opener.try_open_swo() {
                self.state = PioSWOState::SWO(swo);
            }
        }

        match &mut self.state {
            PioSWOState::SWO(swo) => {
                swo.set_mode_uart(baudrate)
            }
            PioSWOState::Disabled => false
        }
    }

    fn set_mode_manchester(&mut self, baudrate: u32) -> bool {
        if matches!(self.state, PioSWOState::Disabled) {
            if let Some(swo) = self.opener.try_open_swo() {
                self.state = PioSWOState::SWO(swo);
            }
        }

        match &mut self.state {
            PioSWOState::SWO(swo) => {
                swo.set_mode_manchester(baudrate)
            }
            PioSWOState::Disabled => false
        }
    }

    fn close(&mut self) {
        self.state = PioSWOState::Disabled;
    }

    async fn read_trace_data(&mut self, data: &mut [u8]) -> usize {
        match &mut self.state {
            PioSWOState::SWO(swo) => swo.read_trace_data(data).await,
            PioSWOState::Disabled => 0,
        }
    }
}

pub struct Pins<'a> {
    _tokens: (Option<Token<'a>>, Option<Token<'a>>),
    tck: Option<gpio::Output<'a>>,
    tms: Option<gpio::Output<'a>>,
    _tms_dir: Option<gpio::Output<'a>>,
    tdo: Option<gpio::Input<'a>>,
    tdi: Option<gpio::Output<'a>>,
    resetn: Option<gpio::OutputOpenDrain<'a>>,
    tresetn: Option<gpio::OutputOpenDrain<'a>>,
}

impl<'a> Pins<'a> {
    pub fn new(
        tokens: (Option<Token<'a>>, Option<Token<'a>>),
        tck: Option<Peri<'a, impl gpio::Pin + 'a>>,
        tms_dir: Option<Peri<'a, impl gpio::Pin + 'a>>,
        tms: Option<Peri<'a, impl gpio::Pin + 'a>>,
        tdi: Option<Peri<'a, impl gpio::Pin + 'a>>,
        tdo: Option<Peri<'a, impl gpio::Pin + 'a>>,
        resetn: Option<Peri<'a, impl gpio::Pin + 'a>>,
        tresetn: Option<Peri<'a, impl gpio::Pin + 'a>>,
    ) -> Self {
        Self {
            _tokens: tokens,
            tck: tck.map(|pin| gpio::Output::new(pin, true.into())),
            _tms_dir: tms_dir.map(|pin| gpio::Output::new(pin, true.into())),
            tms: tms.map(|pin| gpio::Output::new(pin, true.into())),
            tdo: tdo.map(|pin| gpio::Input::new(pin, gpio::Pull::None)),
            tdi: tdi.map(|pin| gpio::Output::new(pin, true.into())),
            resetn: resetn.map(|pin| gpio::OutputOpenDrain::new(pin, true.into())),
            tresetn: tresetn.map(|pin| gpio::OutputOpenDrain::new(pin, true.into())),
        }
    }

    pub fn swj_pins(&mut self, value: cmsis_dap::Pins, mask: cmsis_dap::Pins, wait_us: u32) -> cmsis_dap::Pins {
        // set the output pins
        if mask.get_swclk_tck() {
            self.tck.as_mut().map(|pin| pin.set_level(value.get_swclk_tck().into()));
        }
        if mask.get_swdio_tms() {
            self.tms.as_mut().map(|pin| pin.set_level(value.get_swdio_tms().into()));
        }
        if mask.get_tdi() {
            self.tdi.as_mut().map(|pin| pin.set_level(value.get_tdi().into()));
        }
        if mask.get_nreset() {
            self.resetn.as_mut().map(|pin| pin.set_level(value.get_nreset().into()));
        }
        if mask.get_ntrst() {
            self.tresetn.as_mut().map(|pin| pin.set_level(value.get_ntrst().into()));
        }

        // wait for open drain pins
        let wait_pins_fut = async {
            if let Some(pin) = self.resetn.as_mut() {
                if mask.get_nreset() && value.get_nreset() {
                    pin.wait_for_high().await;
                } else if mask.get_nreset() && !value.get_nreset() {
                    pin.wait_for_low().await;
                }
            }
            if let Some(pin) = self.tresetn.as_mut() {
                if mask.get_ntrst() && value.get_ntrst() {
                    pin.wait_for_high().await;
                } else if mask.get_ntrst() && !value.get_ntrst() {
                    pin.wait_for_low().await;
                }
            }
        };
        let timeout = embassy_time::Duration::from_micros(wait_us as u64);
        embassy_futures::block_on(embassy_time::with_timeout(timeout, wait_pins_fut)).ok();

        // read input pins
        let mut pins = value.clone();
        if mask.get_tdo() {
            self.tdo.as_ref().map(|pin| pins.set_tdo(pin.is_high()));
        }
        if mask.get_nreset() {
            self.resetn.as_ref().map(|pin| pins.set_nreset(pin.is_high()));
        }
        if mask.get_ntrst() {
            self.tresetn.as_ref().map(|pin| pins.set_ntrst(pin.is_high()));
        }
        pins
    }
}

pub struct JTAG<'a, PIO: pio::Instance> {
    _tokens: (Token<'a>, Token<'a>),
    pio: pio::Pio<'a, PIO>,
    _resetn: Option<gpio::OutputOpenDrain<'a>>,
    _tresetn: Option<gpio::OutputOpenDrain<'a>>,
    _idle_address: u8,
    jtag_address: u8,
    swj_address: u8,
    _tms_dir: gpio::Output<'a>,
}

impl<'a, PIO: pio::Instance> JTAG<'a, PIO> {
    pub fn new(
        tokens: (Token<'a>, Token<'a>),
        pio: Peri<'a, PIO>,
        irq: impl Binding<PIO::Interrupt, InterruptHandler<PIO>>,
        tck: Peri<'a, impl PioPin + 'a>,
        tms: Peri<'a, impl PioPin + 'a>,
        tdi: Peri<'a, impl PioPin + 'a>,
        tdo: Peri<'a, impl PioPin + 'a>,
        tms_dir: Peri<'a, impl PioPin + 'a>,
        resetn: Option<Peri<'a, impl gpio::Pin + 'a>>,
        tresetn: Option<Peri<'a, impl gpio::Pin + 'a>>) -> Self
    {
        let mut pio = pio::Pio::new(pio, irq);
        let tck_pin = pio.common.make_pio_pin(tck);
        let tms_pin = pio.common.make_pio_pin(tms);
        let tdi_pin = pio.common.make_pio_pin(tdi);
        let tdo_pin = pio.common.make_pio_pin(tdo);
        let tms_dir_pin = gpio::Output::new(tms_dir, true.into());
        let resetn_pin = resetn.map(|resetn| gpio::OutputOpenDrain::new(resetn, true.into()));
        let tresetn_pin = tresetn.map(|tresetn| gpio::OutputOpenDrain::new(tresetn, true.into()));

        let prog = pio::program::pio_file!("src/jtag.pio");
        let idle_address = prog.public_defines.idle as u8;
        let swj_address = prog.public_defines.swj_clock as u8;
        let jtag_address = prog.public_defines.jtag as u8;
        let prog = pio.common.load_program(&prog.program);

        let mut cfg = pio::Config::default();
        cfg.use_program(&prog, &[&tck_pin]);
        cfg.set_out_pins(&[&tdi_pin]);
        cfg.set_in_pins(&[&tdo_pin]);
        cfg.set_set_pins(&[&tms_pin]);
        cfg.shift_in.auto_fill = true;
        cfg.shift_out.auto_fill = true;

        pio.sm0.set_config(&cfg);
        pio.sm0.set_pins(gpio::Level::High, &[&tck_pin]);
        pio.sm0.set_pin_dirs(pio::Direction::In, &[&tdo_pin]);
        pio.sm0.set_pin_dirs(pio::Direction::Out, &[&tck_pin, &tms_pin, &tdi_pin]);
        unsafe { pio.sm0.exec_jmp(idle_address) };
        pio.sm0.set_enable(true);

        defmt::debug!("enabling jtag");

        Self {
            _tokens: tokens,
            pio,
            _resetn: resetn_pin,
            _tresetn: tresetn_pin,
            _idle_address: idle_address,
            jtag_address,
            swj_address,
            _tms_dir: tms_dir_pin,
        }
    }

    pub fn set_frequency(&mut self, freq_hz: u32) -> bool {
        if freq_hz <= clk_sys_freq() / 4 && freq_hz >= 2000 {
            let divider = get_pio_divider(4, freq_hz);
            self.pio.sm0.set_clock_divider(divider);
            true
        } else {
            false
        }
    }

    pub fn swj_sequence(&mut self, num_bits: u32, tms: &[u8]) {
        assert!(self.pio.sm0.rx().empty());
        assert!(self.pio.sm0.tx().empty());
        assert!(tms.len() == ((num_bits + 7) / 8) as usize);

        let (num_bits, num_bytes) = truncate(num_bits, tms.len());
        let data = &tms[..num_bytes];

        let tx = self.pio.sm0.tx();
        let command = (num_bits - 1) & 0x03FFFFFF | ((self.swj_address as u32) << 27);
        block_on(tx.wait_push(command));

        for i in (0..data.len()).step_by(4) {
            let end_i = data.len().min(i+4);
            let mut chunk = [0u8; 4];
            chunk[..end_i - i].copy_from_slice(&data[i..end_i]);
            block_on(tx.wait_push(u32::from_le_bytes(chunk)));
        }
        while !tx.empty() {}

        defmt::debug!("jtag clock; bits: {}, tms: {:02X}", num_bits, tms);
    }

    pub fn transfer(&mut self, num_bits: u32, tms: bool, tdi: &[u8], tdo: &mut [u8]) {
        assert!(self.pio.sm0.rx().empty());
        assert!(self.pio.sm0.tx().empty());
        assert!(tdo.len() == 0 || tdi.len() == tdo.len());

        let (num_bits, num_bytes) = truncate(num_bits, tdi.len());
        let tdi = &tdi[..num_bytes];
        let tdo = if tdo.len() == 0 { tdo } else { &mut tdo[..num_bytes] };

        let (rx, tx) = self.pio.sm0.rx_tx();
        let tms = if tms { 1 } else { 0 } << 26;
        let command = (num_bits - 1) & 0x03FFFFFF | tms | ((self.jtag_address as u32) << 27);
        block_on(tx.wait_push(command));

        for i in (0..tdi.len()).step_by(4) {
            let end_i = tdi.len().min(i+4);
            let mut chunk = [0u8; 4];
            chunk[..end_i - i].copy_from_slice(&tdi[i..end_i]);
            block_on(tx.wait_push(u32::from_le_bytes(chunk)));

            while rx.empty() {}
            let word = block_on(rx.wait_pull());
            if tdo.len() > 0 {
                let bits = (num_bits - i as u32 * 8).min(32);
                tdo[i..end_i].copy_from_slice(&(word >> (32 - bits)).to_le_bytes()[..end_i - i]);
            }
        }
        if num_bits & 0x1F == 0 {
            // if we sent a multple of 32 bits, there's an extra pull.
            let _ = block_on(rx.wait_pull());
        }

        defmt::debug!("jtag transfer; bits: {}, tms: {}, tdi: {:02X}, tdo: {:02X}", num_bits, tms != 0, tdi, tdo);
    }
}

fn truncate(num_bits: u32, data_len: usize) -> (u32, usize) {
    let num_bytes = (num_bits as usize + 7) / 8;
    if num_bytes < data_len {
        (num_bits, num_bytes)
    } else if num_bytes > data_len {
        (data_len as u32 * 8, data_len)
    } else {
        (num_bits, data_len)
    }
}

pub struct SWD<'a, PIO: pio::Instance> {
    _token: Token<'a>,
    pio: pio::Pio<'a, PIO>,
    _resetn: Option<gpio::OutputOpenDrain<'a>>,
    addr: (u8, u8, u8), // (idle, write, read)
}

impl<'a, PIO: pio::Instance> SWD<'a, PIO> {
    pub fn new(
        token: Token<'a>,
        pio: Peri<'a, PIO>,
        irq: impl Binding<PIO::Interrupt, InterruptHandler<PIO>>,
        swdio: Peri<'a, impl pio::PioPin + 'a>,
        swclk: Peri<'a, impl pio::PioPin + 'a>,
        swdio_dir: Peri<'a, impl pio::PioPin + 'a>,
        resetn: Option<Peri<'a, impl gpio::Pin + 'a>>
    ) -> Self {
        let mut pio = pio::Pio::new(pio, irq);
        let swclk_pin = pio.common.make_pio_pin(swclk);
        let swdio_pin = pio.common.make_pio_pin(swdio);
        let swdio_dir_pin = pio.common.make_pio_pin(swdio_dir);
        let resetn_pin = resetn.map(|resetn| gpio::OutputOpenDrain::new(resetn, true.into()));
        assert_eq!(swdio_dir_pin.pin(), swclk_pin.pin() + 1);

        let prog = pio::program::pio_file!("src/swd.pio");
        let idle_addr = prog.public_defines.idle as u8;
        let write_addr = prog.public_defines.write_cmd as u8;
        let read_addr = prog.public_defines.read_cmd as u8;
        let swd_prog = pio.common.load_program(&prog.program);

        let mut cfg = pio::Config::default();
        cfg.use_program(&swd_prog, &[&swclk_pin, &swdio_dir_pin]);
        cfg.set_out_pins(&[&swdio_pin]);
        cfg.set_in_pins(&[&swdio_pin]);
        cfg.set_set_pins(&[&swdio_pin]);
        let mut shift_cfg = pio::ShiftConfig::default();
        shift_cfg.auto_fill = true;
        cfg.shift_in = shift_cfg;
        cfg.shift_out = shift_cfg;

        pio.sm0.set_config(&cfg);
        pio.sm0.set_pins(gpio::Level::Low, &[&swdio_dir_pin]);
        pio.sm0.set_pins(gpio::Level::High, &[&swclk_pin]);
        pio.sm0.set_pin_dirs(pio::Direction::In, &[&swdio_pin]);
        pio.sm0.set_pin_dirs(pio::Direction::Out, &[&swclk_pin, &swdio_dir_pin]);
        unsafe { pio.sm0.exec_jmp(idle_addr) };
        pio.sm0.set_enable(true);

        Self {
            _token: token,
            pio,
            _resetn: resetn_pin,
            addr: (idle_addr, write_addr, read_addr),
        }
    }

    pub fn set_frequency(&mut self, freq_hz: u32) -> bool {
        if freq_hz <= clk_sys_freq() / 2 && freq_hz >= 2000 {
            let divider = get_pio_divider(2, freq_hz);
            self.pio.sm0.set_clock_divider(divider);
            true
        } else {
            false
        }
    }

    pub fn write_bits(&mut self, num_bits: u32, data: &[u8]) {
        let (num_bits, num_bytes) = truncate(num_bits, data.len());
        let data = &data[..num_bytes];

        let tx = self.pio.sm0.tx();
        let command = ((num_bits - 1) & 0x07FFFFFF) | ((self.addr.1 as u32) << 27);
        block_on(tx.wait_push(command));

        for i in (0..data.len()).step_by(4) {
            let end_i = data.len().min(i+4);
            let mut chunk = [0u8; 4];
            chunk[..end_i - i].copy_from_slice(&data[i..end_i]);
            block_on(tx.wait_push(u32::from_le_bytes(chunk)));
        }
    }

    pub fn read_bits(&mut self, num_bits: u32, data: &mut [u8]) {
        let (num_bits, num_bytes) = truncate(num_bits, data.len());
        let data = &mut data[..num_bytes];

        let (rx, tx) = self.pio.sm0.rx_tx();
        let command = ((num_bits - 1) & 0x07FFFFFF) | ((self.addr.2 as u32) << 27);
        block_on(tx.wait_push(command));

        for i in (0..data.len()).step_by(4) {
            let word = block_on(rx.wait_pull());
            let end_i = data.len().min(i+4);
            let bits = (num_bits - i as u32 * 8).min(32);
            data[i..end_i].copy_from_slice(&(word >> (32 - bits)).to_le_bytes()[..end_i - i]);
        }
        if num_bits & 0x1F == 0 {
            // if we sent a multple of 32 bits, there's an extra pull.
            let _ = block_on(rx.wait_pull());
        }
    }
}

pub struct WrappedUartSWO<'a, P> {
    _token: Token<'a>,
    inner: P,
}

impl<'a, P: cmsis_dap::SWOPort> cmsis_dap::SWOPort for WrappedUartSWO<'a, P> {
    const SUPPORTS_MANCHESTER: bool = P::SUPPORTS_MANCHESTER;
    const SUPPORTS_UART: bool = P::SUPPORTS_UART;

    fn set_mode_manchester(&mut self, baudrate: u32) -> bool {
        self.inner.set_mode_manchester(baudrate)
    }

    fn set_mode_uart(&mut self, baudrate: u32) -> bool {
        self.inner.set_mode_uart(baudrate)
    }

    async fn read_trace_data(&mut self, data: &mut [u8]) -> usize {
        self.inner.read_trace_data(data).await
    }

    fn close(&mut self) {
        self.inner.close()
    }
}
