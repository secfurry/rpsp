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
use core::cmp;
use core::fmt::Debug;
use core::iter::Iterator;
use core::marker::{Copy, PhantomData};
use core::ops::{Deref, DerefMut, Drop, FnOnce};
use core::option::Option::{self, None, Some};
use core::ptr::{read_volatile, write_volatile};
use core::result::Result::{self, Err, Ok};

use crate::asm::nop;
use crate::pac::pio0::{RegisterBlock, SM};
use crate::pac::{PIO0, PIO1, RESETS};
use crate::pin::{PinDirection, PinID, PinState};
use crate::pio::state::{Running, Stopped, Uninit};
use crate::{Pico, write_reg};

mod config;
mod group;
mod int;
mod io;

pub use self::config::*;
pub use self::group::*;
pub use self::int::*;
pub use self::io::*;

pub const MAX_INSTRUCTIONS: usize = 0x20usize;

#[repr(u8)]
pub enum Slot {
    Index0 = 0u8,
    Index1 = 1u8,
    Index2 = 2u8,
    Index3 = 3u8,
}
pub enum PioID {
    Pio0,
    Pio1,
}
pub enum PioError {
    TooLarge,
    WouldBlock,
    InvalidProgram,
}

pub struct Pio {
    sm:   UnsafeCell<u8>,
    dev:  *const RegisterBlock,
    used: u32,
}
pub struct Handle {
    src:    u8,
    mask:   u32,
    offset: u8,
    target: u8,
}
pub struct Synced<'a> {
    s: &'a mut State<'a, Stopped>,
    m: u32,
}
pub struct Machine<S: PioState> {
    sm:  *const SM,
    idx: Slot,
    pio: *const RegisterBlock,
    _p:  PhantomData<S>,
}
pub struct State<'a, S: PioState> {
    m:  Machine<S>,
    _p: PhantomData<&'a SM>,
}
pub struct Program<const N: usize = MAX_INSTRUCTIONS> {
    pub code:        [u16; N],
    pub start:       Option<u8>,
    pub wrap_src:    u8,
    pub wrap_target: u8,
    len:             u8,
}

pub trait PioState {}
pub trait PioStateDone: PioState {}
pub trait PioStateOccupied: PioState {}

impl Pio {
    pub fn get(_p: &Pico, i: PioID) -> Pio {
        let r = unsafe { RESETS::steal() };
        let v = match i {
            PioID::Pio0 => {
                r.reset().modify(|_, r| r.pio0().set_bit());
                r.reset().modify(|_, r| r.pio0().clear_bit());
                while r.reset_done().read().pio0().bit_is_clear() {
                    nop();
                }
                PIO0::ptr()
            },
            PioID::Pio1 => {
                r.reset().modify(|_, r| r.pio1().set_bit());
                r.reset().modify(|_, r| r.pio1().clear_bit());
                while r.reset_done().read().pio1().bit_is_clear() {
                    nop();
                }
                PIO1::ptr()
            },
        };
        Pio {
            sm:   UnsafeCell::new(0u8),
            dev:  v,
            used: 0u32,
        }
    }

