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
use core::marker::{Copy, PhantomData};

use crate::int::Acknowledge;
use crate::pac::pwm::CH;
use crate::pac::PWM;
use crate::pin::gpio::{Input, Output};
use crate::pin::PinIO;
use crate::write_reg;

#[repr(u8)]
pub enum PwmID {
    Pwm0A = 0x00u8,
    Pwm0B = 0x10u8,
    Pwm1A = 0x01u8,
    Pwm1B = 0x11u8,
    Pwm2A = 0x02u8,
    Pwm2B = 0x12u8,
    Pwm3A = 0x03u8,
    Pwm3B = 0x13u8,
    Pwm4A = 0x04u8,
    Pwm4B = 0x14u8,
    Pwm5A = 0x05u8,
    Pwm5B = 0x15u8,
    Pwm6A = 0x06u8,
    Pwm6B = 0x16u8,
    Pwm7A = 0x07u8,
    Pwm7B = 0x17u8,
}
pub enum PwmMode {
    Free,
    High,
    Rising,
    Falling,
}

pub struct PwmPin<F: PinIO> {
    i:  PwmID,
    s:  PwmState,
    _p: PhantomData<F>,
}

pub type PwmInput = PwmPin<Input>;
pub type PwmOutput = PwmPin<Output>;

struct PwmState(UnsafeCell<(u16, bool)>);

impl PwmState {
    #[inline(always)]
    fn new() -> PwmState {
        PwmState(UnsafeCell::new((0, true)))
    }

    #[inline(always)]
    fn value(&self) -> u16 {
        unsafe { &*self.0.get() }.0
    }
    #[inline(always)]
    fn enabled(&self) -> bool {
        unsafe { &*self.0.get() }.1
    }
    #[inline(always)]
    fn set_value(&self, v: u16) {
        unsafe { (&mut *self.0.get()).0 = v }
    }
    #[inline(always)]
    fn set_enabled(&self, v: bool) {
        unsafe { (&mut *self.0.get()).1 = v }
    }
}

impl PwmID {
    #[inline(always)]
    pub(super) fn is_b(&self) -> bool {
        ((*self as u8) >> 4) == 1
    }
    #[inline]
    pub(super) fn set_defaults(&self) {
        self.set_phase_correct(true);
        self.set_div_int(1u8);
        self.set_div_frac(0u8);
        self.set_inv(false, true);
        self.set_top(0xFFFEu16);
        self.set_counter(0u16);
        self.set_duty(0u16, true);
        self.set_state(false);
    }
    #[inline]
    pub(super) fn set_state(&self, en: bool) {
        self.reg().csr().modify(|_, r| r.en().bit(en))
    }

    #[inline]
    fn interrupt_clear(&self) {
        unsafe { (*PWM::ptr()).intr().write(|r| r.bits(1 << (*self as usize) & 0xF)) }
    }
    #[inline]
    fn set_top(&self, v: u16) {
        self.reg().top().write(|r| unsafe { r.top().bits(v) })
    }
    #[inline]
    fn reg<'a>(&self) -> &'a CH {
        unsafe { (*PWM::ptr()).ch((*self as usize) & 0xF) }
    }
    #[inline]
    fn set_div_int(&self, v: u8) {
        self.reg().div().modify(|_, r| unsafe { r.int().bits(v) })
    }
    #[inline]
    fn set_counter(&self, v: u16) {
        self.reg().ctr().write(|r| unsafe { r.ctr().bits(v) })
    }
    #[inline]
    fn set_div_frac(&self, v: u8) {
        self.reg().div().modify(|_, r| unsafe { r.frac().bits(v) })
    }
    #[inline]
    fn is_overflown(&self) -> bool {
        let v = 1 << ((*self as u32) & 0xF);
        unsafe { (*PWM::ptr()).intr().read().bits() & v == v }
    }
    #[inline(always)]
    fn interrupt_set(&self, en: bool) {
        write_reg(
            unsafe { (&*PWM::ptr()).inte().as_ptr() },
            1 << ((*self as u32) & 0xF),
            !en,
        )
    }
    #[inline]
    fn set_phase_correct(&self, en: bool) {
        self.reg().csr().modify(|_, r| r.ph_correct().bit(en))
    }
    #[inline]
    fn set_duty(&self, v: u16, both: bool) {
        self.reg().cc().modify(|_, r| unsafe {
            if both {
                r.a().bits(v).b().bits(v)
            } else {
                if self.is_b() {
                    r.b().bits(v)
                } else {
                    r.a().bits(v)
                }
            }
        })
    }
    #[inline]
    fn set_inv(&self, inv: bool, both: bool) {
        self.reg().csr().modify(|_, r| {
            if both {
                r.a_inv().bit(inv).b_inv().bit(inv)
            } else {
                if self.is_b() {
                    r.b_inv().bit(inv)
                } else {
                    r.a_inv().bit(inv)
                }
            }
        })
    }
}
impl PwmPin<Output> {
    #[inline(always)]
    pub fn low(&self) {
        self.set_duty(0)
    }
    #[inline(always)]
    pub fn high(&self) {
        self.set_duty(self.get_max_duty())
    }
    #[inline]
    pub fn set_on(&self, en: bool) {
        if en {
            self.high();
        } else {
            self.low();
        }
    }
}
impl<F: PinIO> PwmPin<F> {
    #[inline(always)]
    pub(super) fn new(i: PwmID) -> PwmPin<F> {
        PwmPin {
            i,
            s: PwmState::new(),
            _p: PhantomData,
        }
    }

