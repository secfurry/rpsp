// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
//

#![no_implicit_prelude]

extern crate core;

use core::clone::Clone;
use core::convert::From;
use core::fmt::{self, Debug, Formatter};
use core::iter::Iterator;
use core::marker::{PhantomData, Send};
use core::ops::{Deref, DerefMut};
use core::option::Option::{self, None, Some};
use core::ptr::NonNull;
use core::result::Result::{self, Err, Ok};

use crate::Board;
use crate::asm::nop;
use crate::i2c::mode::{Controller, Peripheral, State};
use crate::pac::i2c0::RegisterBlock;
use crate::pac::{I2C0, I2C1, RESETS};
use crate::pin::{I2cID, PinFunction, PinID, pins_i2c};

pub enum I2cError {
    WouldBlock,
    InvalidPins,
    InvalidAddress,
    InvalidFrequency,
    ReadBreak,
    ReadOverrun,
    ReadInvalid,
    AbortBus,
    AbortLoss,
    AbortOther,
    AbortNoAckData,
    AbortNoAckAddress,
}
pub enum I2cEvent {
    Stop,
    Start,
    Restart,
    Read,
    Write,
}

pub enum I2cBus<'a, M: I2cMode> {
    Owned(I2c<M>),
    Shared(&'a mut I2c<M>),
    Duplicated((I2c<M>, PhantomData<&'a I2c<M>>)),
}

pub struct I2cAddress(u16);
pub struct I2c<M: I2cMode> {
    dev:  NonNull<RegisterBlock>,
    mode: M,
}

pub trait I2cMode: Clone {
    const CONTROLLER: bool;
}

pub type I2cController = I2c<Controller>;
pub type I2cPeripheral = I2c<Peripheral>;

impl I2cAddress {
    #[inline(always)]
    pub const fn new_7bit(v: u8) -> I2cAddress {
        I2cAddress(v as u16 | 0x8000u16)
    }
    #[inline(always)]
    pub const fn new_10bit(v: u16) -> I2cAddress {
        I2cAddress(v & 0x7FFFu16)
    }

    #[inline]
    pub fn value(&self) -> u16 {
        self.0 & 0x7FFFu16
    }
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.is_10bit() || ((self.0 & 0x7FFFu16) < 0x80u16)
    }
    #[inline]
    pub fn is_10bit(&self) -> bool {
        self.0 & 0x8000u16 == 0
    }
}
impl I2c<Peripheral> {
    #[inline(always)]
    pub fn new(p: &Board, sda: PinID, scl: PinID, addr: I2cAddress) -> Result<I2c<Peripheral>, I2cError> {
        I2cPeripheral::new_peripheral(p, sda, scl, addr)
    }

    pub fn write(&mut self, b: &[u8]) -> usize {
        let d = self.ptr();
        let _ = d.ic_clr_tx_abrt().read();
        let mut n = 0usize;
        for i in b {
            if self.tx_is_full() {
                break;
            }
            d.ic_data_cmd().write(|r| unsafe { r.dat().bits(*i) });
            n += 1;
        }
        let _ = d.ic_clr_rd_req().read();
        n
    }
    pub fn event(&mut self) -> Option<I2cEvent> {
        let d = self.ptr();
        let s = d.ic_raw_intr_stat().read();
        match self.mode.state {
            State::Idle if s.start_det().bit_is_set() => {
                let _ = d.ic_clr_start_det().read();
                self.mode.state = State::Active;
                Some(I2cEvent::Start)
            },
            State::Active if s.rd_req().bit_is_set() => {
                if s.stop_det().bit_is_set() {
                    d.ic_clr_stop_det().read();
                }
                self.mode.state = State::Reading;
                Some(I2cEvent::Read)
            },
            State::Active if !self.rx_is_empty() => {
                self.mode.state = State::Writing;
                Some(I2cEvent::Write)
            },
            State::Reading if s.rd_req().bit_is_set() => Some(I2cEvent::Read),
            State::Writing if !self.rx_is_empty() => Some(I2cEvent::Write),
            State::Reading | State::Writing if s.restart_det().bit_is_set() => {
                let _ = d.ic_clr_restart_det().read();
                let _ = d.ic_clr_start_det().read();
                self.mode.state = State::Active;
                Some(I2cEvent::Restart)
            },
            _ if s.stop_det().bit_is_set() => {
                let _ = d.ic_clr_stop_det().read();
                let _ = d.ic_clr_tx_abrt().read();
                Some(I2cEvent::Stop)
            },
            _ => None,
        }
    }
    #[inline]
    pub fn read_single(&mut self) -> Option<u8> {
        if self.rx_is_empty() {
            return None;
        } else {
            Some(self.ptr().ic_data_cmd().read().dat().bits())
        }
    }
    pub fn read(&mut self, b: &mut [u8]) -> usize {
        let d = self.ptr();
        let mut n = 0usize;
        for i in b.iter_mut() {
            if self.rx_is_empty() {
                break;
            }
            *i = d.ic_data_cmd().read().dat().bits();
            n += 1;
        }
        n
    }
    pub fn write_single(&mut self, v: u8) -> bool {
        let d = self.ptr();
        let _ = d.ic_clr_tx_abrt().read();
        if self.tx_is_full() {
            return false;
        }
        d.ic_data_cmd().write(|r| unsafe { r.dat().bits(v) });
        let _ = d.ic_clr_rd_req().read();
        true
    }
}
impl I2c<Controller> {
    pub const DEFAULT_FREQ: u32 = 400_000u32;

    #[inline(always)]
    pub fn new(p: &Board, sda: PinID, scl: PinID, freq: u32) -> Result<I2c<Controller>, I2cError> {
        I2cController::new_controller(p, sda, scl, freq)
    }

    #[inline]
    pub fn read_single(&mut self, addr: I2cAddress) -> Result<u8, I2cError> {
        self.prepare(addr)?;
        self.read_raw_single(true, true)
    }
    #[inline]
    pub fn write(&mut self, addr: I2cAddress, b: &[u8]) -> Result<usize, I2cError> {
        self.prepare(addr)?;
        self.write_raw(true, true, b)
    }
    #[inline]
    pub fn write_single(&mut self, addr: I2cAddress, v: u8) -> Result<(), I2cError> {
        self.prepare(addr)?;
        self.write_raw_single(true, true, v)
    }
    #[inline]
    pub fn read(&mut self, addr: I2cAddress, b: &mut [u8]) -> Result<usize, I2cError> {
        self.prepare(addr)?;
        self.read_raw(true, true, b)
    }
    #[inline]
    pub fn write_then_read_single(&mut self, addr: I2cAddress, b: &[u8]) -> Result<u8, I2cError> {
        self.prepare(addr)?;
        self.write_raw(true, false, b)?;
        self.read_raw_single(true, true)
    }
    #[inline]
    pub fn transfer(&mut self, addr: I2cAddress, input: &[u8], out: &mut [u8]) -> Result<(), I2cError> {
        self.prepare(addr)?;
        self.write_raw(true, false, input)?;
        self.read_raw(false, true, out)?;
        Ok(())
    }
    #[inline]
    pub fn write_single_then_read(&mut self, addr: I2cAddress, v: u8, out: &mut [u8]) -> Result<usize, I2cError> {
        self.prepare(addr)?;
        self.write_raw_single(true, false, v)?;
        self.read_raw(false, true, out)
    }

    fn reset(&self) {
        let d = self.ptr();
        d.ic_enable().modify(|_, r| r.abort().set_bit());
        while d.ic_enable().read().abort().bit_is_set() {
            nop();
        }
        while d.ic_raw_intr_stat().read().tx_abrt().bit_is_clear() {
            nop();
        }
        let _ = d.ic_clr_tx_abrt().read();
        let _ = d.ic_tx_abrt_source().read();
    }
    #[inline]
    fn check_errors(&self) -> u32 {
        let d = self.ptr();
        let r = d.ic_tx_abrt_source().read().bits();
        if r > 0 {
            let _ = d.ic_clr_tx_abrt().read();
        }
        r
    }
    #[inline]
    fn check_errors_break(&self) -> Result<(), I2cError> {
        let e = self.check_errors();
        if e > 0 { Err(abort_type(e)) } else { Ok(()) }
    }
    fn prepare(&self, addr: I2cAddress) -> Result<(), I2cError> {
        if !addr.is_valid() {
            return Err(I2cError::InvalidAddress);
        }
        let d = self.ptr();
        d.ic_enable().write(|r| r.enable().disabled());
        d.ic_con().modify(|_, r| r.ic_10bitaddr_master().bit(addr.is_10bit()));
        d.ic_tar().write(|r| unsafe { r.ic_tar().bits(addr.value()) });
        d.ic_enable().write(|r| r.enable().enabled());
        Ok(())
    }
    fn read_raw_single(&self, init: bool, stop: bool) -> Result<u8, I2cError> {
        while self.tx_is_full() {
            nop();
        }
        let d = self.ptr();
        d.ic_data_cmd().write(|r| {
            if !init {
                r.restart().enable();
            }
            r.stop().bit(stop).cmd().read()
        });
        while d.ic_rxflr().read().bits() == 0 {
            self.check_errors_break()?;
        }
        Ok(d.ic_data_cmd().read().dat().bits())
    }
    fn check_errors_spin(&self, stop: bool, last: u32) -> Result<(), I2cError> {
        let e = if last > 0 {
            while self.tx_is_not_empty() {
                nop();
            }
            self.check_errors()
        } else {
            0u32
        };
        if e > 0 || stop {
            while self.tx_is_not_stop() {
                nop();
            }
            self.ptr().ic_clr_stop_det().read().clr_stop_det();
        }
        if e > 0 { Err(abort_type(e)) } else { Ok(()) }
    }
    fn write_raw(&self, init: bool, stop: bool, b: &[u8]) -> Result<usize, I2cError> {
        if b.is_empty() {
            self.reset();
            return Ok(0);
        }
        let (d, mut e) = (self.ptr(), 0u32);
        for (i, v) in b.iter().enumerate() {
            e = self.check_errors();
            if e > 0 {
                break;
            }
            while self.tx_is_full() {
                nop();
            }
            d.ic_data_cmd().write(|r| {
                if i == 0 && !init {
                    r.restart().enable();
                }
                r.stop().bit(stop && i + 1 >= b.len());
                unsafe { r.dat().bits(*v) }
            });
        }
        self.check_errors_spin(stop, e)?;
        Ok(b.len())
    }
    fn write_raw_single(&self, init: bool, stop: bool, v: u8) -> Result<(), I2cError> {
        let e = self.check_errors();
        if e > 0 {
            return self.check_errors_spin(stop, e);
        }
        while self.tx_is_full() {
            nop();
        }
        let d = self.ptr();
        d.ic_data_cmd().write(|r| {
            if !init {
                r.restart().enable();
            }
            r.stop().bit(stop);
            unsafe { r.dat().bits(v) }
        });
        self.check_errors_spin(stop, 0)
    }
    fn read_raw(&self, init: bool, stop: bool, b: &mut [u8]) -> Result<usize, I2cError> {
        if b.is_empty() {
            self.reset();
            return Ok(0);
        }
        let (d, c) = (self.ptr(), b.len());
        for (i, v) in b.iter_mut().enumerate() {
            while self.tx_is_full() {
                nop();
            }
            d.ic_data_cmd().write(|r| {
                if i == 0 && !init {
                    r.restart().enable();
                }
                r.stop().bit(stop && i + 1 >= c).cmd().read()
            });
            while d.ic_rxflr().read().bits() == 0 {
                self.check_errors_break()?;
            }
            *v = d.ic_data_cmd().read().dat().bits();
        }
        Ok(b.len())
    }
}
impl<M: I2cMode> I2c<M> {
    pub fn new_controller(p: &Board, sda: PinID, scl: PinID, freq: u32) -> Result<I2c<Controller>, I2cError> {
        if freq > 1_000_000 {
            return Err(I2cError::InvalidFrequency);
        }
        let s = p.system_freq();
        let b = (s + freq / 2) / freq;
        let l = b * 3 / 5;
        let h = b - l;
        if h > 0xFFFF || l > 0xFFFF || h < 8 || l < 8 {
            return Err(I2cError::InvalidFrequency);
        }
        let c = if freq < 1_000_000 {
            ((s * 3) / 10_000_000) + 1
        } else {
            if s < 32_000_000 {
                return Err(I2cError::InvalidFrequency);
            }
            ((s * 3) / 25_000_000) + 1
        };
        if c > l - 2 {
            return Err(I2cError::InvalidFrequency);
        }
        let v = pins_i2c(&sda, &scl).ok_or(I2cError::InvalidPins)?;
        let r = unsafe { RESETS::steal() };
        let d = match v {
            I2cID::I2C0 => {
                r.reset().modify(|_, r| r.i2c0().set_bit());
                r.reset().modify(|_, r| r.i2c0().clear_bit());
                while r.reset_done().read().i2c0().bit_is_clear() {
                    nop();
                }
                I2C0::PTR
            },
            I2cID::I2C1 => {
                r.reset().modify(|_, r| r.i2c1().set_bit());
                r.reset().modify(|_, r| r.i2c1().clear_bit());
                while r.reset_done().read().i2c1().bit_is_clear() {
                    nop();
                }
                I2C1::PTR
            },
        };
        unsafe {
            let x = &*d;
            x.ic_enable().write(|r| r.enable().disabled());
            x.ic_con().modify(|_, r| {
                r.speed()
                    .bits(0x2)
                    .master_mode()
                    .enabled()
                    .ic_slave_disable()
                    .slave_disabled()
                    .ic_restart_en()
                    .enabled()
                    .tx_empty_ctrl()
                    .enabled()
            });
            x.ic_tx_tl().write(|r| r.tx_tl().bits(0));
            x.ic_rx_tl().write(|r| r.rx_tl().bits(0));
            x.ic_fs_scl_hcnt().write(|r| r.ic_fs_scl_hcnt().bits(h as u16));
            x.ic_fs_scl_lcnt().write(|r| r.ic_fs_scl_lcnt().bits(l as u16));
            x.ic_fs_spklen()
                .write(|r| r.ic_fs_spklen().bits(if l < 0x10 { 1u8 } else { (l / 0x10) as u8 }));
            x.ic_sda_hold().modify(|_, r| r.ic_sda_tx_hold().bits(c as u16));
            x.ic_tx_tl().write(|r| r.tx_tl().bits(0x10));
            x.ic_rx_tl().write(|r| r.rx_tl().bits(0));
            x.ic_con().modify(|_, r| r.rx_fifo_full_hld_ctrl().enabled());
            x.ic_enable().write(|r| r.enable().enabled());
        }
        sda.set_function(PinFunction::I2c);
        scl.set_function(PinFunction::I2c);
        scl.set_output();
        sda.set_output();
        Ok(I2c {
            dev:  unsafe { NonNull::new_unchecked(d as *mut RegisterBlock) },
            mode: Controller,
        })
    }
    pub fn new_peripheral(_p: &Board, sda: PinID, scl: PinID, addr: I2cAddress) -> Result<I2c<Peripheral>, I2cError> {
        if !addr.is_valid() {
            return Err(I2cError::InvalidAddress);
        }
        let v = pins_i2c(&sda, &scl).ok_or(I2cError::InvalidPins)?;
        let r = unsafe { RESETS::steal() };
        let d = match v {
            I2cID::I2C0 => {
                r.reset().modify(|_, r| r.i2c0().set_bit());
                r.reset().modify(|_, r| r.i2c0().clear_bit());
                while r.reset_done().read().i2c0().bit_is_clear() {
                    nop();
                }
                I2C0::PTR
            },
            I2cID::I2C1 => {
                r.reset().modify(|_, r| r.i2c1().set_bit());
                r.reset().modify(|_, r| r.i2c1().clear_bit());
                while r.reset_done().read().i2c1().bit_is_clear() {
                    nop();
                }
                I2C1::PTR
            },
        };
        unsafe {
            let x = &*d;
            x.ic_enable().write(|r| r.enable().disabled());
            x.ic_sar().write(|r| r.ic_sar().bits(addr.value()));
            x.ic_con().modify(|_, r| {
                r.speed()
                    .bits(0x2)
                    .master_mode()
                    .disabled()
                    .ic_slave_disable()
                    .slave_enabled()
                    .rx_fifo_full_hld_ctrl()
                    .enabled()
                    .ic_restart_en()
                    .enabled()
                    .ic_10bitaddr_slave()
                    .bit(addr.is_10bit())
            });
            x.ic_tx_tl().write(|r| r.tx_tl().bits(0));
            x.ic_rx_tl().write(|r| r.rx_tl().bits(0));
            let _ = x.ic_clr_intr().read();
            x.ic_intr_mask().write_with_zero(|r| {
                r.m_start_det()
                    .disabled()
                    .m_rd_req()
                    .disabled()
                    .m_rx_full()
                    .disabled()
                    .m_stop_det()
                    .disabled()
            });
            x.ic_enable().write(|r| r.enable().enabled());
        }
        sda.set_function(PinFunction::I2c);
        scl.set_function(PinFunction::I2c);
        scl.set_input();
        sda.set_input();
        Ok(I2c {
            dev:  unsafe { NonNull::new_unchecked(d as *mut RegisterBlock) },
            mode: Peripheral { state: State::Idle },
        })
    }

    pub fn close(&self) {
        let r = unsafe { RESETS::steal() };
        r.reset().modify(
            |_, r| {
                if self.dev.as_ptr().addr() == I2C0::PTR.addr() { r.i2c0().set_bit() } else { r.i2c1().set_bit() }
            },
        );
    }
    #[inline]
    pub fn rx_used(&self) -> u8 {
        self.ptr().ic_rxflr().read().rxflr().bits()
    }
    #[inline]
    pub fn tx_used(&self) -> u8 {
        self.ptr().ic_txflr().read().txflr().bits()
    }
    #[inline(always)]
    pub fn tx_available(&self) -> u8 {
        0x10u8.saturating_sub(self.tx_used())
    }
    #[inline]
    pub fn tx_is_full(&self) -> bool {
        self.ptr().ic_status().read().tfnf().bit_is_clear()
    }
    #[inline(always)]
    pub fn rx_available(&self) -> u8 {
        0x10u8.saturating_sub(self.rx_used())
    }
    #[inline]
    pub fn tx_is_empty(&self) -> bool {
        self.ptr().ic_raw_intr_stat().read().tx_empty().is_active()
    }
    #[inline]
    pub fn rx_is_empty(&self) -> bool {
        self.ptr().ic_status().read().rfne().bit_is_clear()
    }
    #[inline(always)]
    pub fn is_controller(&self) -> bool {
        M::CONTROLLER
    }

    #[inline(always)]
    fn ptr(&self) -> &RegisterBlock {
        unsafe { self.dev.as_ref() }
    }
    #[inline]
    fn tx_is_not_stop(&self) -> bool {
        self.ptr().ic_raw_intr_stat().read().stop_det().is_inactive()
    }
    #[inline]
    fn tx_is_not_empty(&self) -> bool {
        self.ptr().ic_raw_intr_stat().read().tx_empty().is_inactive()
    }
}

impl I2cMode for Controller {
    const CONTROLLER: bool = true;
}
impl I2cMode for Peripheral {
    const CONTROLLER: bool = false;
}

impl Iterator for I2c<Peripheral> {
    type Item = I2cEvent;

    #[inline(always)]
    fn next(&mut self) -> Option<I2cEvent> {
        self.event()
    }
}

impl<M: I2cMode> Deref for I2cBus<'_, M> {
    type Target = I2c<M>;

    #[inline(always)]
    fn deref(&self) -> &I2c<M> {
        match self {
            I2cBus::Owned(v) => &v,
            I2cBus::Shared(v) => v,
            I2cBus::Duplicated((v, _)) => v,
        }
    }
}
impl<M: I2cMode> DerefMut for I2cBus<'_, M> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut I2c<M> {
        match self {
            I2cBus::Owned(v) => v,
            I2cBus::Shared(v) => v,
            I2cBus::Duplicated((v, _)) => v,
        }
    }
}
impl<'a, M: I2cMode> From<I2c<M>> for I2cBus<'a, M> {
    #[inline(always)]
    fn from(v: I2c<M>) -> I2cBus<'a, M> {
        I2cBus::Owned(v)
    }
}
impl<'a, M: I2cMode> From<&'a I2c<M>> for I2cBus<'a, M> {
    #[inline(always)]
    fn from(v: &'a I2c<M>) -> I2cBus<'a, M> {
        I2cBus::Duplicated((
            I2c {
                dev:  v.dev,
                mode: v.mode.clone(),
            },
            PhantomData,
        ))
    }
}
impl<'a, M: I2cMode> From<&'a mut I2c<M>> for I2cBus<'a, M> {
    #[inline(always)]
    fn from(v: &'a mut I2c<M>) -> I2cBus<'a, M> {
        I2cBus::Shared(v)
    }
}

