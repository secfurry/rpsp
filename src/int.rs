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

use core::cell::UnsafeCell;
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{From, Into};
use core::marker::{Copy, PhantomData};
use core::mem::{MaybeUninit, size_of};
use core::ops::{Index, IndexMut};
use core::option::Option;
use core::ptr;

use cortex_m::interrupt::free;

use crate::asm::{wfe, wfi};
use crate::pac::{NVIC, PPB};

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use crate::pac::interrupt;

#[repr(u8)]
pub enum Interrupt {
    Alarm0  = 0,
    Alarm1  = 1,
    Alarm2  = 2,
    Alarm3  = 3,
    Pwm     = 4,
    UsbCtrl = 5,
    Xip     = 6,
    Pio0A   = 7,
    Pio0B   = 8,
    Pio1A   = 9,
    Pio1B   = 10,
    Dma0    = 11,
    Dma1    = 12,
    Bank0   = 13,
    QSpi    = 14,
    Sio0    = 15,
    Sio1    = 16,
    Clocks  = 17,
    Spi0    = 18,
    Spi1    = 19,
    Uart0   = 20,
    Uart1   = 21,
    Adc     = 22,
    I2c0    = 23,
    I2c1    = 24,
    Rtc     = 25,
    Sw0     = 26,
    Sw1     = 27,
    Sw2     = 28,
    Sw3     = 29,
    Sw4     = 30,
    Sw5     = 31,
}

pub struct Ack<'a>([Entry<'a>; 32]);
pub struct Custom<'a>([Call<'a>; 32]);
pub struct Standard(PhantomData<*const ()>);
pub struct Object<'a>(MaybeUninit<&'a mut dyn Interrupted>);
#[repr(transparent)]
pub struct InterruptHandler<E: InterruptExtension>(UnsafeCell<Handler<E>>);

pub trait Acknowledge {
    fn ack_interrupt(&mut self) -> bool;
}
pub trait Interrupted {
    fn interrupt(&mut self, i: Interrupt);
}
pub trait InterruptExtension {
    fn call(&mut self, _i: Interrupt) {}
}

pub type BaseHandler = InterruptHandler<Standard>;
pub type AckHandler<'a> = InterruptHandler<Ack<'a>>;
pub type CustomHandler<'a> = InterruptHandler<Custom<'a>>;
pub type ObjectHandler<'a> = InterruptHandler<Object<'a>>;

const ADDR_BASE: u32 = 0x10000100u32;
const ADDR_OFFSET: u32 = size_of::<usize>() as u32 * 48;

#[repr(u8)]
enum Extension {
    Standard = 0,
    Ack      = 1,
    Custom   = 2,
    Object   = 4,
}

struct Entry<'a> {
    ack:  MaybeUninit<&'a mut dyn Acknowledge>,
    func: Func,
}
#[repr(C, align(256))]
struct Handler<E: InterruptExtension> {
    ints: [Func; 48],
    ver:  Extension,
    ext:  E,
}
struct Call<'a>(MaybeUninit<&'a mut dyn Interrupted>);

union Func {
    ptr:      usize,
    func:     fn(),
    external: extern "C" fn(),
}

impl Interrupt {
    #[inline(always)]
    pub fn pend(&self) {
        set_pending(*self, true);
    }
    #[inline(always)]
    pub fn unpend(&self) {
        set_pending(*self, false);
    }
    #[inline(always)]
    pub fn enable(&self) {
        set_interrupt(*self, true);
    }
    #[inline(always)]
    pub fn disable(&self) {
        set_interrupt(*self, false);
    }
    #[inline(always)]
    pub fn set(&self, en: bool) {
        set_interrupt(*self, en);
    }
    #[inline(always)]
    pub fn priority(&self) -> u8 {
        get_priority(*self)
    }
    #[inline(always)]
    pub fn is_enabled(&self) -> bool {
        is_enabled(*self)
    }
    #[inline(always)]
    pub fn is_pending(&self) -> bool {
        is_pending(*self)
    }
    #[inline(always)]
    pub fn set_priority(&self, pri: u8) {
        set_priority(*self, pri);
    }

