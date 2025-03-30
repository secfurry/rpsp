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
#![cfg(feature = "cyw")]

extern crate core;

use core::clone::Clone;
use core::cmp::Ord;
use core::fmt::Write;
use core::result::Result::{self, Err, Ok};
use core::slice::{from_raw_parts, from_raw_parts_mut};

use crate::Board;
use crate::clock::Timer;
use crate::cyw::CywError;
use crate::pin::gpio::Output;
use crate::pin::{Pin, PinDirection, PinID, PinState};
use crate::pio::state::{Running, Stopped};
use crate::pio::{Config, Machine, Pio, PioID, Program, Rx, Shift, Slot, State, Tx};

pub struct Device {
    t:      Timer,
    sm:     Machine<Running>,
    tx:     Tx<u32>,
    rx:     Rx<u32>,
    bp:     u32,
    cs:     Pin<Output>,
    pwr:    Pin<Output>,
    offset: u8,
    status: u32,
}

impl Device {
    #[inline]
    pub fn new(p: &Board, offset: u8, sm: State<'_, Stopped>, pwr: PinID, cs: PinID) -> Device {
        let m = unsafe { sm.start_paused().uncouple() };
        Device {
            offset,
            t: p.timer().clone(),
            tx: m.tx_u32(),
            rx: m.rx_u32(),
            bp: 0u32,
            cs: Pin::get(&p, PinID::Pin25).output(true),
            pwr: Pin::get(&p, PinID::Pin23).output(false),
            status: 0u32,
            sm: m,
        }
    }

