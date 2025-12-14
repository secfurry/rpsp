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
use core::cmp::Ord;

use crate::pac::{PSM, WATCHDOG};

pub enum Scratch {
    Register0,
    Register1,
    Register2,
    Register3,
    Register4,
    Register5,
    Register6,
    Register7,
}

pub struct Watchdog {
    dog:   WATCHDOG,
    value: UnsafeCell<u32>,
}

impl Watchdog {
    #[inline]
    pub(crate) fn new(freq: u32) -> Watchdog {
        let w = unsafe { WATCHDOG::steal() };
        w.tick().write(|r| unsafe { r.bits((freq / 1_000_000).min(0xFF)) });
        Watchdog {
            dog:   w,
            value: UnsafeCell::new(0u32),
        }
    }

    #[inline]
    pub fn feed(&self) {
        self.dog.load().write(|r| unsafe { r.bits(*self.value.get()) });
    }
    #[inline]
    pub fn ping(&self) {
        self.feed();
    }
    #[inline]
    pub fn disable(&self) {
        self.dog.ctrl().write(|r| r.enable().clear_bit());
    }
    #[inline]
    pub fn enable_ticks(&self) {
        self.dog.tick().modify(|v, r| unsafe { r.bits(0x200 | v.bits()) })
    }
    #[inline]
    pub fn start(&self, ms: u32) {
        self.start_us(ms * 1_000);
    }
    #[inline]
    pub fn countdown(&self) -> u16 {
        self.dog.tick().read().cycles().bits()
    }
    #[inline]
    pub fn restart(&self, ms: u32) {
        self.restart_us(ms * 1_000)
    }
    pub fn start_us(&self, us: u32) {
        let d = us.min(0x7FFFFF);
        self.dog.ctrl().write(|r| r.enable().clear_bit());
        unsafe {
            PSM::steal()
                .wdsel()
                .write_with_zero(|r| r.bits(0x0001FFFF).xosc().clear_bit().rosc().clear_bit());
            *self.value.get() = d.saturating_mul(2);
        };
        if !self.is_ticking() {
            self.enable_ticks();
        }
        self.feed();
        self.dog.ctrl().write(|r| r.enable().set_bit())
    }
    #[inline]
    pub fn is_ticking(&self) -> bool {
        self.dog.tick().read().running().bit()
    }
    #[inline]
    pub fn restart_us(&self, us: u32) {
        self.disable();
        self.start_us(us);
    }
    #[inline]
    pub fn pause_on_debug(&self, pause_en: bool) {
        self.dog.ctrl().write(|r| {
            r.pause_dbg0()
                .bit(pause_en)
                .pause_dbg1()
                .bit(pause_en)
                .pause_jtag()
                .bit(pause_en)
        });
    }
    #[inline]
    pub fn enable_with_cycles(&self, cycles: u8) {
        self.dog.tick().write(|r| unsafe { r.bits(0x200 | cycles as u32) });
    }
    #[inline]
    pub fn read_scratch(&self, r: Scratch) -> u32 {
        match r {
            Scratch::Register0 => self.dog.scratch0().read().bits(),
            Scratch::Register1 => self.dog.scratch1().read().bits(),
            Scratch::Register2 => self.dog.scratch2().read().bits(),
            Scratch::Register3 => self.dog.scratch3().read().bits(),
            Scratch::Register4 => self.dog.scratch4().read().bits(),
            Scratch::Register5 => self.dog.scratch5().read().bits(),
            Scratch::Register6 => self.dog.scratch6().read().bits(),
            Scratch::Register7 => self.dog.scratch7().read().bits(),
        }
    }
    #[inline]
    pub fn write_scratch(&self, r: Scratch, v: u32) {
        match r {
            Scratch::Register0 => self.dog.scratch0().write(|r| unsafe { r.bits(v) }),
            Scratch::Register1 => self.dog.scratch1().write(|r| unsafe { r.bits(v) }),
            Scratch::Register2 => self.dog.scratch2().write(|r| unsafe { r.bits(v) }),
            Scratch::Register3 => self.dog.scratch3().write(|r| unsafe { r.bits(v) }),
            Scratch::Register4 => self.dog.scratch4().write(|r| unsafe { r.bits(v) }),
            Scratch::Register5 => self.dog.scratch5().write(|r| unsafe { r.bits(v) }),
            Scratch::Register6 => self.dog.scratch6().write(|r| unsafe { r.bits(v) }),
            Scratch::Register7 => self.dog.scratch7().write(|r| unsafe { r.bits(v) }),
        }
    }
}