    #[inline(always)]
    fn ipr(&self) -> usize {
        (*self as usize) / 4
    }
    #[inline(always)]
    fn pos(&self) -> usize {
        (*self as usize) / 32
    }
    #[inline(always)]
    fn value(&self) -> u32 {
        1 << (*self as u32 % 32)
    }
    #[inline(always)]
    fn shift(&self) -> usize {
        ((*self as usize) % 4) * 8
    }
}
impl<'a> Ack<'a> {
    #[inline(always)]
    const fn new() -> Ack<'a> {
        Ack([const { Entry::new() }; 32])
    }
}
impl<'a> Entry<'a> {
    #[inline(always)]
    const fn new() -> Entry<'a> {
        Entry {
            ack:  MaybeUninit::uninit(),
            func: Func { ptr: 0usize },
        }
    }

    #[inline]
    fn call(&mut self) {
        unsafe {
            if self.ack.assume_init_mut().ack_interrupt() {
                (self.func.func)()
            }
        }
    }
    #[inline]
    fn set(&mut self, ack: &'a mut impl Acknowledge, func: fn()) {
        self.ack.write(ack);
        self.func.func = func;
    }
}
impl<'a> Custom<'a> {
    #[inline(always)]
    const fn new() -> Custom<'a> {
        Custom([const { Call(MaybeUninit::uninit()) }; 32])
    }
}
impl InterruptHandler<Standard> {
    #[inline]
    pub fn new() -> InterruptHandler<Standard> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Standard,
            ext:  Standard(PhantomData),
            ints: Self::interrupt_table(false),
        }))
    }

    #[inline]
    pub fn to_ack<'a>(self) -> InterruptHandler<Ack<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Ack,
            ext:  Ack::new(),
            ints: self.0.into_inner().ints,
        }))
    }
    #[inline]
    pub fn to_custom<'a>(self) -> InterruptHandler<Custom<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Custom,
            ext:  Custom::new(),
            ints: self.0.into_inner().ints,
        }))
    }
    #[inline]
    pub fn to_object<'a>(self, obj: &'a mut impl Interrupted) -> InterruptHandler<Object<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Object,
            ext:  Object(MaybeUninit::new(obj)),
            ints: self.0.into_inner().ints,
        }))
    }
}
impl<'a> InterruptHandler<Ack<'a>> {
    #[inline]
    pub fn new() -> InterruptHandler<Ack<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Ack,
            ext:  Ack::new(),
            ints: Self::interrupt_table(false),
        }))
    }

    #[inline]
    pub fn to_custom(self) -> InterruptHandler<Custom<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Custom,
            ext:  Custom::new(),
            ints: self.0.into_inner().ints,
        }))
    }
    #[inline]
    pub fn to_standard(self) -> InterruptHandler<Standard> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Standard,
            ext:  Standard(PhantomData),
            ints: self.0.into_inner().ints,
        }))
    }
    #[inline]
    pub fn enable(&mut self, i: Interrupt, ack: &'a mut impl Acknowledge, func: fn()) {
        free(|_| {
            self.ptr().ext.0[i as usize].set(ack, func);
            self.set_inner(i, interrupt_handler);
        })
    }
    #[inline]
    pub fn to_object(self, obj: &'a mut impl Interrupted) -> InterruptHandler<Object<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Object,
            ext:  Object(MaybeUninit::new(obj)),
            ints: self.0.into_inner().ints,
        }))
    }
}
impl<'a> InterruptHandler<Custom<'a>> {
    #[inline]
    pub fn new() -> InterruptHandler<Custom<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Custom,
            ext:  Custom::new(),
            ints: Self::interrupt_table(false),
        }))
    }

    #[inline]
    pub fn to_ack(self) -> InterruptHandler<Ack<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Ack,
            ext:  Ack::new(),
            ints: self.0.into_inner().ints,
        }))
    }
    #[inline]
    pub fn to_standard(self) -> InterruptHandler<Standard> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Standard,
            ext:  Standard(PhantomData),
            ints: self.0.into_inner().ints,
        }))
    }
    #[inline]
    pub fn enable(&mut self, i: Interrupt, v: &'a mut impl Interrupted) {
        free(|_| {
            self.ptr().ext.0[i as usize].0.write(v);
            self.set_inner(i, interrupt_handler);
        })
    }
    #[inline]
    pub fn to_object(self, obj: &'a mut impl Interrupted) -> InterruptHandler<Object<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Object,
            ext:  Object(MaybeUninit::new(obj)),
            ints: self.0.into_inner().ints,
        }))
    }
}
impl<'a> InterruptHandler<Object<'a>> {
    #[inline]
    pub fn new() -> InterruptHandler<Object<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            // SAFETY: We mark it as Standard until an object is set to prevent
            //         a null dereference.
            ver:  Extension::Standard,
            ext:  Object(MaybeUninit::uninit()),
            ints: Self::interrupt_table(false),
        }))
    }
    #[inline]
    pub fn new_with(obj: &'a mut impl Interrupted) -> InterruptHandler<Object<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Object,
            ext:  Object(MaybeUninit::new(obj)),
            ints: Self::interrupt_table(false),
        }))
    }

    #[inline(always)]
    pub fn enable(&mut self, i: Interrupt) {
        free(|_| self.set_inner(i, interrupt_handler))
    }
    #[inline]
    pub fn to_ack(self) -> InterruptHandler<Ack<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Ack,
            ext:  Ack::new(),
            ints: self.0.into_inner().ints,
        }))
    }
    #[inline]
    pub fn to_custom(self) -> InterruptHandler<Custom<'a>> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Custom,
            ext:  Custom::new(),
            ints: self.0.into_inner().ints,
        }))
    }
    #[inline]
    pub fn to_standard(self) -> InterruptHandler<Standard> {
        InterruptHandler(UnsafeCell::new(Handler {
            ver:  Extension::Standard,
            ext:  Standard(PhantomData),
            ints: self.0.into_inner().ints,
        }))
    }
    #[inline]
    pub fn update(&mut self, obj: &'a mut impl Interrupted) {
        free(|_| {
            self.ptr().ext.0.write(obj);
            // SAFETY: Mark it as safe now.
            self.ptr().ver = Extension::Object
        })
    }
}
impl<E: InterruptExtension> InterruptHandler<E> {
    #[inline]
    pub fn sync(&mut self) {
        unsafe {
            PPB::steal()
                .vtor()
                .write(|r| r.bits(self.ptr() as *const Handler<E> as u32))
        }
    }
    #[inline]
    pub fn remove(&mut self) {
        unsafe { PPB::steal().vtor().write(|r| r.bits(ADDR_BASE)) }
    }
    pub fn disable(&mut self, i: Interrupt) {
        free(|_| {
            // Disable the interrupt.
            i.disable();
            let i = 0x10 + (i as usize);
            // Read directly from ROM.
            let t = Self::interrupt_table(true);
            // Reset the default value.
            unsafe { self.ptr().ints[i].ptr = t[i].ptr };
        })
    }
    #[inline(always)]
    pub fn set(&mut self, i: Interrupt, func: extern "C" fn()) {
        free(|_| self.set_inner(i, func))
    }

