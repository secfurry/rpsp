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

use core::cell::UnsafeCell;
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{From, TryFrom};
use core::fmt::{self, Debug, Formatter};
use core::marker::{Copy, PhantomData};
use core::option::Option::{self, None, Some};
use core::result::Result::{self, Err, Ok};
use core::unreachable;

use crate::asm::nop;
use crate::int::Acknowledge;
use crate::pac::pads_bank0::GPIO;
use crate::pac::{ADC, IO_BANK0, PADS_BANK0, RESETS, SIO, SYSCFG};
use crate::pin::gpio::{Input, Output};
use crate::pin::pwm::{PwmID, PwmPin};
use crate::{Pico, write_reg};

#[path = "pin/boards/lib.rs"]
mod boards;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::boards::pins::*;

pub mod adc;
pub mod led;
pub mod pwm;

pub enum PinPull {
    Up,
    Down,
    None,
}
#[repr(u8)]
pub enum PinSlew {
    Slow = 0u8,
    Fast = 1u8,
}
#[repr(u8)]
pub enum PinState {
    Low  = 0u8,
    High = 1u8,
}
#[repr(u8)]
pub enum PinStrength {
    Drive2ma  = 0u8,
    Drive4ma  = 1u8,
    Drive8ma  = 2u8,
    Drive12ma = 3u8,
}
#[repr(u8)]
pub enum PinFunction {
    JTag  = 0x00u8,
    Spi   = 0x01u8,
    Uart  = 0x02u8,
    I2c   = 0x03u8,
    Pwm   = 0x04u8,
    Sio   = 0x05u8,
    Pio0  = 0x06u8,
    Pio1  = 0x07u8,
    Clock = 0x08u8,
    Usb   = 0x09u8,
    None  = 0x1Fu8,
}
#[repr(u8)]
pub enum PinDirection {
    In  = 0u8,
    Out = 1u8,
}
#[repr(u8)]
pub enum PinInterrupt {
    Low      = 0x1u8,
    High     = 0x2u8,
    EdgeLow  = 0x4u8,
    EdgeHigh = 0x8u8,
    All      = 0xFu8,
}

pub struct Pin<F: PinIO> {
    i:  PinID,
    _p: PhantomData<UnsafeCell<F>>,
}
pub struct PinInvalidError;

pub trait PinIO {
    const INPUT: bool;
}

pub type PinInput = Pin<Input>;
pub type PinOutput = Pin<Output>;

#[allow(unused)]
// In case any boards don't use all I2C busses.
pub(super) enum I2cID {
    I2C0,
    I2C1,
}
#[allow(unused)]
// In case any boards don't use all SPI busses.
pub(super) enum SpiID {
    Spi0,
    Spi1,
}
#[allow(unused)]
// In case any boards don't use all UART busses.
pub(super) enum UartID {
    Uart0,
    Uart1,
}

impl PinID {
    #[inline]
    pub(super) fn set_pio(&self, pio0: bool) {
        let v = unsafe { &*IO_BANK0::PTR }
            .gpio(*self as usize)
            .gpio_ctrl()
            .read()
            .funcsel()
            .bits();
        match (v, pio0) {
            (0x6, true) => return,
            (0x7, false) => return,
            _ => (),
        }
        self.ctrl().modify(|_, r| {
            r.slewfast()
                .set_bit()
                .schmitt()
                .set_bit()
                .slewfast()
                .set_bit()
                .ie()
                .set_bit()
                .od()
                .clear_bit()
                .pue()
                .clear_bit()
                .pde()
                .clear_bit()
                .drive()
                .bits(3)
        });
        self.set_function(if pio0 { PinFunction::Pio0 } else { PinFunction::Pio1 });
    }
    #[inline]
    pub(super) fn set_input(&self) {
        unsafe { &*SIO::PTR }
            .gpio_oe_clr()
            .write(|r| unsafe { r.bits(self.mask()) });
        self.ctrl().modify(|_, r| r.ie().bit(true).od().bit(false));
    }
    #[inline]
    pub(super) fn set_output(&self) {
        unsafe { &*SIO::PTR }
            .gpio_oe_set()
            .write(|r| unsafe { r.bits(self.mask()) });
        self.ctrl().modify(|_, r| r.ie().bit(true).od().bit(false));
    }
    #[inline]
    pub(super) fn set_function(&self, f: PinFunction) {
        unsafe { &*IO_BANK0::PTR }
            .gpio(*self as usize)
            .gpio_ctrl()
            .modify(|_, r| unsafe { r.funcsel().bits(f as u8) });
        self.ctrl().modify(|_, r| r.ie().bit(f as u8 != 0x1F))
    }

