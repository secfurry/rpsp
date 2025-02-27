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

use core::marker::PhantomData;
use core::ops::Drop;
use core::option::Option::{self, None, Some};

use crate::asm::nop;
use crate::pac::SIO;

pub struct Spinlock<const N: u8>(PhantomData<*const ()>);

pub type Spinlock0 = Spinlock<0>;
pub type Spinlock1 = Spinlock<1>;
pub type Spinlock2 = Spinlock<2>;
pub type Spinlock3 = Spinlock<3>;
pub type Spinlock4 = Spinlock<4>;
pub type Spinlock5 = Spinlock<5>;
pub type Spinlock6 = Spinlock<6>;
pub type Spinlock7 = Spinlock<7>;
pub type Spinlock8 = Spinlock<8>;
pub type Spinlock9 = Spinlock<9>;
pub type Spinlock10 = Spinlock<10>;
pub type Spinlock11 = Spinlock<11>;
pub type Spinlock12 = Spinlock<12>;
pub type Spinlock13 = Spinlock<13>;
pub type Spinlock14 = Spinlock<14>;
pub type Spinlock15 = Spinlock<15>;
pub type Spinlock16 = Spinlock<16>;
pub type Spinlock17 = Spinlock<17>;
pub type Spinlock18 = Spinlock<18>;
pub type Spinlock19 = Spinlock<19>;
pub type Spinlock20 = Spinlock<20>;
pub type Spinlock21 = Spinlock<21>;
pub type Spinlock22 = Spinlock<22>;
pub type Spinlock23 = Spinlock<23>;
pub type Spinlock24 = Spinlock<24>;
pub type Spinlock25 = Spinlock<25>;
pub type Spinlock26 = Spinlock<26>;
pub type Spinlock27 = Spinlock<27>;
pub type Spinlock28 = Spinlock<28>;
pub type Spinlock29 = Spinlock<29>;
pub type Spinlock30 = Spinlock<30>;

pub(super) type Spinlock31 = Spinlock<31>;

impl<const N: u8> Spinlock<N> {
    pub fn claim() -> Spinlock<N> {
        let d = unsafe { SIO::steal() };
        let p = d.spinlock(N as usize);
        while p.read().bits() == 0 {
            nop();
        }
        Spinlock(PhantomData)
    }
    pub fn try_claim() -> Option<Spinlock<N>> {
        let p = unsafe { SIO::steal() };
        if p.spinlock(N as usize).read().bits() > 0 { Some(Spinlock(PhantomData)) } else { None }
    }

    #[inline]
    pub fn release(self) {
        unsafe { SIO::steal().spinlock(N as usize).write_with_zero(|r| r.bits(1)) }
    }

    #[inline]
    pub unsafe fn free() {
        unsafe { SIO::steal().spinlock(N as usize).write_with_zero(|r| r.bits(1)) }
    }
}

impl<const N: u8> Drop for Spinlock<N> {
    #[inline]
    fn drop(&mut self) {
        unsafe { SIO::steal().spinlock(N as usize).write_with_zero(|r| r.bits(1)) }
    }
}

#[inline]
pub fn spinlock_state() -> [bool; 32] {
    let mut r = [false; 32];
    let v = unsafe { SIO::steal() }.spinlock_st().read().bits();
    for i in 0..32 {
        r[i] = (v & (1 << i)) > 0;
    }
    r
}