impl From<u8> for I2cAddress {
    #[inline(always)]
    fn from(v: u8) -> I2cAddress {
        I2cAddress::new_7bit(v)
    }
}
impl From<u16> for I2cAddress {
    #[inline(always)]
    fn from(v: u16) -> I2cAddress {
        I2cAddress::new_10bit(v)
    }
}

unsafe impl<M: I2cMode> Send for I2c<M> {}

#[cfg(feature = "debug")]
impl Debug for I2cError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            I2cError::WouldBlock => f.write_str("WouldBlock"),
            I2cError::InvalidPins => f.write_str("InvalidPins"),
            I2cError::InvalidAddress => f.write_str("InvalidAddress"),
            I2cError::InvalidFrequency => f.write_str("InvalidFrequency"),
            I2cError::ReadBreak => f.write_str("ReadBreak"),
            I2cError::ReadOverrun => f.write_str("ReadOverrun"),
            I2cError::ReadInvalid => f.write_str("ReadInvalid"),
            I2cError::AbortBus => f.write_str("AbortBus"),
            I2cError::AbortLoss => f.write_str("AbortLoss"),
            I2cError::AbortOther => f.write_str("AbortOther"),
            I2cError::AbortNoAckData => f.write_str("AbortNoAckData"),
            I2cError::AbortNoAckAddress => f.write_str("AbortNoAckAddress"),
        }
    }
}
#[cfg(not(feature = "debug"))]
impl Debug for I2cError {
    #[inline(always)]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

#[inline]
fn abort_type(e: u32) -> I2cError {
    match e {
        _ if e & 0x1000 != 0 => I2cError::AbortLoss,
        _ if e & 0x80 != 0 || e & 0x40 != 0 => I2cError::AbortBus,
        _ if e & 0x8 != 0 => I2cError::AbortNoAckData,
        _ if e & 0xF != 0 => I2cError::AbortNoAckAddress,
        _ => I2cError::AbortOther,
    }
}

pub mod mode {
    extern crate core;

    use core::clone::Clone;
    use core::marker::Copy;

    pub struct Controller;
    pub struct Peripheral {
        pub(super) state: State,
    }

    pub(super) enum State {
        Idle,
        Active,
        Reading,
        Writing,
    }

    impl Copy for State {}
    impl Clone for State {
        #[inline(always)]
        fn clone(&self) -> State {
            *self
        }
    }

    impl Clone for Controller {
        #[inline(always)]
        fn clone(&self) -> Controller {
            Controller
        }
    }
    impl Clone for Peripheral {
        #[inline(always)]
        fn clone(&self) -> Peripheral {
            Peripheral { state: self.state }
        }
    }
}