    #[inline]
    pub fn irq_flags(&self) -> u8 {
        self.ptr().irq().read().irq().bits()
    }
    #[inline]
    pub fn irq_clear(&self, v: u8) {
        self.ptr().irq().write(|r| unsafe { r.irq().bits(v) })
    }
    #[inline]
    pub fn irq_force(&self, v: u8) {
        self.ptr().irq_force().write(|r| unsafe { r.irq_force().bits(v) })
    }
    #[inline(always)]
    pub fn irq0<'a>(&'a self) -> Interrupt<'a> {
        Interrupt::new(self, Request::Irq0)
    }
    #[inline(always)]
    pub fn irq1<'a>(&'a self) -> Interrupt<'a> {
        Interrupt::new(self, Request::Irq1)
    }
    #[inline(always)]
    pub fn irq<'a>(&'a self, i: Request) -> Interrupt<'a> {
        Interrupt::new(self, i)
    }
    #[inline]
    pub fn release<'a, S: PioStateDone>(&mut self, i: State<'a, S>) {
        unsafe { *self.sm.get() &= 1 << i.m.idx as u8 }
    }
    #[inline]
    pub fn state<'a>(&'a self, i: Slot) -> Option<State<'a, Uninit>> {
        unsafe {
            if *self.sm.get() & (1 << i as u8) != 0 {
                return None;
            }
            *self.sm.get() |= 1 << i as u8;
        }
        Some(State {
            m:  Machine {
                _p:  PhantomData,
                sm:  self.ptr().sm(i as usize) as *const SM,
                idx: i,
                pio: self.dev,
            },
            _p: PhantomData,
        })
    }
    pub fn install<const N: usize>(&mut self, p: &Program<N>) -> Result<Handle, PioError> {
        if p.len == 0 {
            return Err(PioError::InvalidProgram);
        }
        let n = cmp::min(N, p.len as usize);
        if n > MAX_INSTRUCTIONS {
            return Err(PioError::TooLarge);
        }
        let (s, m) = match p.start {
            Some(v) => self.try_install(v, &p.code[0..n]).map(|r| (v, r)),
            None => self.try_install_find(&p.code[0..n]),
        }
        .ok_or(PioError::TooLarge)?;
        Ok(Handle {
            src:    p.wrap_src,
            mask:   m,
            offset: s,
            target: p.wrap_target,
        })
    }

    #[inline(always)]
    pub unsafe fn uninstall(&mut self, h: Handle) {
        self.used &= !h.mask
    }
    #[inline(always)]
    pub unsafe fn state_unsafe<'a>(&'a self, i: Slot) -> State<'a, Uninit> {
        State {
            m:  Machine {
                _p:  PhantomData,
                sm:  self.ptr().sm(i as usize) as *const SM,
                idx: i,
                pio: self.dev,
            },
            _p: PhantomData,
        }
    }

    #[inline(always)]
    fn ptr(&self) -> &RegisterBlock {
        unsafe { &*self.dev }
    }
    #[inline]
    fn try_install_find(&mut self, code: &[u16]) -> Option<(u8, u32)> {
        for i in 0..MAX_INSTRUCTIONS {
            match self.try_install(i as u8, code) {
                Some(v) => return Some((i as u8, v)),
                None => continue,
            }
        }
        None
    }
    fn try_install(&mut self, start: u8, code: &[u16]) -> Option<u32> {
        let (d, mut u) = (self.ptr(), 0u32);
        for (i, x) in code.iter().enumerate() {
            let v = (i as u8 + start) % 0x20;
            let m = 1u32 << v;
            if (self.used | u) & m != 0 {
                return None;
            }
            let e = transform(start, *x)?;
            d.instr_mem(v as usize).write(|r| unsafe { r.instr_mem0().bits(e) });
            u |= m;
        }
        self.used |= u;
        Some(u)
    }
}
impl Handle {
    #[inline(always)]
    pub fn mask(&self) -> u32 {
        self.mask
    }
    #[inline(always)]
    pub fn offset(&self) -> u8 {
        self.offset
    }
    #[inline(always)]
    pub fn wrap_src(&self) -> u8 {
        self.src
    }
    #[inline(always)]
    pub fn wrap_target(&self) -> u8 {
        self.target
    }
    #[inline(always)]
    pub fn wrap_src_adjusted(&self) -> u8 {
        self.src.saturating_add(self.offset)
    }
    #[inline(always)]
    pub fn wrap_target_adjusted(&self) -> u8 {
        self.target.saturating_add(self.offset)
    }
}
impl<'a> Synced<'a> {
    #[inline(always)]
    pub fn add(mut self, other: &'a State<Stopped>) -> Synced<'a> {
        self.m |= 1 << other.idx as u8;
        self
    }
}
impl<'a> State<'a, Uninit> {
    #[inline(always)]
    pub(self) fn init(self) -> State<'a, Stopped> {
        State {
            m:  Machine {
                _p:  PhantomData,
                sm:  self.m.sm,
                idx: self.m.idx,
                pio: self.m.pio,
            },
            _p: PhantomData,
        }
    }
}
impl<'a> State<'a, Stopped> {
    #[inline]
    pub fn start(mut self) -> State<'a, Running> {
        self.set_state(true);
        self.started()
    }
    #[inline]
    pub fn start_paused(self) -> State<'a, Running> {
        self.started()
    }

    #[inline(always)]
    fn started(self) -> State<'a, Running> {
        State {
            m:  Machine {
                _p:  PhantomData,
                sm:  self.m.sm,
                idx: self.m.idx,
                pio: self.m.pio,
            },
            _p: PhantomData,
        }
    }
}
impl<'a> State<'a, Running> {
    #[inline]
    pub fn stop(mut self) -> State<'a, Stopped> {
        self.set_state(false);
        self.stopped()
    }

    #[inline(always)]
    fn stopped(self) -> State<'a, Stopped> {
        State {
            m:  Machine {
                _p:  PhantomData,
                sm:  self.m.sm,
                idx: self.m.idx,
                pio: self.m.pio,
            },
            _p: PhantomData,
        }
    }
}
impl<S: PioState> Machine<S> {
    #[inline]
    pub fn pc(&self) -> u32 {
        self.sm().sm_addr().read().bits()
    }
    #[inline(always)]
    pub fn restart(&mut self) {
        self.ctrl(1 << (self.idx as u8 + 4), false)
    }
    #[inline]
    pub fn x(&mut self) -> u32 {
        unsafe {
            self.exec(0x4020);
            read_volatile(self.pio().rxf(self.idx as usize).as_ptr() as *mut u32)
        }
    }
    #[inline]
    pub fn y(&mut self) -> u32 {
        unsafe {
            self.exec(0x4040);
            read_volatile(self.pio().rxf(self.idx as usize).as_ptr() as *mut u32)
        }
    }
    #[inline]
    pub fn drain_fifo(&mut self) {
        let s = self.sm();
        let v = s.sm_shiftctrl().read().fjoin_rx().bit();
        s.sm_shiftctrl().modify(|_, r| r.fjoin_rx().bit(!v));
        s.sm_shiftctrl().modify(|_, r| r.fjoin_rx().bit(v))
    }
    #[inline(always)]
    pub fn restart_clock(&mut self) {
        self.ctrl(1 << (self.idx as u8 + 8), false)
    }
    #[inline]
    pub fn set_x(&mut self, v: u32) {
        unsafe {
            write_volatile(self.pio().txf(self.idx as usize).as_ptr() as *mut u32, v);
            self.exec(0x6020)
        }
    }
    #[inline]
    pub fn set_y(&mut self, v: u32) {
        unsafe {
            write_volatile(self.pio().txf(self.idx as usize).as_ptr() as *mut u32, v);
            self.exec(0x6040)
        }
    }
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.pio().ctrl().read().sm_enable().bits() & (1 << self.idx as u8) != 0
    }
    #[inline]
    pub fn is_stalled(&self) -> bool {
        self.sm().sm_execctrl().read().exec_stalled().bit()
    }
    #[inline(always)]
    pub fn set_state(&mut self, en: bool) {
        self.ctrl(1 << self.idx as u8, !en)
    }
    #[inline]
    pub fn set_clock_div(&mut self, int: u16, frac: u8) {
        self.sm()
            .sm_clkdiv()
            .write(|r| unsafe { r.int().bits(int).frac().bits(frac) });
    }

    #[inline]
    pub unsafe fn jump(&mut self, addr: u8) {
        unsafe { self.exec(addr as u16) }
    }
    #[inline]
    pub unsafe fn exec(&mut self, inst: u16) {
        self.sm().sm_instr().write(|r| unsafe { r.sm0_instr().bits(inst) })
    }

    #[inline(always)]
    fn sm(&self) -> &SM {
        unsafe { &*self.sm }
    }
    #[inline(always)]
    fn pio(&self) -> &RegisterBlock {
        unsafe { &*self.pio }
    }
    #[inline(always)]
    fn ctrl(&self, v: u32, clear: bool) {
        write_reg(self.pio().ctrl().as_ptr(), v, clear)
    }
}
impl<const N: usize> Program<N> {
    #[inline]
    pub const fn new(start: i8, wrap_src: u8, wrap_target: u8, code: [u16; N]) -> Program<N> {
        Program {
            code,
            wrap_src,
            wrap_target,
            len: code.len() as u8,
            start: if start < 0 { None } else { Some(start as u8) },
        }
    }
}
impl<'a, S: PioState> State<'a, S> {
    #[inline(always)]
    pub fn group(self, other: State<'a, S>) -> StateGroup2<'a, S> {
        StateGroup2::new(self, other)
    }

    #[inline(always)]
    pub unsafe fn recouple(self, m: Machine<S>) -> State<'a, S> {
        State { m, _p: PhantomData }
    }
}
impl<S: PioStateOccupied> Machine<S> {
    #[inline(always)]
    pub fn rx_u8(&self) -> Rx<u8> {
        Rx::new(self)
    }
    #[inline(always)]
    pub fn tx_u8(&self) -> Tx<u8> {
        Tx::new(self)
    }
    #[inline(always)]
    pub fn rx_u16(&self) -> Rx<u16> {
        Rx::new(self)
    }
    #[inline(always)]
    pub fn tx_u16(&self) -> Tx<u16> {
        Tx::new(self)
    }
    #[inline(always)]
    pub fn rx_u32(&self) -> Rx<u32> {
        Rx::new(self)
    }
    #[inline(always)]
    pub fn tx_u32(&self) -> Tx<u32> {
        Tx::new(self)
    }
    #[inline]
    pub fn rx<T: PioIO>(&self) -> Rx<T> {
        Rx::new(self)
    }
    #[inline]
    pub fn tx<T: PioIO>(&self) -> Tx<T> {
        Tx::new(self)
    }
    #[inline(always)]
    pub fn set_pin_sync_bypass(&self, pin: PinID) {
        self.pio().input_sync_bypass().write(|r| unsafe { r.bits(1 << pin as u8) });
    }
    #[inline(always)]
    pub fn set_pins_sync_bypass(&self, pins: &[PinID]) {
        self.pio()
            .input_sync_bypass()
            .write(|r| unsafe { r.bits(pins.iter().map(|v| 1 << *v as u8).sum()) });
    }
    pub fn set_pin_state(&mut self, state: PinState, pin: PinID) {
        let v = match state {
            PinState::Low => 0xE000u16,
            PinState::High => 0xE001u16,
        };
        pin.set_pio(self.pio == PIO0::PTR);
        self.paused(|m| unsafe {
            let s = m.sm();
            s.sm_pinctrl().write(|r| r.set_base().bits(pin as u8).set_count().bits(1));
            s.sm_instr().write(|r| r.sm0_instr().bits(v));
        });
    }
    pub fn set_pins_state(&mut self, state: PinState, pins: &[PinID]) {
        let v = match state {
            PinState::Low => 0xE000u16,
            PinState::High => 0xE001u16,
        };
        let f = self.pio == PIO0::PTR;
        self.paused(|m| {
            let s = m.sm();
            for i in pins {
                i.set_pio(f);
                unsafe {
                    s.sm_pinctrl().write(|r| r.set_base().bits(*i as u8).set_count().bits(1));
                    s.sm_instr().write(|r| r.sm0_instr().bits(v));
                }
            }
        });
    }
    pub fn set_pin_direction(&mut self, dir: PinDirection, pin: PinID) {
        let v = match dir {
            PinDirection::In => 0xE080u16,
            PinDirection::Out => 0xE081u16,
        };
        pin.set_pio(self.pio == PIO0::PTR);
        self.paused(|m| unsafe {
            let s = m.sm();
            s.sm_pinctrl().write(|r| r.set_base().bits(pin as u8).set_count().bits(1));
            s.sm_instr().write(|r| r.sm0_instr().bits(v));
        });
    }
    pub fn set_pins_direction(&mut self, dir: PinDirection, pins: &[PinID]) {
        let v = match dir {
            PinDirection::In => 0xE080u16,
            PinDirection::Out => 0xE081u16,
        };
        let f = self.pio == PIO0::PTR;
        self.paused(|m| {
            let s = m.sm();
            for i in pins {
                i.set_pio(f);
                unsafe {
                    s.sm_pinctrl().write(|r| r.set_base().bits(*i as u8).set_count().bits(1));
                    s.sm_instr().write(|r| r.sm0_instr().bits(v));
                }
            }
        });
    }

