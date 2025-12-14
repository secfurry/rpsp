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

use core::convert::Into;
use core::option::Option::Some;
use core::result::Result::{self, Err, Ok};

use crate::asm::nop;
use crate::clock::{AlarmConfig, RtcError, TimeSource};
use crate::int::Acknowledge;
use crate::pac::{RESETS, RTC};
use crate::time::Time;

pub struct RtcClock {
    rtc: RTC,
}

// RTC does not have the "mut" settings so we can use it's pointer variant
// from the "Board" struct.
impl RtcClock {
    #[inline]
    pub(crate) fn new(rtc: RTC) -> RtcClock {
        // RTC was already init-ed so we don't need to do anything.
        RtcClock { rtc }
    }

    #[inline]
    pub fn close(&self) {
        unsafe { RESETS::steal() }.reset().modify(|_, r| r.rtc().set_bit());
    }
    #[inline]
    pub fn alarm_disable(&self) {
        self.wait_enable(false);
    }
    #[inline]
    pub fn interrupt_clear(&self) {
        self.wait_enable(false);
        self.wait_enable(true);
    }
    #[inline]
    pub fn is_running(&self) -> bool {
        self.rtc.ctrl().read().rtc_active().bit_is_set()
    }
    #[inline]
    pub fn interrupt_set(&self, en: bool) {
        self.rtc.inte().modify(|_, r| r.rtc().bit(en));
    }
    #[inline]
    pub fn set_leap_year_check(&self, en: bool) {
        self.rtc.ctrl().modify(|_, r| r.force_notleapyear().bit(!en));
    }
    #[inline]
    pub fn now(&self) -> Result<Time, RtcError> {
        self.now_inner()
    }
    pub fn set_time(&self, v: Time) -> Result<(), RtcError> {
        if !v.is_valid() {
            return Err(RtcError::InvalidTime);
        }
        self.rtc.ctrl().modify(|_, r| r.rtc_enable().clear_bit());
        while self.rtc.ctrl().read().rtc_active().bit_is_set() {
            nop();
        }
        self.rtc
            .setup_0()
            .write(|r| unsafe { r.day().bits(v.day).month().bits(v.month as u8).year().bits(v.year) });
        self.rtc.setup_1().write(|r| unsafe {
            r.dotw()
                .bits(v.weekday as u8)
                .hour()
                .bits(v.hours)
                .min()
                .bits(v.mins)
                .sec()
                .bits(v.secs)
        });
        self.rtc.ctrl().write(|r| r.load().set_bit().rtc_enable().set_bit());
        while self.rtc.ctrl().read().rtc_active().bit_is_clear() {
            nop();
        }
        Ok(())
    }
    pub fn set_alarm(&self, v: AlarmConfig) -> Result<(), RtcError> {
        if v.is_empty() {
            return Ok(());
        }
        if !v.is_valid() {
            return Err(RtcError::InvalidTime);
        }
        self.wait_enable(false);
        self.rtc.irq_setup_0().write(|r| unsafe {
            if let Some(i) = v.day {
                r.day_ena().set_bit().day().bits(i.get());
            }
            if !v.month.is_none() {
                r.month_ena().set_bit().month().bits(v.month as u8);
            }
            if let Some(i) = v.year {
                r.year_ena().set_bit().year().bits(i.get());
            }
            r
        });
        self.rtc.irq_setup_1().write(|r| unsafe {
            if !v.weekday.is_none() {
                r.dotw_ena().set_bit().dotw().bits(v.weekday as u8);
            }
            if let Some(i) = v.hours {
                r.hour_ena().set_bit().hour().bits(i);
            }
            if let Some(i) = v.mins {
                r.min_ena().set_bit().min().bits(i);
            }
            if let Some(i) = v.secs {
                r.sec_ena().set_bit().sec().bits(i);
            }
            r
        });
        self.wait_enable(true);
        Ok(())
    }
    #[inline]
    pub fn set_time_from(&self, mut v: impl TimeSource) -> Result<(), RtcError> {
        self.set_time(v.now().map_err(|e| e.into())?)
    }

    #[inline]
    fn wait_enable(&self, en: bool) {
        self.rtc.irq_setup_0().modify(|_, r| r.match_ena().bit(en));
        while self.rtc.irq_setup_0().read().match_ena().bit() != en {
            nop();
        }
    }
    fn now_inner(&self) -> Result<Time, RtcError> {
        if !self.is_running() {
            return Err(RtcError::NotRunning);
        }
        let (a, b) = (self.rtc.rtc_0().read(), self.rtc.rtc_1().read());
        let d = Time::new(
            b.year().bits(),
            b.month().bits().into(),
            b.day().bits(),
            a.hour().bits(),
            a.min().bits(),
            a.sec().bits(),
            a.dotw().bits().into(),
        );
        if !d.is_valid() { Err(RtcError::InvalidTime) } else { Ok(d) }
    }
}

impl TimeSource for RtcClock {
    type Error = RtcError;

    #[inline]
    fn now(&mut self) -> Result<Time, RtcError> {
        self.now_inner()
    }
}
impl TimeSource for &RtcClock {
    type Error = RtcError;

    #[inline]
    fn now(&mut self) -> Result<Time, RtcError> {
        self.now_inner()
    }
}
impl Acknowledge for RtcClock {
    #[inline]
    fn ack_interrupt(&mut self) -> bool {
        self.interrupt_clear();
        true
    }
}