    #[inline(always)]
    fn mask(&self) -> u32 {
        1 << (*self as u32)
    }
    #[inline(always)]
    fn is_odd(&self) -> bool {
        (*self as u8) % 2 != 0
    }
    #[inline(always)]
    fn offset(&self) -> usize {
        (*self as usize) % 8 * 4
    }
    #[inline]
    fn into_input(self) -> PinID {
        self.set_input();
        self.set_function(PinFunction::Sio);
        self
    }
    #[inline]
    fn into_output(self) -> PinID {
        self.set_output();
        self.set_function(PinFunction::Sio);
        self
    }
    #[inline]
    fn ctrl<'a>(&self) -> &'a GPIO {
        unsafe { &*PADS_BANK0::PTR }.gpio(*self as usize)
    }
    fn inter_set(&self, i: PinInterrupt, en: bool) {
        let (p, n) = (unsafe { &*IO_BANK0::PTR }, (*self as usize) / 8);
        write_reg(
            if on_core0() { p.proc0_inte(n).as_ptr() } else { p.proc1_inte(n).as_ptr() },
            (i as u32) << self.offset(),
            !en,
        )
    }
    fn inter_status(&self, i: PinInterrupt) -> bool {
        let (p, n) = (unsafe { &*IO_BANK0::PTR }, (*self as usize) / 8);
        let r = if on_core0() { p.proc0_ints(n).read().bits() } else { p.proc1_ints(n).read().bits() } >> self.offset();
        let m = i as u32;
        r & m == m
    }
    fn inter_enabled(&self, i: PinInterrupt) -> bool {
        let (p, n) = (unsafe { &*IO_BANK0::PTR }, (*self as usize) / 8);
        let r = if on_core0() { p.proc0_inte(n).read().bits() } else { p.proc1_inte(n).read().bits() } >> self.offset();
        let m = i as u32;
        r & m == m
    }
    #[inline]
    fn dorm_wake_set(&self, i: PinInterrupt, en: bool) {
        let p = unsafe { &*IO_BANK0::PTR };
        write_reg(
            p.dormant_wake_inte((*self as usize) / 8).as_ptr(),
            (i as u32) << self.offset(),
            !en,
        )
    }
    #[inline]
    fn dorm_wake_status(&self, i: PinInterrupt) -> bool {
        let (p, m) = (unsafe { &*IO_BANK0::PTR }, i as u32);
        (p.dormant_wake_ints((*self as usize) / 8).read().bits() >> self.offset()) & m == m
    }
    #[inline]
    fn dorm_wake_enabled(&self, i: PinInterrupt) -> bool {
        let (p, m) = (unsafe { &*IO_BANK0::PTR }, i as u32);
        (p.dormant_wake_inte((*self as usize) / 8).read().bits() >> self.offset()) & m == m
    }
}
impl PinPull {
    #[inline(always)]
    fn sets(&self) -> (bool, bool) {
        match self {
            PinPull::Up => (true, false),
            PinPull::Down => (false, true),
            PinPull::None => (false, false),
        }
    }
}
impl Pin<Input> {
    #[inline]
    pub fn is_low(&self) -> bool {
        unsafe { &*SIO::PTR }.gpio_in().read().bits() & self.i.mask() == 0
    }
    #[inline(always)]
    pub fn is_high(&self) -> bool {
        !self.is_low()
    }
    #[inline]
    pub fn get_state(&self) -> bool {
        self.i.ctrl().read().ie().bit_is_set()
    }
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.i.ctrl().read().ie().bit_is_set()
    }
    #[inline]
    pub fn set_state(&self, en: bool) {
        self.i.ctrl().modify(|_, r| r.ie().bit(en))
    }
    #[inline]
    pub fn into_output(self) -> Pin<Output> {
        Pin {
            i:  self.i.into_output(),
            _p: PhantomData,
        }
    }
    #[inline]
    pub fn into_pwm(self) -> Option<PwmPin<Input>> {
        let i = pins_pwm(&self.i);
        if i.is_b() {
            return None;
        }
        self.i.set_function(PinFunction::Pwm);
        i.set_state(true);
        Some(PwmPin::<Input>::new(i))
    }
}
impl Pin<Output> {
    #[inline]
    pub fn get(_p: &Pico, i: PinID) -> Pin<Output> {
        // NOTE(sf): We require the Board struct to make sure the Pins are
        // initialized first.
        let v: Pin<Output> = Pin {
            i:  i.into_output(),
            _p: PhantomData,
        };
        v.low();
        v.set_pull_type(PinPull::None);
        v
    }

