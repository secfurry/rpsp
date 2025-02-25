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
extern crate cortex_m;

use core::cell::{Ref, RefCell, RefMut, UnsafeCell};
use core::default::Default;
use core::marker::{PhantomData, Send, Sync};
use core::mem::{drop, forget};
use core::ops::{Drop, FnOnce};
use core::option::Option::Some;
use core::sync::atomic::{compiler_fence, AtomicU8, Ordering};

use cortex_m::interrupt::{disable, enable};
use cortex_m::register::primask::read;

use crate::asm::nop;
use crate::locks::Spinlock31;
use crate::pac::SIO;

static SINGLETON: AtomicU8 = AtomicU8::new(0);

pub struct Lock(u8);
pub struct Mutex<T> {
    v: UnsafeCell<T>,
}
pub struct Section<'a> {
    _p: PhantomData<&'a ()>,
    _s: PhantomData<*mut ()>,
}

struct Guard(Lock);

impl Lock {
    fn release(l: &Lock) {
        if l.0 == 0xFF {
            return;
        }
        SINGLETON.store(0, Ordering::Relaxed);
        compiler_fence(Ordering::SeqCst);
        unsafe { Spinlock31::free() };
        if l.0 > 0 {
            unsafe { enable() };
        }
    }
    fn acquire() -> Lock {
        let e = read().is_active();
        let c = unsafe { (*SIO::ptr()).cpuid().read().bits() } as u8 + 1u8;
        if SINGLETON.load(Ordering::Acquire) == c {
            return Lock(0xFF);
        }
        loop {
            disable();
            compiler_fence(Ordering::SeqCst);
            if let Some(v) = Spinlock31::try_claim() {
                forget(v);
                SINGLETON.store(c, Ordering::Relaxed);
                break;
            }
            if e {
                unsafe { enable() };
            }
            nop();
        }
        Lock(e as u8)
    }
}
impl<T> Mutex<T> {
    #[inline(always)]
    pub const fn new(v: T) -> Mutex<T> {
        Mutex { v: UnsafeCell::new(v) }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.v.into_inner()
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.v.get() }
    }
    #[inline]
    pub fn borrow<'a>(&'a self, _s: Section<'a>) -> &'a T {
        unsafe { &*self.v.get() }
    }
    #[inline]
    pub fn borrow_mut<'a>(&'a self, _s: Section<'a>) -> &'a mut T {
        unsafe { &mut *self.v.get() }
    }
}
impl<'a> Section<'a> {
    #[inline(always)]
    const fn new() -> Section<'a> {
        Section { _p: PhantomData, _s: PhantomData }
    }
}
impl<T> Mutex<RefCell<T>> {
    #[track_caller]
    #[inline]
    pub fn replace<'a>(&'a self, s: Section<'a>, t: T) -> T {
        self.borrow(s).replace(t)
    }
    #[track_caller]
    #[inline]
    pub fn borrow_ref<'a>(&'a self, s: Section<'a>) -> Ref<'a, T> {
        self.borrow(s).borrow()
    }
    #[track_caller]
    #[inline]
    pub fn borrow_ref_mut<'a>(&'a self, s: Section<'a>) -> RefMut<'a, T> {
        self.borrow(s).borrow_mut()
    }
    #[track_caller]
    #[inline]
    pub fn replace_with<'a>(&'a self, s: Section<'a>, f: impl FnOnce(&mut T) -> T) -> T {
        self.borrow(s).replace_with(f)
    }
}
impl<T: Default> Mutex<RefCell<T>> {
    #[track_caller]
    #[inline]
    pub fn take<'a>(&'a self, s: Section<'a>) -> T {
        self.borrow(s).take()
    }
}

impl Drop for Guard {
    #[inline(always)]
    fn drop(&mut self) {
        Lock::release(&self.0)
    }
}

unsafe impl<T> Sync for Mutex<T> where T: Send {}

#[inline]
pub fn with<T>(func: impl FnOnce(Section) -> T) -> T {
    let g = Guard(Lock::acquire());
    let r = func(Section::new());
    // Explicitly drop 'g'.
    drop(g);
    r
}

#[macro_export]
macro_rules! static_instance {
    ($name:ident, $type:ty, $expression:expr) => {
        static $name: Mutex<$type> = Mutex::new($expression);
    };
}