    fn paused(&mut self, func: impl FnOnce(&mut Machine<S>)) {
        let x = self.is_enabled();
        self.set_state(false);
        let (p, e) = {
            let s = self.sm();
            let (p, e) = (s.sm_pinctrl().read().bits(), s.sm_execctrl().read().bits());
            unsafe { s.sm_execctrl().write_with_zero(|r| r.out_sticky().set_bit()) }
            (p, e)
        };
        func(self);
        unsafe {
            let s = self.sm();
            s.sm_pinctrl().write(|r| r.bits(p));
            s.sm_execctrl().write(|r| r.bits(e));
        }
        self.set_state(x);
    }
}
impl<'a, S: PioStateOccupied> State<'a, S> {
    #[inline]
    pub fn release(mut self) -> State<'a, Uninit> {
        self.set_state(false);
        State {
            m:  Machine {
                _p:  PhantomData,
                sm:  self.m.sm,
                idx: self.m.idx,
                pio: self.m.pio,
            },
            _p: PhantomData,
        }
    }

    #[inline(always)]
    pub unsafe fn uncouple(self) -> Machine<S> {
        self.m
    }
}

impl Drop for Synced<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        self.s.ctrl(self.m << 8, false)
    }
}

impl Copy for Slot {}
impl Clone for Slot {
    #[inline(always)]
    fn clone(&self) -> Slot {
        *self
    }
}

