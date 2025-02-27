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
use core::marker::{Copy, PhantomData};
use core::option::Option::{self, None, Some};
use core::ptr::{read_volatile, write_volatile};
use core::result::Result::{self, Err, Ok};

use crate::asm::nop;
use crate::dma::{DmaReader, DmaWord, DmaWriter};
use crate::pac::PIO0;
use crate::pac::pio0::{RXF, RegisterBlock, TXF};
use crate::pio::{Machine, PioError, PioStateOccupied, Slot};
use crate::write_reg;

#[repr(u8)]
pub enum Request {
    Irq0 = 0u8,
    Irq1 = 1u8,
}

pub struct Rx<T: PioIO> {
    ptr: *const RXF,
    dev: *const RegisterBlock,
    idx: Slot,
    _p:  PhantomData<T>,
}
pub struct Tx<T: PioIO> {
    ptr: *const TXF,
    dev: *const RegisterBlock,
    idx: Slot,
    _p:  PhantomData<T>,
}

pub trait PioIO {}

impl Rx<u8> {
    #[inline(always)]
    pub fn read(&mut self) -> u8 {
        self.read_raw() as u8
    }
    #[inline(always)]
    pub fn try_read(&mut self) -> Option<u8> {
        self.try_read_raw().map(|v| v as u8)
    }
}
impl Tx<u8> {
    #[inline(always)]
    pub fn write(&mut self, v: u8) {
        self.write_raw(v as u32)
    }
    #[inline(always)]
    pub fn try_write(&mut self, v: u8) -> Result<(), PioError> {
        self.try_write_raw(v as u32)
    }
}
impl Rx<u16> {
    #[inline(always)]
    pub fn read(&mut self) -> u16 {
        self.read_raw() as u16
    }
    #[inline(always)]
    pub fn try_read(&mut self) -> Option<u16> {
        self.try_read_raw().map(|v| v as u16)
    }
}
impl Tx<u16> {
    #[inline(always)]
    pub fn write(&mut self, v: u16) {
        self.write_raw(v as u32)
    }
    #[inline(always)]
    pub fn try_write(&mut self, v: u16) -> Result<(), PioError> {
        self.try_write_raw(v as u32)
    }
}
impl Rx<u32> {
    #[inline(always)]
    pub fn read(&mut self) -> u32 {
        self.read_raw()
    }
    #[inline(always)]
    pub fn try_read(&mut self) -> Option<u32> {
        self.try_read_raw()
    }
}
impl Tx<u32> {
    #[inline(always)]
    pub fn write(&mut self, v: u32) {
        self.write_raw(v)
    }
    #[inline(always)]
    pub fn try_write(&mut self, v: u32) -> Result<(), PioError> {
        self.try_write_raw(v)
    }
}
impl<T: PioIO> Rx<T> {
    #[inline]
    pub fn dreq(&self) -> u8 {
        (if self.dev == PIO0::PTR { 0x4u8 } else { 0xCu8 }) + self.idx as u8
    }
    #[inline]
    pub fn clear_stalled(&self) {
        self.pio()
            .fdebug()
            .write(|r| unsafe { r.rxstall().bits(1 << self.idx as u8) })
    }
    #[inline]
    pub fn clear_underrun(&self) {
        self.pio()
            .fdebug()
            .write(|r| unsafe { r.rxunder().bits(1 << self.idx as u8) })
    }
    #[inline]
    pub fn is_full(&self) -> bool {
        self.pio().fstat().read().rxfull().bits() & (1 << self.idx as u8) != 0
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pio().fstat().read().rxempty().bits() & (1 << self.idx as u8) != 0
    }
    #[inline]
    pub fn is_stalled(&self) -> bool {
        self.pio().fdebug().read().rxstall().bits() & (1 << self.idx as u8) != 0
    }
    #[inline]
    pub fn is_underrun(&self) -> bool {
        self.pio().fdebug().read().rxunder().bits() & (1 << self.idx as u8) != 0
    }
    #[inline]
    pub fn read_raw(&mut self) -> u32 {
        while self.is_empty() {
            nop();
        }
        unsafe { read_volatile(self.ptr().as_ptr()) }
    }
    #[inline]
    pub fn address(&self) -> *const u32 {
        self.ptr().as_ptr()
    }
    #[inline]
    pub fn set_autopush(&mut self, en: bool) {
        self.pio()
            .sm(self.idx as usize)
            .sm_shiftctrl()
            .write(|r| r.autopush().bit(en));
    }
    #[inline]
    pub fn try_read_raw(&mut self) -> Option<u32> {
        if self.is_empty() { None } else { Some(unsafe { read_volatile(self.ptr().as_ptr()) }) }
    }
    #[inline]
    pub fn set_non_empty_irq(&self, i: Request, en: bool) {
        write_reg(
            self.pio().sm_irq(i as usize).irq_inte().as_ptr(),
            1 << self.idx as u8,
            !en,
        )
    }
    #[inline]
    pub fn set_non_empty_irq_state(&self, i: Request, en: bool) {
        write_reg(
            self.pio().sm_irq(i as usize).irq_intf().as_ptr(),
            1 << self.idx as u8,
            !en,
        )
    }

