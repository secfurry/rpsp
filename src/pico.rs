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

use core::mem::zeroed;
use core::ptr::{NonNull, write_volatile};
use core::result::Result;

use crate::atomic::{Mutex, with};
use crate::clock::{Clock, RtcClock, Timer};
use crate::pin::gpio::Output;
use crate::pin::pwm::PwmPin;
use crate::pin::{self, Pin, PinID};
use crate::static_instance;
use crate::watchdog::Watchdog;

static_instance!(INSTANCE, Inner, Inner::new());

pub struct Pico(NonNull<Inner>);

pub type MayFail<T> = Result<!, T>;

struct Inner {
    clk:   Clock,
    dog:   Watchdog,
    timer: Timer,
}

impl Pico {
    #[inline]
    pub fn get() -> Pico {
        Pico(with(|x| {
            let p = INSTANCE.borrow_mut(x);
            if !p.is_ready() {
                p.setup();
            }
            unsafe { NonNull::new_unchecked(p) }
        }))
    }

    #[inline(always)]
    pub fn sleep(&self, ms: u32) {
        self.ptr().timer.sleep_ms(ms)
    }
    #[inline(always)]
    pub fn timer(&self) -> &Timer {
        &self.ptr().timer
    }
    #[inline(always)]
    pub fn rtc(&self) -> &RtcClock {
        self.ptr().clk.rtc()
    }
    #[inline(always)]
    pub fn sleep_us(&self, us: u32) {
        self.ptr().timer.sleep_us(us)
    }
    #[inline(always)]
    pub fn system_freq(&self) -> u32 {
        self.ptr().clk.freq()
    }
    #[inline(always)]
    pub fn current_tick(&self) -> u64 {
        self.ptr().timer.current_tick()
    }
    #[inline(always)]
    pub fn watchdog(&self) -> &Watchdog {
        &self.ptr().dog
    }
    #[inline(always)]
    pub fn system_clock(&self) -> &Clock {
        &self.ptr().clk
    }
    #[inline(always)]
    pub fn pin(&self, p: PinID) -> Pin<Output> {
        Pin::get(self, p)
    }

    #[inline]
    pub(crate) fn enable_ticks(&self) {
        if !self.watchdog().is_ticking() {
            self.watchdog().enable_ticks();
        }
    }

    #[inline(always)]
    fn ptr(&self) -> &mut Inner {
        unsafe { &mut *self.0.as_ptr() }
    }
}
impl Inner {
    #[inline(always)]
    const fn new() -> Inner {
        unsafe { zeroed() }
    }

    #[inline]
    fn setup(&mut self) {
        // Setup pins first.
        pin::setup_pins();
        self.clk = Clock::new();
        self.timer = Timer::new(&self.clk);
        self.dog = Watchdog::new(self.clk.freq());
    }
    #[inline(always)]
    fn is_ready(&self) -> bool {
        self.clk.freq() > 0
    }
}

#[inline(always)]
pub fn ticks() -> u64 {
    Pico::get().current_tick()
}
#[inline(always)]
pub fn sleep(ms: u32) {
    Pico::get().sleep(ms);
}
#[inline(always)]
pub fn watchdog_feed() {
    Pico::get().watchdog().feed();
}
#[inline(always)]
pub fn ticks_ms() -> u64 {
    Pico::get().current_tick() / 1_000
}
#[inline(always)]
pub fn sleep_us(us: u32) {
    Pico::get().sleep_us(us);
}
#[inline(always)]
pub fn watchdog_enable_ticks() {
    Pico::get().watchdog().enable_ticks();
}
#[inline(always)]
pub fn watchdog_start(ms: u32) {
    Pico::get().watchdog().start(ms);
}
#[inline(always)]
pub fn pin(p: PinID) -> Pin<Output> {
    Pico::get().pin(p)
}
#[inline(always)]
pub fn pwm(p: PinID) -> PwmPin<Output> {
    Pico::get().pin(p).into_pwm()
}

#[inline]
pub(super) fn write_reg(reg: *mut u32, v: u32, clear: bool) {
    // NOTE(sf): This seems so weird, but it works lol.
    //           See https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf#atomic-rwtype
    unsafe {
        write_volatile(
            (reg as usize + if clear { 0x3000 } else { 0x2000 }) as *mut u32,
            v,
        )
    }
}
