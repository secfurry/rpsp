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
use core::default::Default;
use core::ops::{Deref, DerefMut};

use crate::Board;

pub struct Rand(u32);
pub struct RandMut(UnsafeCell<Rand>);

impl Rand {
    #[inline(always)]
    pub const fn empty() -> Rand {
        Rand(0u32)
    }
    #[inline(always)]
    pub const fn with_seed(seed: u32) -> Rand {
        Rand(seed)
    }

    #[inline(always)]
    pub fn new() -> Rand {
        Rand(Board::get().system_clock().seed())
    }

    #[inline(always)]
    pub fn reseed(&mut self) {
        self.0 = Board::get().system_clock().seed()
    }
    #[inline]
    pub fn rand_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x78BD642F);
        let v = (self.0 as u64).wrapping_mul((self.0 ^ 0xA0B428DB) as u64);
        (v.wrapping_shr(32) ^ v) as u32
    }
    #[inline(always)]
    pub fn rand(&mut self) -> [u8; 4] {
        self.rand_u32().to_be_bytes()
    }
    #[inline(always)]
    pub fn set_seed(&mut self, v: u32) {
        self.0 = v
    }
    #[inline]
    pub fn rand_u32n(&mut self, n: u32) -> u32 {
        if n == 0 {
            return self.rand_u32();
        }
        if n & (n - 1) == 0 {
            return self.rand_u32() & (n - 1);
        }
        let m = ((1 << 31) - 1 - (1 << 31) % n as u32) as u32;
        let mut v = self.rand_u32();
        while v > m {
            v = self.rand_u32();
        }
        ((v as u64 * n as u64) >> 32) as u32
    }
}
impl RandMut {
    #[inline(always)]
    pub const fn empty() -> RandMut {
        RandMut(UnsafeCell::new(Rand::empty()))
    }
    #[inline(always)]
    pub const fn with_seed(seed: u32) -> RandMut {
        RandMut(UnsafeCell::new(Rand::with_seed(seed)))
    }

    #[inline(always)]
    pub fn new() -> RandMut {
        RandMut(UnsafeCell::new(Rand::new()))
    }

    #[inline(always)]
    pub fn reseed(&self) {
        unsafe { &mut *self.0.get() }.reseed()
    }
    #[inline(always)]
    pub fn rand_u32(&self) -> u32 {
        unsafe { &mut *self.0.get() }.rand_u32()
    }
    #[inline(always)]
    pub fn rand(&self) -> [u8; 4] {
        unsafe { &mut *self.0.get() }.rand()
    }
    #[inline(always)]
    pub fn set_seed(&mut self, v: u32) {
        unsafe { &mut *self.0.get() }.set_seed(v);
    }
    #[inline(always)]
    pub fn rand_u32n(&self, n: u32) -> u32 {
        unsafe { &mut *self.0.get() }.rand_u32n(n)
    }
}

impl Clone for Rand {
    #[inline(always)]
    fn clone(&self) -> Rand {
        Rand(self.0)
    }
}
impl Default for Rand {
    #[inline(always)]
    fn default() -> Rand {
        Rand::new()
    }
}

impl Clone for RandMut {
    #[inline(always)]
    fn clone(&self) -> RandMut {
        RandMut(UnsafeCell::new(self.deref().clone()))
    }
}
impl Deref for RandMut {
    type Target = Rand;

    #[inline(always)]
    fn deref(&self) -> &Rand {
        unsafe { &*self.0.get() }
    }
}
impl Default for RandMut {
    #[inline(always)]
    fn default() -> RandMut {
        RandMut::new()
    }
}
impl DerefMut for RandMut {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Rand {
        unsafe { &mut *self.0.get() }
    }
}