    #[inline]
    pub(super) fn new<S: PioStateOccupied>(state: &Machine<S>) -> Rx<T> {
        Rx {
            _p:  PhantomData,
            ptr: state.pio().rxf(state.idx as usize) as *const RXF,
            dev: state.pio,
            idx: state.idx,
        }
    }

    #[inline(always)]
    fn ptr(&self) -> &RXF {
        unsafe { &*self.ptr }
    }
    #[inline(always)]
    fn pio(&self) -> &RegisterBlock {
        unsafe { &*self.dev }
    }
}
impl<T: PioIO> Tx<T> {
    #[inline]
    pub fn dreq(&self) -> u8 {
        (if self.dev == PIO0::PTR { 0x0u8 } else { 0x8u8 }) + self.idx as u8
    }
    #[inline]
    pub fn clear_stalled(&self) {
        self.pio()
            .fdebug()
            .write(|r| unsafe { r.txstall().bits(1 << self.idx as u8) })
    }
    #[inline]
    pub fn clear_overrun(&self) {
        self.pio()
            .fdebug()
            .write(|r| unsafe { r.txover().bits(1 << self.idx as u8) })
    }
    #[inline]
    pub fn is_full(&self) -> bool {
        self.pio().fstat().read().txfull().bits() & (1 << self.idx as u8) != 0
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pio().fstat().read().txempty().bits() & (1 << self.idx as u8) != 0
    }
    #[inline]
    pub fn is_overrun(&self) -> bool {
        self.pio().fdebug().read().txover().bits() & (1 << self.idx as u8) != 0
    }
    #[inline]
    pub fn is_stalled(&self) -> bool {
        self.pio().fdebug().read().txstall().bits() & (1 << self.idx as u8) != 0
    }
    #[inline]
    pub fn address(&self) -> *const u32 {
        self.ptr().as_ptr()
    }
    #[inline]
    pub fn write_raw(&mut self, v: u32) {
        while self.is_full() {
            nop();
        }
        unsafe { write_volatile(self.ptr().as_ptr(), v) }
    }
    #[inline]
    pub fn set_autopull(&mut self, en: bool) {
        self.pio()
            .sm(self.idx as usize)
            .sm_shiftctrl()
            .write(|r| r.autopull().bit(en));
    }
    #[inline]
    pub fn set_non_empty_irq(&self, i: Request, en: bool) {
        write_reg(
            self.pio().sm_irq(i as usize).irq_inte().as_ptr(),
            1 << (self.idx as u8 + 4),
            !en,
        )
    }
    #[inline]
    pub fn set_non_empty_irq_state(&self, i: Request, en: bool) {
        write_reg(
            self.pio().sm_irq(i as usize).irq_intf().as_ptr(),
            1 << (self.idx as u8 + 4),
            !en,
        )
    }
    #[inline]
    pub fn try_write_raw(&mut self, v: u32) -> Result<(), PioError> {
        if self.is_full() {
            Err(PioError::WouldBlock)
        } else {
            Ok(unsafe { write_volatile(self.ptr().as_ptr(), v) })
        }
    }

    #[inline]
    pub(super) fn new<S: PioStateOccupied>(state: &Machine<S>) -> Tx<T> {
        Tx {
            _p:  PhantomData,
            ptr: state.pio().txf(state.idx as usize) as *const TXF,
            dev: state.pio,
            idx: state.idx,
        }
    }

    #[inline(always)]
    fn ptr(&self) -> &TXF {
        unsafe { &*self.ptr }
    }
    #[inline(always)]
    fn pio(&self) -> &RegisterBlock {
        unsafe { &*self.dev }
    }
}

impl<T: PioIO + DmaWord> DmaReader<T> for Rx<T> {
    #[inline]
    fn rx_req(&self) -> Option<u8> {
        Some((if self.dev == PIO0::PTR { 0u8 } else { 8u8 }) | self.idx as u8 | 0x4)
    }
    #[inline]
    fn rx_info(&self) -> (u32, u32) {
        (self.ptr().as_ptr() as u32, u32::MAX)
    }
    #[inline]
    fn rx_incremented(&self) -> bool {
        false
    }
}
impl<T: PioIO + DmaWord> DmaWriter<T> for Tx<T> {
    #[inline]
    fn tx_req(&self) -> Option<u8> {
        Some((if self.dev == PIO0::PTR { 0u8 } else { 8u8 }) | self.idx as u8)
    }
    #[inline]
    fn tx_info(&self) -> (u32, u32) {
        (self.ptr().as_ptr() as u32, u32::MAX)
    }
    #[inline]
    fn tx_incremented(&self) -> bool {
        false
    }
}

impl PioIO for u8 {}
impl PioIO for u16 {}
impl PioIO for u32 {}

impl Copy for Request {}
impl Clone for Request {
    #[inline(always)]
    fn clone(&self) -> Request {
        *self
    }
}
