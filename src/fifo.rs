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

use core::option::Option::{self, None, Some};

use crate::asm::{nop, sev, wfe};
use crate::pac::SIO;

pub struct Fifo(SIO);

impl Fifo {
    #[inline(always)]
    pub fn get() -> Fifo {
        Fifo(unsafe { SIO::steal() })
    }

    #[inline]
    pub fn drain(&mut self) {
        while self.0.fifo_st().read().vld().bit_is_set() {
            let _ = self.0.fifo_rd().read().bits();
        }
    }
    #[inline]
    pub fn status(&mut self) -> u32 {
        self.0.fifo_st().read().bits()
    }
    pub fn read_block(&mut self) -> u32 {
        while self.0.fifo_st().read().vld().bit_is_clear() {
            wfe();
        }
        self.0.fifo_rd().read().bits()
    }
    #[inline]
    pub fn is_read_ready(&self) -> bool {
        self.0.fifo_st().read().vld().bit_is_set()
    }
    #[inline]
    pub fn is_write_ready(&self) -> bool {
        self.0.fifo_st().read().rdy().bit_is_set()
    }
    #[inline]
    pub fn read(&mut self) -> Option<u32> {
        if self.0.fifo_st().read().vld().bit_is_set() {
            Some(self.0.fifo_rd().read().bits())
        } else {
            None
        }
    }
    pub fn write_block(&mut self, v: u32) {
        while self.0.fifo_st().read().rdy().bit_is_clear() {
            nop();
        }
        self.0.fifo_wr().write(|r| unsafe { r.bits(v) });
        sev()
    }
    #[inline]
    pub fn write(&mut self, v: u32) -> bool {
        if self.0.fifo_st().read().rdy().bit_is_clear() {
            return false;
        }
        self.0.fifo_wr().write(|r| unsafe { r.bits(v) });
        sev();
        true
    }
}
