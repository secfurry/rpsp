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
use core::hint::unreachable_unchecked;
use core::iter::Iterator;
use core::mem::zeroed;
use core::ops::AddAssign;
use core::option::Option::{None, Some};
use core::unreachable;

use crate::asm::{delay, nop};
use crate::clock::RtcClock;
use crate::pac::{CLOCKS, PLL_SYS, PLL_USB, RESETS, ROSC, RTC, SCB, SYST, TIMER, XOSC};

pub(crate) const DIV: u32 = 0x100u32;

const FREQ_RTC: u32 = 46_875u32;
const FREQ_XOSC: u32 = 12_000_000u32;
const FREQ_ROSC: u32 = 149_500_000u32;

pub struct Timer {
    clk:  SYST,
    int:  TIMER,
    freq: u32,
}
pub struct Clock {
    rtc:  RtcClock,
    freq: u32,
    seed: u32,
}

impl Clock {
    #[inline]
    pub(crate) fn new() -> Clock {
        Clock::new_with_freq(FREQ_ROSC)
    }
    #[inline]
    pub(crate) fn new_with_freq(freq: u32) -> Clock {
        let c = unsafe { CLOCKS::steal() }; // Disable Resus
        unsafe { c.clk_sys_resus_ctrl().write_with_zero(|w| w) };
        // Setup XOSC and set it as the reference clock.
        let x = setup_xosc();
        // Setup and tune the ROSC.
        let (f, t) = setup_rosc(&c, freq);
        // Setup the internal clocks.
        setup_ref(&c, false);
        setup_sys(&c);
        setup_per(&c);
        // TODO(sf): Correct clock skew
        let r = setup_rtc(&c, (f as f32 * 1f32) as u32, FREQ_RTC + 125);
        // Enable the RTC and ROSC to go DORMANT
        c.sleep_en0().write(|r| unsafe { r.bits(0x300000) });
        c.sleep_en1().write(|r| unsafe { r.bits(0) });
        // Disable and spin-down the XOSC.
        unsafe { x.ctrl().write_with_zero(|r| r.enable().disable()) };
        while x.status().read().stable().bit_is_set() || x.ctrl().read().enable().is_enable() {
            nop();
        }
        setup_powersave(&c); // Disable the unused clocks.
        Clock {
            rtc:  RtcClock::new(r),
            freq: f,
            seed: t,
        }
    }

    #[inline]
    pub fn freq(&self) -> u32 {
        self.freq
    }
    #[inline]
    pub fn seed(&self) -> u32 {
        self.seed
    }
    #[inline]
    pub fn rtc(&self) -> &RtcClock {
        &self.rtc
    }
    #[inline]
    pub fn set_wake_only_with_enabled(&self, en: bool) {
        // 0x10 - SEVONPEND
        unsafe { (&*SCB::PTR).scr.modify(|r| if en { r | 0x10 } else { r & !0x10 }) }
    }
}
impl Timer {
    #[inline]
    pub(crate) fn new(c: &Clock) -> Timer {
        let v: SYST = unsafe { zeroed() };
        unsafe { v.csr.modify(|r| r | 0x4) };
        let r = unsafe { RESETS::steal() };
        // Reset Alarms and Timers
        r.reset().modify(|_, r| r.timer().set_bit());
        r.reset().modify(|_, r| r.timer().clear_bit());
        while r.reset_done().read().timer().bit_is_clear() {
            nop();
        }
        Timer {
            clk:  v,
            int:  unsafe { TIMER::steal() },
            freq: c.freq,
        }
    }

    pub fn sleep_ms(&self, v: u32) {
        let mut m = v;
        while m > 0x418937 {
            self.sleep_us(0xFFFFFED8);
            m = match m.checked_sub(0x418937) {
                Some(i) => i,
                None => return,
            }
        }
        self.sleep_us(m * 1000);
    }
    pub fn sleep_us(&self, v: u32) {
        let t = (v as u64) * ((self.freq as u64) / 1_000_000);
        let c = unsafe { t.unchecked_shr(24) };
        if c > 0 {
            unsafe {
                self.clk.rvr.write(0xFFFFFF);
                self.clk.cvr.write(0);
                self.clk.csr.modify(|r| r | 0x1);
            }
            for _ in 0..c {
                while self.clk.csr.read() & 0x10000 == 0 {
                    nop();
                }
                nop();
            }
        }
        let t = (t & 0xFFFFFF) as u32;
        if t > 1 {
            unsafe {
                self.clk.rvr.write(t - 1);
                self.clk.cvr.write(0);
                self.clk.csr.modify(|r| r | 0x1);
            }
            while self.clk.csr.read() & 0x10000 == 0 {
                nop();
            }
        }
        unsafe { self.clk.csr.modify(|r| r & !0x1) }
    }
    pub fn current_tick(&self) -> u64 {
        let mut v = self.int.timerawh().read().bits();
        loop {
            let (l, h) = (
                self.int.timerawl().read().bits(),
                self.int.timerawh().read().bits(),
            );
            if v == h {
                return unsafe { (h as u64).unchecked_shl(32) | l as u64 };
            }
            v = h;
        }
    }
}

