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
use core::cmp::Ord;
use core::default::Default;
use core::ops::{Deref, DerefMut};
use core::ptr::copy_nonoverlapping;

use crate::Board;

pub struct Rand(u32);
pub struct RandMut(UnsafeCell<Rand>);

impl Rand {
    #[inline]
    pub const fn empty() -> Rand {
        Rand(0u32)
    }
    #[inline]
    pub const fn with_seed(seed: u32) -> Rand {
        Rand(seed)
    }

    #[inline]
    pub fn new() -> Rand {
        Rand(Board::get().system_clock().seed())
    }

    #[inline]
    pub fn reseed(&mut self) {
        self.0 = Board::get().system_clock().seed()
    }
    #[inline]
    pub fn rand_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x78BD642F);
        let v = (self.0 as u64).wrapping_mul((self.0 ^ 0xA0B428DB) as u64);
        (v.wrapping_shr(32) ^ v) as u32
    }
    #[inline]
    pub fn rand(&mut self) -> [u8; 4] {
        self.rand_u32().to_be_bytes()
    }
    #[inline]
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
        let m = 0x7FFFFFFF - (0x80000000 % n as u32) as u32;
        let mut v = self.rand_u32();
        while v > m {
            v = self.rand_u32();
        }
        unsafe { (v as u64 * n as u64).unchecked_shr(32) as u32 }
    }
    pub fn read_into(&mut self, b: &mut [u8]) -> usize {
        if b.len() < 4 {
            let v = self.rand();
            let i = v.len().min(b.len());
            unsafe { copy_nonoverlapping(v.as_ptr(), b.as_mut_ptr(), i) };
            return i;
        }
        let (c, r) = b.as_chunks_mut::<4>();
        let mut n = 0;
        if c.len() > 0 {
            for i in c {
                let v = self.rand();
                unsafe { copy_nonoverlapping(v.as_ptr(), i.as_mut_ptr(), 4) };
                n += 4;
            }
        }
        if r.len() > 0 {
            let v = self.rand();
            let i = v.len().min(r.len());
            unsafe { copy_nonoverlapping(v.as_ptr(), r.as_mut_ptr(), i) };
            n += i;
        }
        n
    }
}
impl RandMut {
    #[inline]
    pub const fn empty() -> RandMut {
        RandMut(UnsafeCell::new(Rand::empty()))
    }
    #[inline]
    pub const fn with_seed(seed: u32) -> RandMut {
        RandMut(UnsafeCell::new(Rand::with_seed(seed)))
    }

    #[inline]
    pub fn new() -> RandMut {
        RandMut(UnsafeCell::new(Rand::new()))
    }

    #[inline]
    pub fn reseed(&self) {
        unsafe { &mut *self.0.get() }.reseed()
    }
    #[inline]
    pub fn rand_u32(&self) -> u32 {
        unsafe { &mut *self.0.get() }.rand_u32()
    }
    #[inline]
    pub fn rand(&self) -> [u8; 4] {
        unsafe { &mut *self.0.get() }.rand()
    }
    #[inline]
    pub fn set_seed(&mut self, v: u32) {
        unsafe { &mut *self.0.get() }.set_seed(v);
    }
    #[inline]
    pub fn rand_u32n(&self, n: u32) -> u32 {
        unsafe { &mut *self.0.get() }.rand_u32n(n)
    }
    #[inline]
    pub fn read_into(&mut self, b: &mut [u8]) -> usize {
        unsafe { &mut *self.0.get() }.read_into(b)
    }
}

impl Clone for Rand {
    #[inline]
    fn clone(&self) -> Rand {
        Rand(self.0)
    }
}
impl Default for Rand {
    #[inline]
    fn default() -> Rand {
        Rand::new()
    }
}

impl Clone for RandMut {
    #[inline]
    fn clone(&self) -> RandMut {
        RandMut(UnsafeCell::new(self.deref().clone()))
    }
}
impl Deref for RandMut {
    type Target = Rand;

    #[inline]
    fn deref(&self) -> &Rand {
        unsafe { &*self.0.get() }
    }
}
impl Default for RandMut {
    #[inline]
    fn default() -> RandMut {
        RandMut::new()
    }
}
impl DerefMut for RandMut {
    #[inline]
    fn deref_mut(&mut self) -> &mut Rand {
        unsafe { &mut *self.0.get() }
    }
}