    #[inline]
    pub fn low(&self) {
        unsafe { &*SIO::PTR }
            .gpio_out_clr()
            .write(|r| unsafe { r.gpio_out_clr().bits(self.i.mask()) })
    }
    #[inline]
    pub fn high(&self) {
        unsafe { &*SIO::PTR }
            .gpio_out_set()
            .write(|r| unsafe { r.gpio_out_set().bits(self.i.mask()) })
    }
    #[inline]
    pub fn toggle(&self) {
        unsafe { &*SIO::PTR }
            .gpio_out_xor()
            .write(|r| unsafe { r.gpio_out_xor().bits(self.i.mask()) })
    }
    #[inline]
    pub fn set_on(&self, en: bool) {
        if en {
            self.high();
        } else {
            self.low();
        }
    }
    #[inline]
    pub fn get_state(&self) -> bool {
        self.i.ctrl().read().od().bit_is_set()
    }
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.i.ctrl().read().od().bit_is_set()
    }
    #[inline]
    pub fn is_set_low(&self) -> bool {
        unsafe { &*SIO::PTR }.gpio_out().read().bits() & self.i.mask() == 0
    }
    #[inline(always)]
    pub fn is_set_high(&self) -> bool {
        !self.is_set_low()
    }
    #[inline]
    pub fn set_state(&self, en: bool) {
        self.i.ctrl().modify(|_, r| r.od().bit(!en))
    }
    #[inline(always)]
    pub fn into_input(self) -> Pin<Input> {
        Pin {
            i:  self.i.into_input(),
            _p: PhantomData,
        }
    }
    pub fn into_pwm(self) -> PwmPin<Output> {
        let i = pins_pwm(&self.i);
        self.i.set_function(PinFunction::Pwm);
        i.set_state(true);
        PwmPin::<Output>::new(i)
    }
    #[inline]
    pub fn output_high(self) -> Pin<Output> {
        self.high();
        self
    }
    #[inline]
    pub fn output(self, high: bool) -> Pin<Output> {
        if high {
            self.high()
        } else {
            self.low()
        }
        self
    }
}
impl<F: PinIO> Pin<F> {
    #[inline(always)]
    pub fn id(&self) -> &PinID {
        &self.i
    }
    #[inline]
    pub fn get_schmitt(&self) -> bool {
        self.i.ctrl().read().schmitt().bit_is_set()
    }
    #[inline]
    pub fn get_slew(&self) -> PinSlew {
        if self.i.ctrl().read().slewfast().bit_is_set() { PinSlew::Fast } else { PinSlew::Slow }
    }
    #[inline]
    pub fn set_slew(&self, s: PinSlew) {
        self.i.ctrl().modify(|_, r| r.slewfast().bit(s as u8 == 1));
    }
    #[inline]
    pub fn pull_type(&self) -> PinPull {
        let v = self.i.ctrl().read();
        match (v.pue().bit_is_set(), v.pde().bit_is_set()) {
            (true, false) => PinPull::Up,
            (false, true) => PinPull::Down,
            _ => PinPull::None,
        }
    }
    #[inline]
    pub fn set_schmitt(&self, en: bool) {
        self.i.ctrl().modify(|_, r| r.schmitt().bit(en));
    }
    #[inline]
    pub fn is_sync_bypass(&self) -> bool {
        let i = self.i.mask();
        unsafe { SYSCFG::steal() }.proc_in_sync_bypass().read().bits() & i == i
    }
    #[inline(always)]
    pub fn is_pwm_avaliable(&self) -> bool {
        !F::INPUT || (F::INPUT && self.i.is_odd())
    }
    #[inline]
    pub fn pull(self, p: PinPull) -> Pin<F> {
        self.set_pull_type(p);
        self
    }
    #[inline]
    pub fn set_pull_type(&self, p: PinPull) {
        let (x, y) = p.sets();
        self.i.ctrl().modify(|_, r| r.pue().bit(x).pde().bit(y))
    }
    #[inline(always)]
    pub fn set_drive(&self, s: PinStrength) {
        self.i.ctrl().modify(|_, r| r.drive().bits(s as _));
    }
    #[inline]
    pub fn get_strength(&self) -> PinStrength {
        match self.i.ctrl().read().drive().bits() {
            0 => PinStrength::Drive2ma,
            1 => PinStrength::Drive4ma,
            2 => PinStrength::Drive8ma,
            3 => PinStrength::Drive12ma,
            _ => unreachable!(),
        }
    }
    #[inline(always)]
    pub fn set_function(&self, f: PinFunction) {
        self.i.set_function(f)
    }
    #[inline]
    pub fn interrupt_clear(&self, i: PinInterrupt) {
        unsafe { &*IO_BANK0::PTR }
            .intr((self.i as usize) / 8)
            .write(|r| unsafe { r.bits((i as u32) << self.i.offset()) })
    }
    #[inline(always)]
    pub fn interrupt_set(&self, i: PinInterrupt, en: bool) {
        self.i.inter_set(i, en)
    }
    #[inline(always)]
    pub fn interrupt_status(&self, i: PinInterrupt) -> bool {
        self.i.inter_status(i)
    }
    #[inline(always)]
    pub fn interrupt_enabled(&self, i: PinInterrupt) -> bool {
        self.i.inter_enabled(i)
    }
    #[inline(always)]
    pub fn dormant_wake_set(&self, i: PinInterrupt, en: bool) {
        self.i.dorm_wake_set(i, en)
    }
    #[inline(always)]
    pub fn dormant_wake_status(&self, i: PinInterrupt) -> bool {
        self.i.dorm_wake_status(i)
    }
    #[inline(always)]
    pub fn dormant_wake_enabled(&self, i: PinInterrupt) -> bool {
        self.i.dorm_wake_enabled(i)
    }