impl Clone for Timer {
    #[inline]
    fn clone(&self) -> Timer {
        Timer {
            clk:  unsafe { zeroed() },
            int:  unsafe { TIMER::steal() },
            freq: self.freq,
        }
    }
}

#[inline]
fn setup_xosc() -> XOSC {
    let v = unsafe { XOSC::steal() };
    v.ctrl().write(|r| unsafe { r.freq_range().bits(0xAA0) });
    // Setup our frequency.
    // We're using the default 12MHz.
    v.startup()
        .write(|r| unsafe { r.delay().bits((FREQ_XOSC / 256_000).saturating_mul(64) as u16) });
    // Enable the XOSC.
    v.ctrl().write(|r| r.enable().enable());
    // Wait for it to be stable.
    while v.status().read().stable().bit_is_clear() {
        nop();
    }
    v
}
#[inline]
fn rosc_reset(rosc: &ROSC) {
    rosc_set_div(rosc, 1);
    rosc.ctrl().write(|r| unsafe { r.freq_range().bits(0xFA4) });
    rosc_write_freq(rosc, &[0, 0, 0, 0, 0, 0, 0, 0]);
}
#[inline]
fn setup_per(clocks: &CLOCKS) {
    clocks.clk_peri_ctrl().modify(|_, r| r.enable().clear_bit());
    while clocks.clk_peri_ctrl().read().enable().bit_is_set() {
        nop();
    }
    delay(100);
    clocks
        .clk_peri_ctrl()
        .modify(|_, r| unsafe { r.auxsrc().bits(0).enable().set_bit() });
    while clocks.clk_peri_ctrl().read().enable().bit_is_clear() {
        nop();
    }
}
#[inline]
fn setup_sys(clocks: &CLOCKS) {
    if clocks.clk_sys_div().read().bits() < DIV {
        clocks.clk_sys_div().modify(|_, r| unsafe { r.bits(DIV) });
    }
    clocks.clk_sys_ctrl().modify(|_, r| r.src().clear_bit());
    while clocks.clk_sys_selected().read().bits() != 0x1 {
        nop();
    }
    clocks.clk_sys_ctrl().modify(|_, r| unsafe { r.auxsrc().bits(0x2) });
    clocks.clk_sys_ctrl().modify(|_, r| r.src().clear_bit());
    while clocks.clk_sys_selected().read().bits() != 0x1 {
        nop();
    }
    clocks.clk_sys_div().modify(|_, r| unsafe { r.bits(DIV) });
}
fn rosc_drive(rosc: &ROSC) -> bool {
    let mut s = [0u8; 8];
    for (i, v) in s.iter_mut().enumerate() {
        *v = rosc_state(rosc, i as u8);
    }
    let l = match rosc.ctrl().read().freq_range().bits() {
        0xFA4 => 8,
        0xFA5 => 6,
        0xFA7 => 4,
        0xFA6 => unreachable!(), // Shouldn't happen
        _ => return false,
    };
    unsafe {
        let mut n = 0;
        for (i, v) in s.get_unchecked(0..l).windows(2).enumerate() {
            if *v.get_unchecked(1) < *v.get_unchecked(0) {
                n = i + 1;
                break;
            }
        }
        if *s.get_unchecked(n) < 3 {
            s.get_unchecked_mut(n).add_assign(1);
            let m = s.get_unchecked(0..l).iter().min().map(|v| *v).unwrap_or(0);
            for v in s.get_unchecked_mut(l..).iter_mut() {
                *v = m;
            }
            rosc_write_freq(rosc, &s);
            return true;
        }
    }
    false
}
#[inline]
fn rosc_range(rosc: &ROSC) -> bool {
    match rosc.ctrl().read().freq_range().bits() {
        0xFA6 | 0xFA7 => return false,
        0xFA5 => {
            rosc.ctrl().write(|r| unsafe { r.freq_range().bits(0xFA7) });
            rosc_write_freq(rosc, &[0, 0, 0, 0, 0, 0, 0, 0]);
        },
        0xFA4 => {
            rosc.ctrl().write(|r| unsafe { r.freq_range().bits(0xFA5) });
            rosc_write_freq(rosc, &[0, 0, 0, 0, 0, 0, 0, 0]);
        },
        _ => {
            rosc.ctrl().write(|r| unsafe { r.freq_range().bits(0xFA4) });
            rosc_write_freq(rosc, &[0, 0, 0, 0, 0, 0, 0, 0]);
        },
    }
    true
}
#[inline]
fn setup_powersave(clocks: &CLOCKS) {
    clocks.clk_usb_ctrl().modify(|_, r| r.enable().clear_bit());
    clocks.clk_adc_ctrl().modify(|_, r| r.enable().clear_bit());
    clocks.clk_gpout0_ctrl().modify(|_, r| r.enable().clear_bit());
    clocks.clk_gpout1_ctrl().modify(|_, r| r.enable().clear_bit());
    clocks.clk_gpout2_ctrl().modify(|_, r| r.enable().clear_bit());
    clocks.clk_gpout3_ctrl().modify(|_, r| r.enable().clear_bit());
    // Disable PLLs.
    let u = unsafe { PLL_USB::steal() };
    u.cs().write(|r| r.bypass().set_bit());
    u.pwr()
        .write(|r| r.pd().set_bit().dsmpd().set_bit().postdivpd().set_bit().vcopd().set_bit());
    let p = unsafe { PLL_SYS::steal() };
    p.cs().write(|r| r.bypass().set_bit());
    p.pwr()
        .write(|r| r.pd().set_bit().dsmpd().set_bit().postdivpd().set_bit().vcopd().set_bit());
    // Enable DEEP sleep.
    unsafe { (&*SCB::PTR).scr.modify(|r| r | 0x4) }
}
#[inline]
fn rosc_read(clocks: &CLOCKS) -> u32 {
    while clocks.fc0_status().read().running().bit_is_set() {
        nop();
    }
    clocks.fc0_ref_khz().write(|r| unsafe { r.fc0_ref_khz().bits(0x2EE0) });
    clocks.fc0_interval().write(|r| unsafe { r.fc0_interval().bits(0xA) });
    clocks.fc0_min_khz().write(|r| unsafe { r.fc0_min_khz().bits(0) });
    clocks.fc0_max_khz().write(|r| unsafe { r.fc0_max_khz().bits(0x1FFFFFF) });
    clocks.fc0_src().write(|r| unsafe { r.fc0_src().bits(0x3) });
    while clocks.fc0_status().read().done().bit_is_clear() {
        nop();
    }
    clocks.fc0_result().read().khz().bits() * 1_000
}
#[inline]
fn rosc_set_div(rosc: &ROSC, v: u32) {
    rosc.div()
        .write(|r| unsafe { r.bits(0xAA0 + if v == 0x20 { 0 } else { v }) });
}
#[inline]
fn rosc_state(rosc: &ROSC, v: u8) -> u8 {
    match v {
        0 => rosc.freqa().read().ds0().bits(),
        1 => rosc.freqa().read().ds1().bits(),
        2 => rosc.freqa().read().ds2().bits(),
        3 => rosc.freqa().read().ds3().bits(),
        4 => rosc.freqb().read().ds4().bits(),
        5 => rosc.freqb().read().ds5().bits(),
        6 => rosc.freqb().read().ds6().bits(),
        7 => rosc.freqb().read().ds7().bits(),
        _ => unsafe { unreachable_unchecked() },
    }
}
#[inline]
fn setup_ref(clocks: &CLOCKS, xosc: bool) {
    if clocks.clk_ref_div().read().bits() < DIV {
        clocks.clk_ref_div().modify(|_, r| unsafe { r.bits(DIV) });
    }
    if xosc {
        clocks.clk_ref_ctrl().modify(|_, r| unsafe { r.src().bits(0x2) });
        while clocks.clk_ref_selected().read().bits() != 0x4 {
            nop();
        }
    } else {
        clocks.clk_ref_ctrl().modify(|_, r| unsafe { r.src().bits(0) });
        while clocks.clk_ref_selected().read().bits() != 0x1 {
            nop();
        }
    }
    clocks.clk_ref_div().modify(|_, r| unsafe { r.bits(DIV) })
}
#[inline]
fn rosc_write_freq(rosc: &ROSC, v: &[u8; 8]) {
    let mut a = 0x96960000u32;
    let (x, y) = unsafe { v.split_at_unchecked(4) };
    for (i, v) in x.iter().enumerate() {
        unsafe { a |= ((*v & 0x7) as u32).unchecked_shl(i as u32 * 4) };
    }
    let mut b = 0x96960000u32;
    for (i, v) in y.iter().enumerate() {
        unsafe { b |= ((*v & 0x7) as u32).unchecked_shl(i as u32 * 4) };
    }
    rosc.freqa().write(|r| unsafe { r.bits(a) });
    rosc.freqb().write(|r| unsafe { r.bits(b) });
}
fn setup_rosc(clocks: &CLOCKS, freq: u32) -> (u32, u32) {
    let v = unsafe { ROSC::steal() };
    // Make sure the ROSC is enabled and stable first.
    v.ctrl().write(|r| r.enable().enable());
    while v.status().read().stable().bit_is_clear() {
        nop();
    }
    // Set the XOSC as the Reference clock for now as we mess with the ROSC.
    // If we don't do this, the ROSC won't update.
    setup_ref(&clocks, true);
    v.ctrl().write(|r| r.enable().disable());
    // Wait for spindown of the ROSC.
    while v.status().read().stable().bit_is_set() || v.status().read().enabled().bit_is_set() {
        nop();
    }
    // Restart the ROSC to free it.
    v.ctrl().write(|r| r.enable().enable());
    while v.status().read().stable().bit_is_clear() {
        nop();
    }
    // Enable the Phase-shifted output.
    v.phase().write(|r| r.enable().set_bit());
    // Tune the ROSC to get a good freqency value.
    rosc_tune(&v, clocks, freq)
}
fn setup_rtc(clocks: &CLOCKS, clk_freq: u32, freq: u32) -> RTC {
    // BUG(sf): RTC clock skews a bit after a period of time in a linear path.
    //          This is potentially due to the system clock frequency?
    let f = ((clk_freq as f32 / (FREQ_RTC as f32)) * 100f32) as u32;
    let d = unsafe { (f / 100).unchecked_shl(8) } | (f % 100);
    if clocks.clk_rtc_div().read().bits() < d {
        clocks.clk_rtc_div().modify(|_, r| unsafe { r.bits(d) });
    }
    clocks.clk_rtc_ctrl().modify(|_, r| r.enable().clear_bit());
    while clocks.clk_rtc_ctrl().read().enable().bit_is_set() {
        nop();
    }
    delay(((clk_freq / freq) + 1) * 3);
    clocks.clk_rtc_ctrl().modify(|_, r| unsafe { r.auxsrc().bits(0x2) });
    clocks.clk_rtc_div().modify(|_, r| unsafe { r.bits(d) });
    clocks.clk_rtc_ctrl().modify(|_, r| r.enable().set_bit());
    while clocks.clk_rtc_ctrl().read().enable().bit_is_clear() {
        nop();
    }
    // Nudge the RTC.
    // clocks.clk_rtc_ctrl().modify(|_, r| r.nudge().set_bit());
    let (v, r) = (unsafe { RTC::steal() }, unsafe { RESETS::steal() });
    // Reset RTC first.
    r.reset().modify(|_, r| r.rtc().set_bit());
    r.reset().modify(|_, r| r.rtc().clear_bit());
    while r.reset_done().read().rtc().bit_is_clear() {
        nop();
    }
    v.ctrl().modify(|_, r| r.rtc_enable().clear_bit());
    // Set the status of the RTC to disabled for configuration.
    while v.ctrl().read().rtc_active().bit_is_set() {
        nop();
    }
    // Set the initial date to Sunday 1/1/0 00:00:00.
    v.setup_0()
        .write(|r| unsafe { r.year().bits(0).month().bits(1).day().bits(1) });
    v.setup_1()
        .write(|r| unsafe { r.dotw().bits(1).hour().bits(0).min().bits(0).sec().bits(0) });
    // Set our ticking frequency.
    v.clkdiv_m1().write(|r| unsafe { r.bits(freq.saturating_sub(2)) });
    v.ctrl()
        .write(|r| r.force_notleapyear().clear_bit().load().set_bit().rtc_enable().set_bit());
    // Start the RTC and load it.
    while v.ctrl().read().rtc_active().bit_is_clear() {
        nop();
    }
    v
}
fn rosc_tune(rosc: &ROSC, clocks: &CLOCKS, target: u32) -> (u32, u32) {
    rosc_reset(rosc);
    let mut m;
    let (mut d, mut t) = (1u32, 1u32);
    // 't' is a seed base that we'll compound together from all the frequencies
    // read so we have a more volatile number.
    loop {
        m = rosc_read(clocks);
        t = t.saturating_add(m);
        if m > target {
            d += 1;
            rosc_set_div(rosc, d);
        } else {
            break;
        }
    }
    loop {
        m = rosc_read(clocks);
        t = t.saturating_add(m);
        if m > target {
            break;
        }
        if !rosc_drive(rosc) {
            if !rosc_range(rosc) {
                break;
            }
        }
    }
    (m, t)
}