    #[inline]
    fn interrupt_table(default: bool) -> [Func; 48] {
        let mut i = [const { Func { ptr: 0usize } }; 48];
        unsafe {
            ptr::copy_nonoverlapping(
                if default { ADDR_BASE as *mut usize } else { PPB::steal().vtor().read().bits() as *mut usize },
                i.as_mut_ptr() as *mut usize,
                48,
            );
        }
        i
    }

    #[inline(always)]
    fn ptr(&self) -> &mut Handler<E> {
        unsafe { &mut *self.0.get() }
    }
    #[inline]
    fn set_inner(&self, i: Interrupt, func: extern "C" fn()) {
        i.enable();
        self.ptr().ints[0x10 + (i as usize)].external = func
    }
}

impl Eq for Interrupt {}
impl Ord for Interrupt {
    #[inline(always)]
    fn cmp(&self, other: &Interrupt) -> Ordering {
        (&(*self as u8)).cmp(&(*other as u8))
    }
}
impl Copy for Interrupt {}
impl Clone for Interrupt {
    #[inline(always)]
    fn clone(&self) -> Interrupt {
        *self
    }
}
impl From<u8> for Interrupt {
    #[inline]
    fn from(v: u8) -> Interrupt {
        match v.saturating_sub(0x10) {
            0 => Interrupt::Alarm0,
            1 => Interrupt::Alarm1,
            2 => Interrupt::Alarm2,
            3 => Interrupt::Alarm3,
            4 => Interrupt::Pwm,
            5 => Interrupt::UsbCtrl,
            6 => Interrupt::Xip,
            7 => Interrupt::Pio0A,
            8 => Interrupt::Pio0B,
            9 => Interrupt::Pio1A,
            10 => Interrupt::Pio1B,
            11 => Interrupt::Dma0,
            12 => Interrupt::Dma1,
            13 => Interrupt::Bank0,
            14 => Interrupt::QSpi,
            15 => Interrupt::Sio0,
            16 => Interrupt::Sio1,
            17 => Interrupt::Clocks,
            18 => Interrupt::Spi0,
            19 => Interrupt::Spi1,
            20 => Interrupt::Uart0,
            21 => Interrupt::Uart1,
            22 => Interrupt::Adc,
            23 => Interrupt::I2c0,
            24 => Interrupt::I2c1,
            25 => Interrupt::Rtc,
            26 => Interrupt::Sw0,
            27 => Interrupt::Sw1,
            28 => Interrupt::Sw2,
            29 => Interrupt::Sw3,
            30 => Interrupt::Sw4,
            _ => Interrupt::Sw5,
        }
    }
}
impl From<u16> for Interrupt {
    #[inline(always)]
    fn from(v: u16) -> Interrupt {
        (v as u8).into()
    }
}
impl PartialEq for Interrupt {
    #[inline(always)]
    fn eq(&self, other: &Interrupt) -> bool {
        (*self as u8) == (*other as u8)
    }
}
impl PartialOrd for Interrupt {
    #[inline(always)]
    fn partial_cmp(&self, other: &Interrupt) -> Option<Ordering> {
        (&(*self as u8)).partial_cmp(&(*other as u8))
    }
}