impl Copy for PioID {}
impl Clone for PioID {
    #[inline(always)]
    fn clone(&self) -> PioID {
        *self
    }
}

impl Debug for PioError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidProgram => f.write_str("InvalidProgram"),
            Self::TooLarge => f.write_str("TooLarge"),
            PioError::WouldBlock => f.write_str("WouldBlock"),
        }
    }
}

impl<'a, S: PioState> Deref for State<'a, S> {
    type Target = Machine<S>;

    #[inline(always)]
    fn deref(&self) -> &Machine<S> {
        &self.m
    }
}
impl<'a, S: PioState> DerefMut for State<'a, S> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Machine<S> {
        &mut self.m
    }
}

impl PioState for Uninit {}
impl PioState for Running {}
impl PioState for Stopped {}

impl PioStateDone for Uninit {}
impl PioStateDone for Stopped {}

impl PioStateOccupied for Running {}
impl PioStateOccupied for Stopped {}

#[inline]
fn transform(start: u8, x: u16) -> Option<u16> {
    if x & 0xE000 != 0 {
        return Some(x);
    }
    let v = (x & 0x1F) as u8 + start;
    if v > MAX_INSTRUCTIONS as u8 {
        return None;
    }
    Some((x & 0xFFE0) | v as u16)
}

pub mod state {
    pub struct Uninit;
    pub struct Running;
    pub struct Stopped;
}
