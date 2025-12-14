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
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::marker::{Copy, PhantomData};
use core::option::Option;

use crate::pac::pio0::RegisterBlock;
use crate::pio::{Pio, Request};
use crate::write_reg;

pub struct Interrupt<'a> {
    irq: Request,
    dev: *const RegisterBlock,
    _p:  PhantomData<&'a Pio>,
}
pub struct InterruptState(u32);

#[repr(u8)]
pub enum InterruptIndex {
    Num0 = 0u8,
    Num1 = 1u8,
    Num2 = 2u8,
    Num3 = 3u8,
}

impl InterruptState {
    #[inline]
    pub fn sm0(&self) -> bool {
        self.0 & 0x100 != 0
    }
    #[inline]
    pub fn sm1(&self) -> bool {
        self.0 & 0x200 != 0
    }
    #[inline]
    pub fn sm2(&self) -> bool {
        self.0 & 0x400 != 0
    }
    #[inline]
    pub fn sm3(&self) -> bool {
        self.0 & 0x800 != 0
    }
    #[inline]
    pub fn rx_not_empty_sm0(&self) -> bool {
        self.0 & 0x1 != 0
    }
    #[inline]
    pub fn rx_not_empty_sm1(&self) -> bool {
        self.0 & 0x2 != 0
    }
    #[inline]
    pub fn rx_not_empty_sm2(&self) -> bool {
        self.0 & 0x4 != 0
    }
    #[inline]
    pub fn rx_not_empty_sm3(&self) -> bool {
        self.0 & 0x8 != 0
    }
    #[inline]
    pub fn tx_not_empty_sm0(&self) -> bool {
        self.0 & 0x10 != 0
    }
    #[inline]
    pub fn tx_not_empty_sm1(&self) -> bool {
        self.0 & 0x20 != 0
    }
    #[inline]
    pub fn tx_not_empty_sm2(&self) -> bool {
        self.0 & 0x40 != 0
    }
    #[inline]
    pub fn tx_not_empty_sm3(&self) -> bool {
        self.0 & 0x80 != 0
    }
}
impl<'a> Interrupt<'a> {
    #[inline]
    pub fn raw(&self) -> InterruptState {
        InterruptState(self.ptr().intr().read().bits())
    }
    #[inline]
    pub fn state(&self) -> InterruptState {
        InterruptState(self.ptr().sm_irq(self.irq as usize).irq_ints().read().bits())
    }
    #[inline]
    pub fn set_interrupt(&self, i: InterruptIndex, en: bool) {
        write_reg(
            self.ptr().sm_irq(self.irq as usize).irq_inte().as_ptr(),
            unsafe { 1u32.unchecked_shl(i as u32 + 8) },
            !en,
        )
    }
    #[inline]
    pub fn set_interrupt_state(&self, i: InterruptIndex, en: bool) {
        write_reg(
            self.ptr().sm_irq(self.irq as usize).irq_intf().as_ptr(),
            unsafe { 1u32.unchecked_shl(i as u32 + 8) },
            !en,
        )
    }

    #[inline]
    pub(super) fn new(p: &'a Pio, i: Request) -> Interrupt<'a> {
        Interrupt {
            dev: p.dev,
            irq: i,
            _p:  PhantomData,
        }
    }

    #[inline]
    fn ptr(&self) -> &RegisterBlock {
        unsafe { &*self.dev }
    }
}

impl Eq for InterruptState {}
impl Ord for InterruptState {
    #[inline]
    fn cmp(&self, other: &InterruptState) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Copy for InterruptState {}
impl Clone for InterruptState {
    #[inline]
    fn clone(&self) -> InterruptState {
        InterruptState(self.0)
    }
}
impl PartialEq for InterruptState {
    #[inline]
    fn eq(&self, other: &InterruptState) -> bool {
        self.0 == other.0
    }
}
impl PartialOrd for InterruptState {
    #[inline]
    fn partial_cmp(&self, other: &InterruptState) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Copy for InterruptIndex {}
impl Clone for InterruptIndex {
    #[inline]
    fn clone(&self) -> InterruptIndex {
        *self
    }
}