impl InterruptExtension for Ack<'_> {
    #[inline(never)]
    fn call(&mut self, i: Interrupt) {
        self.0[i as usize].call()
    }
}
impl InterruptExtension for Standard {}
impl InterruptExtension for Custom<'_> {
    #[inline(never)]
    fn call(&mut self, i: Interrupt) {
        unsafe { self.0[i as usize].0.assume_init_mut() }.interrupt(i)
    }
}
impl InterruptExtension for Object<'_> {
    #[inline(never)]
    fn call(&mut self, i: Interrupt) {
        unsafe { self.0.assume_init_mut() }.interrupt(i)
    }
}

impl<E: InterruptExtension> Index<Interrupt> for Handler<E> {
    type Output = Func;

    #[inline(always)]
    fn index(&self, i: Interrupt) -> &Func {
        &self.ints[i as usize]
    }
}
impl<E: InterruptExtension> IndexMut<Interrupt> for Handler<E> {
    #[inline(always)]
    fn index_mut(&mut self, i: Interrupt) -> &mut Func {
        &mut self.ints[i as usize]
    }
}

#[inline(always)]
pub fn wait_for_event() {
    wfe();
}
#[inline(always)]
pub fn wait_for_interrupt() {
    wfi();
}
#[inline]
pub fn is_enabled(i: Interrupt) -> bool {
    let v = i.value();
    unsafe { (&*NVIC::PTR).iser[i.pos()].read() & v == v }
}
#[inline]
pub fn is_pending(i: Interrupt) -> bool {
    let v = i.value();
    unsafe { (&*NVIC::PTR).ispr[i.pos()].read() & v == v }
}
#[inline]
pub fn get_priority(i: Interrupt) -> u8 {
    unsafe { (((&*NVIC::PTR).ipr[i.ipr()].read() >> i.shift()) & 0xFF) as u8 }
}
#[inline]
pub fn set_priority(i: Interrupt, pri: u8) {
    unsafe { (&*NVIC::PTR).ipr[i.ipr()].modify(|r| (r & !(0xFF << (i.shift()))) | ((pri as u32) << i.shift())) }
}
#[inline]
pub fn set_pending(i: Interrupt, pend: bool) {
    if pend {
        unsafe { (&*NVIC::PTR).ispr[i.pos()].write(i.value()) }
    } else {
        unsafe { (&*NVIC::PTR).icpr[i.pos()].write(i.value()) }
    }
}
#[inline]
pub fn set_interrupt(i: Interrupt, en: bool) {
    if en {
        unsafe { (&*NVIC::PTR).iser[i.pos()].write(i.value()) }
    } else {
        unsafe { (&*NVIC::PTR).icer[i.pos()].write(i.value()) }
    }
}

#[inline(never)]
extern "C" fn interrupt_handler() {
    let r = unsafe { PPB::steal() };
    let (i, p) = (
        r.icsr().read().vectactive().bits().into(),
        r.vtor().read().bits(),
    );
    match unsafe { &*((p + ADDR_OFFSET) as *const Extension) } {
        Extension::Ack => unsafe { &mut *(p as *mut Handler<Ack>) }.ext.call(i),
        Extension::Custom => unsafe { &mut *(p as *mut Handler<Custom>) }.ext.call(i),
        Extension::Object => unsafe { &mut *(p as *mut Handler<Object>) }.ext.call(i),
        _ => (),
    }
}
