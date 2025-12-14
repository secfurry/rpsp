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
use core::fmt::{self, Debug, Formatter};
use core::marker::{Copy, Sync};
use core::matches;
use core::mem::{ManuallyDrop, drop, zeroed};
use core::ops::FnOnce;
use core::result::Result::{self, Err, Ok};
use core::sync::atomic::{Ordering, compiler_fence};

use crate::asm::{nop, sev, udf};
use crate::atomic::{Mutex, with};
use crate::fifo::Fifo;
use crate::pac::{MPU, PPB, PSM, RESETS, SIO, SYST};
use crate::static_instance;

const ATTEMPTS: u8 = 0x8u8;

static_instance!(CORE1_STATE, CoreState, CoreState::Uninit);

#[repr(u8)]
pub enum Core {
    C0 = 0,
    C1 = 1,
}
pub enum CoreError {
    InUse,
    NotActive,
    NoResponse,
    InvalidCore,
}

#[repr(C, align(32))]
pub struct CoreStack<const N: usize = 2048>(UnsafeCell<[usize; N]>);

enum CoreState {
    Uninit,
    Active,
    Available,
}

impl Core {
    #[inline]
    pub fn current() -> Core {
        if unsafe { (*SIO::ptr()).cpuid().read().bits() % 2 == 0 } { Core::C0 } else { Core::C1 }
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        is_running(*self)
    }
}
impl<const N: usize> CoreStack<N> {
    #[inline]
    pub const fn new() -> CoreStack<N> {
        CoreStack(UnsafeCell::new([0usize; N]))
    }
}

impl Copy for Core {}
impl Clone for Core {
    #[inline]
    fn clone(&self) -> Core {
        *self
    }
}

impl Copy for CoreState {}
impl Clone for CoreState {
    #[inline]
    fn clone(&self) -> CoreState {
        *self
    }
}

impl Debug for CoreError {
    #[cfg(feature = "debug")]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::InUse => f.write_str("InUse"),
            CoreError::NotActive => f.write_str("NotActive"),
            CoreError::NoResponse => f.write_str("NoResponse"),
            CoreError::InvalidCore => f.write_str("InvalidCore"),
        }
    }
    #[cfg(not(feature = "debug"))]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

unsafe impl Sync for CoreStack {}

#[inline]
pub fn is_running(core: Core) -> bool {
    match core {
        Core::C0 => true,
        Core::C1 => matches!(core1_get_status(), CoreState::Active),
    }
}
#[inline]
pub fn interrupt(core: Core) -> Result<(), CoreError> {
    match core {
        Core::C0 => Err(CoreError::InvalidCore),
        Core::C1 if matches!(core1_get_status(), CoreState::Active | CoreState::Available) => {
            core1_reset();
            Ok(())
        },
        _ => Err(CoreError::NotActive),
    }
}
#[inline]
pub fn spawn_core1<const N: usize, F: FnOnce() -> () + Sync>(stack: &'static CoreStack<N>, func: F) -> Result<(), CoreError> {
    spawn(Core::C1, stack, func)
}
pub fn spawn<const N: usize, F: FnOnce() -> () + Sync>(core: Core, stack: &'static CoreStack<N>, func: F) -> Result<(), CoreError> {
    match core {
        Core::C0 => return Err(CoreError::InvalidCore),
        Core::C1 => (),
    }
    match core1_get_status() {
        CoreState::Available => return core1_push(func),
        CoreState::Active => return Err(CoreError::InUse),
        CoreState::Uninit => (),
    }
    let mut f = Fifo::get();
    let x = unsafe { &mut *stack.0.get() };
    let s = unsafe {
        let v = x.as_mut_ptr().add(x.len());
        v.sub(v.align_offset(8) + 1)
    };
    let mut e = ManuallyDrop::new(func);
    let s = unsafe {
        s.cast::<*mut usize>().write(x.as_mut_ptr());
        let s = s.sub(1);
        s.cast::<*mut ManuallyDrop<F>>().write(&mut e);
        s
    };
    core1_reset();
    compiler_fence(Ordering::Release);
    let d = [
        0,
        0,
        core as usize,
        unsafe { PPB::steal() }.vtor().read().bits() as usize,
        s as usize,
        core1_start::<F> as usize,
    ];
    let mut u = 0;
    'outer: loop {
        if u > ATTEMPTS {
            drop(ManuallyDrop::into_inner(e));
            return Err(CoreError::NoResponse);
        }
        for i in d.iter() {
            if *i == 0 {
                f.drain();
                sev();
            }
            f.write_block(*i as _);
            if f.read_block() != *i as _ {
                u += 1;
                continue 'outer;
            }
        }
        break;
    }
    Ok(())
}

#[inline]
fn core1_reset() {
    let s = unsafe { PSM::steal() };
    s.frce_off().modify(|_, r| r.proc1().set_bit());
    while s.frce_off().read().proc1().bit_is_clear() {
        nop();
    }
    s.frce_off().modify(|_, r| r.proc1().clear_bit())
}
#[inline]
fn core1_timers() {
    let v: SYST = unsafe { zeroed() };
    unsafe { v.csr.modify(|r| r | 0x4) };
    let r = unsafe { RESETS::steal() };
    // Reset Alarms and Timers
    r.reset().modify(|_, r| r.timer().set_bit());
    r.reset().modify(|_, r| r.timer().clear_bit());
    while r.reset_done().read().timer().bit_is_clear() {
        nop();
    }
}
#[inline]
fn core1_status(s: CoreState) {
    with(|x| *CORE1_STATE.borrow_mut(x) = s)
}
#[inline]
fn core1_get_status() -> CoreState {
    with(|x| *CORE1_STATE.borrow(x))
}
#[inline]
fn core1_stack_guard(stack: *mut usize) {
    let m = unsafe { &*MPU::PTR };
    if m.ctrl.read() != 0 {
        udf();
    }
    let a = (stack as u32 + 0x1F) & !0x1F;
    let r = 0xFF ^ unsafe { 1u32.unchecked_shl(a.unchecked_shr(5) & 0x7) };
    unsafe {
        m.ctrl.write(0x5);
        m.rbar.write((a & !0xFF) | 0x10);
        m.rasr.write(r.unchecked_shl(8) | 0x1000000F);
    }
}
#[inline]
fn core1_push<F: FnOnce() -> () + Sync>(func: F) -> Result<(), CoreError> {
    let mut f = Fifo::get();
    let mut e = ManuallyDrop::new(func);
    f.drain();
    sev();
    for _ in 0..ATTEMPTS {
        f.write_block(&mut e as *mut ManuallyDrop<F> as _);
        if f.read_block() == 1 {
            return Ok(());
        }
    }
    drop(ManuallyDrop::into_inner(e));
    Err(CoreError::NoResponse)
}

#[inline(never)]
extern "C" fn core1_start<F: FnOnce() -> () + Sync>(_: u64, _: u64, main: *mut ManuallyDrop<F>, stack: *mut usize) {
    compiler_fence(Ordering::SeqCst);
    core1_stack_guard(stack);
    core1_timers();
    let mut f = Fifo::get();
    f.write_block(1);
    unsafe { ManuallyDrop::take(&mut *main)() };
    core1_status(CoreState::Available);
    loop {
        f.drain();
        let n = f.read_block();
        if n == 0 {
            continue;
        }
        core1_status(CoreState::Active);
        unsafe {
            let x = ManuallyDrop::take(&mut *(n as *mut ManuallyDrop<F>));
            f.write_block(1);
            x();
        }
        core1_status(CoreState::Available);
    }
}
