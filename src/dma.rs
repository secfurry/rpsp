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
use core::cmp;
use core::marker::{Copy, PhantomData};
use core::mem::size_of;
use core::option::Option;
use core::sync::atomic::{compiler_fence, Ordering};

use crate::asm::{dsb, nop};
use crate::dma::mode::{BiDirection, Double, DoubleUp, Single};
use crate::pac::dma::CH;
use crate::pac::DMA;

#[repr(u8)]
pub enum Dma {
    Chan0  = 0u8,
    Chan1  = 1u8,
    Chan2  = 2u8,
    Chan3  = 3u8,
    Chan4  = 4u8,
    Chan5  = 5u8,
    Chan6  = 6u8,
    Chan7  = 7u8,
    Chan8  = 8u8,
    Chan9  = 9u8,
    Chan10 = 10u8,
    Chan11 = 11u8,
}
pub enum DmaPace {
    Sink,
    Source,
}

pub struct DmaConfig<D: DmaDirection>(D);
pub struct DmaStream<D: DmaDirection>(D);

pub trait DmaWord {}
pub trait DmaDirection {}
pub trait DmaReader<T: DmaWord> {
    fn rx_req(&self) -> Option<u8>;
    fn rx_info(&self) -> (u32, u32);
    fn rx_incremented(&self) -> bool;
}
pub trait DmaWriter<T: DmaWord> {
    fn tx_req(&self) -> Option<u8>;
    fn tx_info(&self) -> (u32, u32);
    fn tx_incremented(&self) -> bool;
}
pub trait DmaReadWrite<T: DmaWord>: DmaReader<T> + DmaWriter<T> {}

pub type DmaSingle<T, R, W> = DmaConfig<Single<T, R, W>>;
pub type DmaDouble<T, R, W> = DmaConfig<Double<T, R, W>>;
pub type DmaBiDirection<T, R, W, B> = DmaConfig<BiDirection<T, R, W, B>>;