    pub fn init(&mut self, bt: bool) -> Result<(), CywError> {
        self.pwr.low();
        self.t.sleep_ms(20);
        self.pwr.high();
        self.t.sleep_ms(250);
        let mut s = 0u8;
        while self.read_swap32(0, 0x14) != 0xFEEDBEADu32 {
            if s > 250 {
                self.pwr.low();
                return Err(CywError::InitFailure);
            }
            s = s.saturating_add(1);
        }
        self.write_swap32(0, 0x18, 0xC0FFEBAEu32);
        if self.read_swap32(0, 0x18) != 0xC0FFEBAEu32 {
            self.pwr.low();
            return Err(CywError::InitFailure);
        }
        let bus = self.read_swap32(0, 0);
        self.write_swap32(0, 0, 0x304B1);
        let bus_reg = self.read8(0, 0);
        if self.read_swap32(0, 0x14) != 0xFEEDBEADu32 {
            self.pwr.low();
            return Err(CywError::InitFailure);
        }
        if self.read_swap32(0, 0x18) != 0xC0FFEBAEu32 {
            self.pwr.low();
            return Err(CywError::InitFailure);
        }
        self.write8(0, 0x1D, 0x4);
        self.write8(0, 0x4, 0x99);
        self.write16(0, 0x6, 0xBE | if bt { 0x2000 } else { 0 });
        Ok(())
    }
    #[inline]
    pub fn bp_set_window(&mut self, v: u32) {
        let n = v & !0x7FFF;
        if (n >> 0x18) as u8 != (self.bp >> 0x18) as u8 {
            self.write8(1, 0x1000C, (n >> 0x18) as u8);
        }
        if (n >> 0x10) as u8 != (self.bp >> 0x10) as u8 {
            self.write8(1, 0x1000B, (n >> 0x10) as u8);
        }
        if (n >> 0x8) as u8 != (self.bp >> 0x8) as u8 {
            self.write8(1, 0x1000A, (n >> 0x8) as u8);
        }
        self.bp = v
    }
    #[inline(always)]
    pub fn write_wlan(&mut self, b: &[u32]) {
        self.cmd_write(0xE0000000 | (b.len() as u32 * 4), b)
    }
    #[inline(always)]
    pub fn read_bp8(&mut self, addr: u32) -> u8 {
        self.read_bp(addr, 1) as u8
    }
    #[inline(always)]
    pub fn read_bp16(&mut self, addr: u32) -> u16 {
        self.read_bp(addr, 2) as u16
    }
    #[inline(always)]
    pub fn read_bp32(&mut self, addr: u32) -> u32 {
        self.read_bp(addr, 4)
    }
    #[inline(always)]
    pub fn write_bp8(&mut self, addr: u32, v: u8) {
        self.write_bp(addr, 1, v as u32)
    }
    #[inline(always)]
    pub fn write_bp16(&mut self, addr: u32, v: u16) {
        self.write_bp(addr, 2, v as u32)
    }
    #[inline(always)]
    pub fn write_bp32(&mut self, addr: u32, v: u32) {
        self.write_bp(addr, 4, v)
    }
    #[inline(always)]
    pub fn read8(&mut self, func: u32, addr: u32) -> u8 {
        self.read(func, addr, 1) as u8
    }
    #[inline]
    pub fn read_wlan(&mut self, len: u32, b: &mut [u32]) {
        let n = (len as usize + 3) / 4;
        self.cmd_read(0x60000000 | len, &mut b[0..n])
    }
    #[inline(always)]
    pub fn read16(&mut self, func: u32, addr: u32) -> u16 {
        self.read(func, addr, 2) as u16
    }
    #[inline(always)]
    pub fn read32(&mut self, func: u32, addr: u32) -> u32 {
        self.read(func, addr, 4)
    }
    #[inline(always)]
    pub fn write8(&mut self, func: u32, addr: u32, v: u8) {
        self.write(func, addr, 1, v as u32)
    }
    #[inline]
    pub fn read_bp(&mut self, addr: u32, len: u32) -> u32 {
        self.bp_set_window(addr);
        self.read(
            1,
            (addr & 0x7FFF) | if len == 4 { 0x08000u32 } else { 0u32 },
            len,
        )
    }
    pub fn write_bp_bytes(&mut self, addr: u32, b: &[u8]) {
        let mut w = [0u32; 0x10];
        let (s, mut i, mut a) = (b.len(), 0usize, addr);
        while i < s {
            let o = a & 0x7FFFu32;
            let n = s.saturating_sub(i).min(0x40).min(0x8000usize - o as usize);
            self.bp_set_window(a);
            unsafe {
                from_raw_parts_mut(w.as_mut_ptr() as *mut u8, 0x40)[0..n].copy_from_slice(&b[i..i + n]);
            }
            self.cmd_write(
                0xE0000000 | ((o & 0x1FFFF) << 0xB) | n as u32,
                &w[0..(n + 3) / 4],
            );
            a = a.saturating_add(n as u32);
            i = i.saturating_add(n);
        }
    }
    #[inline]
    pub fn write_bp(&mut self, addr: u32, len: u32, v: u32) {
        self.bp_set_window(addr);
        self.write(
            1,
            (addr & 0x7FFF) | if len == 4 { 0x08000u32 } else { 0u32 },
            len,
            v,
        )
    }
    #[inline(always)]
    pub fn write16(&mut self, func: u32, addr: u32, v: u16) {
        self.write(func, addr, 2, v as u32)
    }
    #[inline(always)]
    pub fn write32(&mut self, func: u32, addr: u32, v: u32) {
        self.write(func, addr, 4, v)
    }
    pub fn read_bp_bytes(&mut self, addr: u32, b: &mut [u8]) {
        let mut w = [0u32; 0x11];
        let (s, mut i, mut a) = (b.len(), 0usize, addr);
        while i < s {
            let o = a & 0x7FFFu32;
            let n = s.saturating_sub(i).min(0x40).min(0x8000usize - o as usize);
            self.bp_set_window(a);
            self.cmd_read(
                0x60000000 | ((o & 0x1FFFF) << 0xB) | n as u32,
                &mut w[0..(n + 3) / 4 + 1],
            );
            unsafe {
                b[i..i + n].copy_from_slice(&(from_raw_parts(w[1..].as_ptr() as *const u8, 0x40))[0..n]);
            }
            a = a.saturating_add(n as u32);
            i = i.saturating_add(n);
        }
    }
    #[inline]
    pub fn read_swap32(&mut self, func: u32, addr: u32) -> u32 {
        let mut b = [0u32; 1];
        self.cmd_read(
            (0x40000004 | (func << 0x1C) | ((addr & 0x1FFFF) << 0xB)).rotate_left(0x10),
            &mut b,
        );
        b[0].rotate_left(0x10)
    }
    #[inline(always)]
    pub fn write_swap32(&mut self, func: u32, addr: u32, v: u32) {
        self.cmd_write(
            (0xC0000004 | (func << 0x1C) | ((addr & 0x1FFFF) << 0xB)).rotate_left(0x10),
            &[v.rotate_left(0x10)],
        )
    }
    #[inline]
    pub fn read(&mut self, func: u32, addr: u32, len: u32) -> u32 {
        let v = if func == 1 { 2usize } else { 1usize };
        let mut b = [0u32; 2];
        self.cmd_read(
            0x40000000 | (func << 0x1C) | ((addr & 0x1FFFF) << 0xB) | len,
            &mut b[0..v],
        );
        b[v - 1]
    }
    #[inline(always)]
    pub fn write(&mut self, func: u32, addr: u32, len: u32, v: u32) {
        self.cmd_write(
            0xC0000000 | (func << 0x1C) | ((addr & 0x1FFFF) << 0xB) | len,
            &[v],
        )
    }

    #[inline]
    fn prepare(&mut self, r: u32, w: u32) {
        self.sm.set_state(false);
        self.sm.set_x(w);
        self.sm.set_y(r);
        unsafe {
            self.sm.exec(0xE081);
            self.sm.jump(self.offset);
        }
        self.sm.set_state(true);
    }
    fn cmd_write(&mut self, cmd: u32, b: &[u32]) {
        self.cs.low();
        self.prepare(0x1F, ((b.len() + 1) as u32 * 0x20).saturating_sub(1));
        self.tx.write(cmd);
        for i in b {
            self.tx.write(*i)
        }
        self.status = self.rx.read();
        self.cs.high()
    }
    fn cmd_read(&mut self, cmd: u32, b: &mut [u32]) {
        self.cs.low();
        self.prepare((b.len() as u32 * 0x20) + 0x1F, 0x1F);
        self.tx.write(cmd);
        for i in b {
            *i = self.rx.read()
        }
        self.status = self.rx.read();
        self.cs.high()
    }
}

#[inline(always)]
fn word(op: bool, inc: bool, f: u32, a: u32, n: u32) -> u32 {
    (if op { 1 } else { 0 } << 0x1F) | (if inc { 1 } else { 0 } << 0x1E) | (f) << 0x1C | ((a & 0x1FFFF) << 0xB) | (n)
}