    #[inline]
    pub unsafe fn set_sync_bypass(&self, en: bool) {
        write_reg(
            unsafe { SYSCFG::steal() }.proc_in_sync_bypass().as_ptr(),
            self.i.mask(),
            !en,
        );
    }
}

impl PinIO for Input {
    const INPUT: bool = true;
}
impl PinIO for Output {
    const INPUT: bool = false;
}

impl<F: PinIO> Clone for Pin<F> {
    #[inline(always)]
    fn clone(&self) -> Pin<F> {
        Pin { i: self.i, _p: PhantomData }
    }
}

impl Eq for PinID {}
impl Ord for PinID {
    #[inline(always)]
    fn cmp(&self, other: &PinID) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl Copy for PinID {}
impl Clone for PinID {
    #[inline(always)]
    fn clone(&self) -> PinID {
        *self
    }
}
impl PartialEq for PinID {
    #[inline(always)]
    fn eq(&self, other: &PinID) -> bool {
        (*self as u8).eq(&(*other as u8))
    }
}
impl PartialOrd for PinID {
    #[inline(always)]
    fn partial_cmp(&self, other: &PinID) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}
impl TryFrom<u8> for PinID {
    type Error = PinInvalidError;

    #[inline]
    fn try_from(v: u8) -> Result<PinID, PinInvalidError> {
        match v {
            0 => Ok(PinID::Pin0),
            1 => Ok(PinID::Pin1),
            2 => Ok(PinID::Pin2),
            3 => Ok(PinID::Pin3),
            4 => Ok(PinID::Pin4),
            5 => Ok(PinID::Pin5),
            6 => Ok(PinID::Pin6),
            7 => Ok(PinID::Pin7),
            8 => Ok(PinID::Pin8),
            9 => Ok(PinID::Pin9),
            10 => Ok(PinID::Pin10),
            11 => Ok(PinID::Pin11),
            12 => Ok(PinID::Pin12),
            13 => Ok(PinID::Pin13),
            14 => Ok(PinID::Pin14),
            15 => Ok(PinID::Pin15),
            16 => Ok(PinID::Pin16),
            17 => Ok(PinID::Pin17),
            18 => Ok(PinID::Pin18),
            19 => Ok(PinID::Pin19),
            20 => Ok(PinID::Pin20),
            21 => Ok(PinID::Pin21),
            22 => Ok(PinID::Pin22),
            26 => Ok(PinID::Pin26),
            27 => Ok(PinID::Pin27),
            28 => Ok(PinID::Pin28),
            _ => Err(PinInvalidError),
        }
    }
}

impl Copy for PinSlew {}
impl Clone for PinSlew {
    #[inline(always)]
    fn clone(&self) -> PinSlew {
        *self
    }
}

impl Copy for PinState {}
impl Clone for PinState {
    #[inline(always)]
    fn clone(&self) -> PinState {
        *self
    }
}

impl Copy for PinStrength {}
impl Clone for PinStrength {
    #[inline(always)]
    fn clone(&self) -> PinStrength {
        *self
    }
}

impl Copy for PinFunction {}
impl Clone for PinFunction {
    #[inline(always)]
    fn clone(&self) -> PinFunction {
        *self
    }
}

impl Copy for PinInterrupt {}
impl Clone for PinInterrupt {
    #[inline(always)]
    fn clone(&self) -> PinInterrupt {
        *self
    }
}

impl Copy for PinDirection {}
impl Clone for PinDirection {
    #[inline(always)]
    fn clone(&self) -> PinDirection {
        *self
    }
}

impl From<Pin<Output>> for Pin<Input> {
    #[inline(always)]
    fn from(v: Pin<Output>) -> Pin<Input> {
        v.into_input()
    }
}

impl<F: PinIO> Acknowledge for Pin<F> {
    #[inline]
    fn ack_interrupt(&mut self) -> bool {
        let r = self.interrupt_status(PinInterrupt::All);
        self.interrupt_clear(PinInterrupt::All);
        r
    }
}

#[cfg(feature = "debug")]
impl Debug for PinInvalidError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("PinInvalidError")
    }
}
#[cfg(not(feature = "debug"))]
impl Debug for PinInvalidError {
    #[inline(always)]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

#[inline]
pub fn emergency_pin_on(i: PinID) {
    let v = i.into_output().mask();
    unsafe { &*SIO::PTR }
        .gpio_out_set()
        .write(|r| unsafe { r.gpio_out_set().bits(v) })
}

pub(crate) fn setup_pins() {
    let s = unsafe { SIO::steal() };
    let r = unsafe { RESETS::steal() };
    let b = unsafe { PADS_BANK0::steal() };
    r.reset().modify(|_, r| r.pads_bank0().set_bit());
    r.reset().modify(|_, r| r.io_bank0().set_bit());
    unsafe {
        s.gpio_oe().write_with_zero(|r| r.bits(0));
        s.gpio_out().write_with_zero(|r| r.bits(0));
    }
    r.reset().modify(|_, r| r.io_bank0().clear_bit());
    while r.reset_done().read().io_bank0().bit_is_clear() {
        nop();
    }
    r.reset().modify(|_, r| r.pads_bank0().clear_bit());
    while r.reset_done().read().pads_bank0().bit_is_clear() {
        nop();
    }
    r.reset().modify(|_, r| r.pwm().clear_bit());
    while r.reset_done().read().pwm().bit_is_clear() {
        nop();
    }
    // Enable all pins by default.
    b.gpio(0).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(1).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(2).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(3).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(4).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(5).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(6).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(7).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(8).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(9).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(10).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(11).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(12).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(13).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(14).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(15).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(16).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(17).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(18).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(19).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(20).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(21).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(22).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(26).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(27).modify(|_, r| r.od().clear_bit().ie().set_bit());
    b.gpio(28).modify(|_, r| r.od().clear_bit().ie().set_bit());
    PwmID::Pwm0A.set_defaults();
    PwmID::Pwm1A.set_defaults();
    PwmID::Pwm2A.set_defaults();
    PwmID::Pwm3A.set_defaults();
    PwmID::Pwm4A.set_defaults();
    PwmID::Pwm5A.set_defaults();
    PwmID::Pwm6A.set_defaults();
    PwmID::Pwm7A.set_defaults();
    unsafe { ADC::steal() }.cs().write(|r| r.en().clear_bit());
}

#[inline]
fn on_core0() -> bool {
    unsafe { (*SIO::ptr()).cpuid().read().bits() == 0 }
}

pub mod gpio {
    pub struct Input;
    pub struct Output;
}