    #[inline(always)]
    pub fn id(&self) -> &PwmID {
        &self.i
    }
    #[inline(always)]
    pub fn set_top(&self, v: u16) {
        self.i.set_top(v)
    }
    #[inline]
    pub fn get_top(&self) -> u16 {
        self.i.reg().top().read().top().bits()
    }
    #[inline]
    pub fn get_duty(&self) -> u16 {
        match self.s.enabled() {
            true if self.i.is_b() => self.i.reg().cc().read().b().bits(),
            true => self.i.reg().cc().read().a().bits(),
            _ => self.s.value(),
        }
    }
    #[inline(always)]
    pub fn interrupt_clear(&self) {
        self.i.interrupt_clear()
    }
    #[inline]
    pub fn set_duty(&self, v: u16) {
        self.s.set_value(v);
        if !self.s.enabled() {
            return;
        }
        self.i.set_duty(v, false)
    }
    #[inline(always)]
    pub fn get_state(&self) -> bool {
        self.s.enabled()
    }
    #[inline]
    pub fn get_counter(&self) -> u16 {
        self.i.reg().ctr().read().ctr().bits()
    }
    #[inline(always)]
    pub fn is_enabled(&self) -> bool {
        self.s.enabled()
    }
    #[inline(always)]
    pub fn set_div_int(&self, v: u8) {
        self.i.set_div_int(v)
    }
    #[inline(always)]
    pub fn set_div_frac(&self, v: u8) {
        self.i.set_div_frac(v)
    }
    #[inline(always)]
    pub fn set_counter(&self, v: u16) {
        self.i.set_counter(v)
    }
    #[inline]
    pub fn get_max_duty(&self) -> u16 {
        self.i.reg().top().read().top().bits().saturating_add(1)
    }
    #[inline]
    pub fn set_state(&self, en: bool) {
        if en {
            self.i.set_duty(self.s.value(), false);
            self.s.set_enabled(true)
        } else {
            self.s.set_value(self.get_duty());
            self.i.set_duty(0, false);
            self.s.set_enabled(false)
        }
    }
    #[inline(always)]
    pub fn is_overflown(&self) -> bool {
        self.i.is_overflown()
    }
    #[inline]
    pub fn set_mode(&self, m: PwmMode) {
        self.i.reg().csr().modify(|_, r| r.divmode().bits(m as _))
    }
    #[inline(always)]
    pub fn set_inverted(&self, inv: bool) {
        self.i.set_inv(inv, false)
    }
    #[inline(always)]
    pub fn interrupt_set(&self, en: bool) {
        self.i.interrupt_set(en)
    }
    #[inline(always)]
    pub fn set_phase_correct(&self, en: bool) {
        self.i.set_phase_correct(en)
    }
}

impl Copy for PwmID {}
impl Clone for PwmID {
    #[inline(always)]
    fn clone(&self) -> PwmID {
        *self
    }
}

impl<F: PinIO> Acknowledge for PwmPin<F> {
    #[inline]
    fn ack_interrupt(&mut self) -> bool {
        let r = self.is_overflown();
        self.interrupt_clear();
        r
    }
}