impl Dma {
    #[inline]
    fn start(&self) {
        unsafe { DMA::steal().multi_chan_trigger().write(|r| r.bits(1 << *self as u32)) }
    }
    #[inline]
    fn ptr(&self) -> &CH {
        unsafe { &*DMA::PTR }.ch(*self as usize)
    }
    #[inline]
    fn link(&self, other: Dma) {
        unsafe {
            DMA::steal()
                .multi_chan_trigger()
                .write(|r| r.bits((1 << *self as u32) | (1 << other as u32)))
        }
    }
    #[inline]
    fn chain(&self, other: Dma) {
        let d = self.ptr();
        d.ch_al1_ctrl()
            .modify(|_, r| unsafe { r.chain_to().bits(other as u8).en().clear_bit() });
        if d.ch_al1_ctrl().read().busy().bit_is_set() {
            d.ch_al1_ctrl().modify(|_, r| r.en().set_bit());
        } else {
            other.start();
        }
    }
    #[inline]
    fn irq0_state(&self) -> bool {
        let d = unsafe { DMA::steal() };
        if (d.ints0().read().bits() & (1 << *self as u32)) == 0 {
            return false;
        }
        d.ints0().write(|r| unsafe { r.bits(1 << *self as u32) });
        true
    }
    #[inline]
    fn irq1_state(&self) -> bool {
        let d = unsafe { DMA::steal() };
        if (d.ints1().read().bits() & (1 << *self as u32)) == 0 {
            return false;
        }
        d.ints1().write(|r| unsafe { r.bits(1 << *self as u32) });
        true
    }
    fn setup<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>>(&self, from: &R, to: &W, swap: bool, pace: &DmaPace, start: bool) {
        let v = match pace {
            DmaPace::Source => from.rx_req().or_else(|| to.tx_req()).unwrap_or(0x3F),
            DmaPace::Sink => to.tx_req().or_else(|| from.rx_req()).unwrap_or(0x3F),
        };
        let (j, k) = from.rx_info();
        let (y, u) = to.tx_info();
        let d = self.ptr();
        d.ch_al1_ctrl().write(|r| unsafe {
            r.data_size()
                .bits(size_of::<T>() as u8 >> 1)
                .incr_read()
                .bit(from.rx_incremented())
                .incr_write()
                .bit(to.tx_incremented())
                .treq_sel()
                .bits(v)
                .bswap()
                .bit(swap)
                .chain_to()
                .bits(*self as u8)
                .en()
                .bit(true)
        });
        d.ch_read_addr().write(|r| unsafe { r.bits(j) });
        d.ch_trans_count().write(|r| unsafe { r.bits(cmp::min(k, u)) });
        if start {
            d.ch_al2_write_addr_trig().write(|r| unsafe { r.bits(y) });
        } else {
            d.ch_write_addr().write(|r| unsafe { r.bits(y) });
        }
    }
}
impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>> DmaConfig<Single<T, R, W>> {
    #[inline(always)]
    pub const fn new(ch: Dma, from: R, to: W) -> DmaConfig<Single<T, R, W>> {
        DmaConfig(Single {
            ch,
            ch_to: to,
            ch_from: from,
            pace: DmaPace::Source,
            swap: false,
            _p: PhantomData,
        })
    }

    #[inline(always)]
    pub fn pace(&mut self, v: DmaPace) {
        self.0.pace = v
    }
    #[inline(always)]
    pub fn bit_swap(&mut self, swap: bool) {
        self.0.swap = swap
    }
    #[inline]
    pub fn start(self) -> DmaStream<Single<T, R, W>> {
        dsb();
        compiler_fence(Ordering::SeqCst);
        self.0.ch.setup(
            &self.0.ch_from,
            &self.0.ch_to,
            self.0.swap,
            &self.0.pace,
            true,
        );
        DmaStream(self.0)
    }
}
impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>> DmaConfig<Double<T, R, W>> {
    #[inline(always)]
    pub const fn new(ch1: Dma, ch2: Dma, from: R, to: W) -> DmaConfig<Double<T, R, W>> {
        DmaConfig(Double {
            ch1,
            ch2,
            ch_to: to,
            ch_from: from,
            pace: DmaPace::Source,
            swap: false,
            first: true,
            _p: PhantomData,
        })
    }

    #[inline(always)]
    pub fn pace(&mut self, v: DmaPace) {
        self.0.pace = v
    }
    #[inline(always)]
    pub fn bit_swap(&mut self, swap: bool) {
        self.0.swap = swap
    }
    #[inline]
    pub fn start(self) -> DmaStream<Double<T, R, W>> {
        dsb();
        compiler_fence(Ordering::SeqCst);
        self.0.ch1.setup(
            &self.0.ch_from,
            &self.0.ch_to,
            self.0.swap,
            &self.0.pace,
            true,
        );
        DmaStream(self.0)
    }
}
impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>> DmaStream<Single<T, R, W>> {
    #[inline]
    pub fn wait(self) {
        while !self.is_done() {
            nop();
        }
        dsb();
        compiler_fence(Ordering::SeqCst);
    }
    #[inline]
    pub fn is_done(&self) -> bool {
        self.0.ch.ptr().ch_ctrl_trig().read().busy().bit_is_clear()
    }
    #[inline(always)]
    pub fn irq0_state(&self) -> bool {
        self.0.ch.irq0_state()
    }
    #[inline(always)]
    pub fn irq1_state(&self) -> bool {
        self.0.ch.irq1_state()
    }
}
impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>> DmaStream<Double<T, R, W>> {
    #[inline]
    pub fn wait(self) {
        while !self.is_done() {
            nop();
        }
        dsb();
        compiler_fence(Ordering::SeqCst);
    }
    #[inline]
    pub fn is_done(&self) -> bool {
        if self.0.first {
            self.0.ch1.ptr().ch_ctrl_trig().read().busy().bit_is_clear()
        } else {
            self.0.ch2.ptr().ch_ctrl_trig().read().busy().bit_is_clear()
        }
    }
    #[inline]
    pub fn irq0_state(&self) -> bool {
        if self.0.first {
            self.0.ch1.irq0_state()
        } else {
            self.0.ch2.irq0_state()
        }
    }
    #[inline]
    pub fn irq1_state(&self) -> bool {
        if self.0.first {
            self.0.ch1.irq1_state()
        } else {
            self.0.ch2.irq1_state()
        }
    }
    pub fn read_next<S: DmaReader<T>>(self, next: S) -> DmaStream<DoubleUp<T, R, W, S>> {
        dsb();
        compiler_fence(Ordering::SeqCst);
        if self.0.first {
            self.0.ch2.setup(&next, &self.0.ch_to, self.0.swap, &self.0.pace, false);
        } else {
            self.0.ch1.setup(&next, &self.0.ch_to, self.0.swap, &self.0.pace, false);
        }
        if self.0.first {
            self.0.ch1.chain(self.0.ch2);
        } else {
            self.0.ch2.chain(self.0.ch1);
        }
        DmaStream(DoubleUp { ch: self.0, state: next })
    }
    pub fn write_next<S: DmaWriter<T>>(self, next: S) -> DmaStream<DoubleUp<T, R, W, S>> {
        dsb();
        compiler_fence(Ordering::SeqCst);
        if self.0.first {
            self.0.ch2.setup(&self.0.ch_from, &next, self.0.swap, &self.0.pace, false);
        } else {
            self.0.ch1.setup(&self.0.ch_from, &next, self.0.swap, &self.0.pace, false);
        }
        if self.0.first {
            self.0.ch1.chain(self.0.ch2);
        } else {
            self.0.ch2.chain(self.0.ch1);
        }
        DmaStream(DoubleUp { ch: self.0, state: next })
    }
}
impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>, S> DmaStream<DoubleUp<T, R, W, S>> {
    #[inline]
    pub fn is_done(&self) -> bool {
        if self.0.ch.first {
            self.0.ch.ch1.ptr().ch_ctrl_trig().read().busy().bit_is_clear()
        } else {
            self.0.ch.ch2.ptr().ch_ctrl_trig().read().busy().bit_is_clear()
        }
    }
    #[inline]
    pub fn irq0_state(&self) -> bool {
        if self.0.ch.first {
            self.0.ch.ch1.irq0_state()
        } else {
            self.0.ch.ch2.irq0_state()
        }
    }
    #[inline]
    pub fn irq1_state(&self) -> bool {
        if self.0.ch.first {
            self.0.ch.ch1.irq1_state()
        } else {
            self.0.ch.ch2.irq1_state()
        }
    }
}
impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>, S: DmaReader<T>> DmaStream<DoubleUp<T, R, W, S>> {
    #[inline]
    pub fn wait_input(self) -> (R, DmaStream<Double<T, S, W>>) {
        while !self.is_done() {
            nop();
        }
        dsb();
        compiler_fence(Ordering::SeqCst);
        (
            self.0.ch.ch_from,
            DmaStream(Double {
                ch1:     self.0.ch.ch1,
                ch2:     self.0.ch.ch2,
                ch_to:   self.0.ch.ch_to,
                ch_from: self.0.state,
                pace:    self.0.ch.pace,
                first:   !self.0.ch.first,
                swap:    self.0.ch.swap,
                _p:      PhantomData,
            }),
        )
    }
}
impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>, S: DmaWriter<T>> DmaStream<DoubleUp<T, R, W, S>> {
    #[inline]
    pub fn wait_output(self) -> (W, DmaStream<Double<T, R, S>>) {
        while !self.is_done() {
            nop();
        }
        dsb();
        compiler_fence(Ordering::SeqCst);
        (
            self.0.ch.ch_to,
            DmaStream(Double {
                ch1:     self.0.ch.ch1,
                ch2:     self.0.ch.ch2,
                ch_to:   self.0.state,
                ch_from: self.0.ch.ch_from,
                pace:    self.0.ch.pace,
                first:   !self.0.ch.first,
                swap:    self.0.ch.swap,
                _p:      PhantomData,
            }),
        )
    }
}
impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>, B: DmaReadWrite<T>> DmaConfig<BiDirection<T, R, W, B>> {
    #[inline(always)]
    pub const fn new(ch1: Dma, ch2: Dma, from: R, bi: B, to: W) -> DmaConfig<BiDirection<T, R, W, B>> {
        DmaConfig(BiDirection {
            ch1,
            ch2,
            swap: false,
            ch_to: to,
            ch_bi: bi,
            ch_from: from,
            pace_to: DmaPace::Sink,
            pace_from: DmaPace::Sink,
            _p: PhantomData,
        })
    }

    #[inline(always)]
    pub fn pace_to(&mut self, v: DmaPace) {
        self.0.pace_to = v
    }
    #[inline(always)]
    pub fn bit_swap(&mut self, swap: bool) {
        self.0.swap = swap
    }
    #[inline(always)]
    pub fn pace_from(&mut self, v: DmaPace) {
        self.0.pace_from = v
    }
    #[inline]
    pub fn start(self) -> DmaStream<BiDirection<T, R, W, B>> {
        dsb();
        compiler_fence(Ordering::SeqCst);
        self.0.ch1.setup(
            &self.0.ch_from,
            &self.0.ch_bi,
            self.0.swap,
            &self.0.pace_from,
            false,
        );
        self.0.ch2.setup(
            &self.0.ch_bi,
            &self.0.ch_to,
            self.0.swap,
            &self.0.pace_to,
            false,
        );
        self.0.ch1.link(self.0.ch2);
        DmaStream(self.0)
    }
}
impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>, B: DmaReadWrite<T>> DmaStream<BiDirection<T, R, W, B>> {
    #[inline]
    pub fn wait(self) {
        while !self.is_done() {
            nop();
        }
        dsb();
        compiler_fence(Ordering::SeqCst);
    }
    #[inline]
    pub fn is_done(&self) -> bool {
        self.0.ch1.ptr().ch_ctrl_trig().read().busy().bit_is_clear() | self.0.ch2.ptr().ch_ctrl_trig().read().busy().bit_is_clear()
    }
    #[inline]
    pub fn irq0_state(&self) -> bool {
        self.0.ch1.irq0_state() | self.0.ch2.irq0_state()
    }
    #[inline]
    pub fn irq1_state(&self) -> bool {
        self.0.ch1.irq1_state() | self.0.ch2.irq1_state()
    }
}

