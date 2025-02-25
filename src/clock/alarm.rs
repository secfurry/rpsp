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
extern crate cortex_m;

use core::clone::Clone;
use core::marker::Copy;

use cortex_m::interrupt::free;

use crate::int::Acknowledge;
use crate::pac::TIMER;
use crate::{write_reg, Pico};

#[repr(u8)]
pub enum AlarmID {
    Alarm0 = 1u8,
    Alarm1 = 2u8,
    Alarm2 = 4u8,
    Alarm3 = 8u8,
}

pub struct Alarm {
    i:   AlarmID,
    dev: TIMER,
}

impl Alarm {
    #[inline]
    pub fn new(p: &Pico, a: AlarmID) -> Alarm {
        // NOTE(sf): Make sure the Watchdog is ticking.
        p.enable_ticks();
        Alarm {
            i:   a,
            dev: unsafe { TIMER::steal() },
        }
    }

    #[inline]
    pub fn cancel(&mut self) {
        let i = self.i as u32;
        unsafe { self.dev.armed().write_with_zero(|r| r.bits(i)) };
        write_reg(self.dev.intf().as_ptr(), i, true)
    }
    #[inline]
    pub fn done(&self) -> bool {
        self.dev.armed().read().bits() & (self.i as u32) == 0
    }
    #[inline(always)]
    pub fn id(&self) -> &AlarmID {
        &self.i
    }
    #[inline]
    pub fn interrupt_clear(&mut self) {
        write_reg(self.dev.intf().as_ptr(), self.i as u32, true);
        match &self.i {
            AlarmID::Alarm0 => unsafe { self.dev.intr().write_with_zero(|r| r.alarm_0().clear_bit_by_one()) },
            AlarmID::Alarm1 => unsafe { self.dev.intr().write_with_zero(|r| r.alarm_1().clear_bit_by_one()) },
            AlarmID::Alarm2 => unsafe { self.dev.intr().write_with_zero(|r| r.alarm_2().clear_bit_by_one()) },
            AlarmID::Alarm3 => unsafe { self.dev.intr().write_with_zero(|r| r.alarm_3().clear_bit_by_one()) },
        }
    }
    pub fn current_tick(&self) -> u64 {
        let mut v = self.dev.timerawh().read().bits();
        loop {
            let (l, h) = (
                self.dev.timerawl().read().bits(),
                self.dev.timerawh().read().bits(),
            );
            if v == h {
                return ((h as u64) << 32) | l as u64;
            }
            v = h;
        }
    }
    #[inline(always)]
    pub fn schedule(&mut self, ms: u32) {
        self.schedule_us(ms * 1_000);
    }
    pub fn schedule_us(&mut self, us: u32) {
        let v = self.current_tick() + us as u64;
        let l = (v & 0xFFFFFFFF) as u32;
        // NOTE(sf): Run without Interrupts
        free(|_| {
            match &self.i {
                AlarmID::Alarm0 => unsafe { self.dev.alarm0().write(|r| r.bits(l)) },
                AlarmID::Alarm1 => unsafe { self.dev.alarm1().write(|r| r.bits(l)) },
                AlarmID::Alarm2 => unsafe { self.dev.alarm2().write(|r| r.bits(l)) },
                AlarmID::Alarm3 => unsafe { self.dev.alarm3().write(|r| r.bits(l)) },
            }
            let (n, i) = (self.current_tick(), self.i as u32);
            if n <= v || self.dev.armed().read().bits() & i == 0 {
                return;
            }
            unsafe { self.dev.armed().write_with_zero(|r| r.bits(i)) };
            write_reg(self.dev.intf().as_ptr(), i, false);
        })
    }
    #[inline(always)]
    pub fn interrupt_set(&mut self, en: bool) {
        write_reg(self.dev.inte().as_ptr(), self.i as u32, !en)
    }
}

impl Copy for AlarmID {}
impl Clone for AlarmID {
    #[inline(always)]
    fn clone(&self) -> AlarmID {
        *self
    }
}

impl Acknowledge for Alarm {
    #[inline]
    fn ack_interrupt(&mut self) -> bool {
        let r = self.done();
        self.interrupt_clear();
        r
    }
}