impl Copy for Dma {}
impl Clone for Dma {
    #[inline(always)]
    fn clone(&self) -> Dma {
        *self
    }
}

impl DmaWord for u8 {}
impl DmaWord for u16 {}
impl DmaWord for u32 {}

pub mod mode {
    extern crate core;

    use core::marker::PhantomData;

    use crate::dma::{Dma, DmaDirection, DmaPace, DmaReadWrite, DmaReader, DmaWord, DmaWriter};

    pub struct Single<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>> {
        pub(super) ch:      Dma,
        pub(super) ch_to:   W,
        pub(super) ch_from: R,
        pub(super) pace:    DmaPace,
        pub(super) swap:    bool,
        pub(super) _p:      PhantomData<T>,
    }
    pub struct Double<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>> {
        pub(super) ch1:     Dma,
        pub(super) ch2:     Dma,
        pub(super) ch_to:   W,
        pub(super) ch_from: R,
        pub(super) pace:    DmaPace,
        pub(super) first:   bool,
        pub(super) swap:    bool,
        pub(super) _p:      PhantomData<T>,
    }
    pub struct DoubleUp<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>, S> {
        pub(super) ch:    Double<T, R, W>,
        pub(super) state: S,
    }
    pub struct BiDirection<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>, B: DmaReadWrite<T>> {
        pub(super) ch1:       Dma,
        pub(super) ch2:       Dma,
        pub(super) ch_to:     W,
        pub(super) ch_bi:     B,
        pub(super) ch_from:   R,
        pub(super) pace_to:   DmaPace,
        pub(super) pace_from: DmaPace,
        pub(super) swap:      bool,
        pub(super) _p:        PhantomData<T>,
    }

    impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>> DmaDirection for Single<T, R, W> {}
    impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>> DmaDirection for Double<T, R, W> {}
    impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>, S> DmaDirection for DoubleUp<T, R, W, S> {}
    impl<T: DmaWord, R: DmaReader<T>, W: DmaWriter<T>, B: DmaReadWrite<T>> DmaDirection for BiDirection<T, R, W, B> {}
}
