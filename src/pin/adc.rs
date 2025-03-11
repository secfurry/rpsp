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
use core::convert::{From, Into};
use core::marker::{Copy, PhantomData};
use core::option::Option::{self, Some};
use core::result::Result::{self, Err, Ok};

use crate::asm::{delay, nop};
use crate::atomic::{Mutex, with};
use crate::clock::DIV;
use crate::dma::{DmaReader, DmaWord};
use crate::pac::{ADC, CLOCKS, IO_BANK0, RESETS};
use crate::pin::gpio::Input;
use crate::pin::{Pin, PinID, PinInvalidError};
use crate::static_instance;

static_instance!(READY, bool, false);

#[repr(u8)]
pub enum AdcChannel {
    Chan0, // Pin26
    Chan1, // Pin27
    Chan2, // Pin28
    Chan3, // Pin29
    Chan4, // Chan4 is the Temperature Sensor
}

pub struct AdcPin {
    i:  AdcChannel,
    _p: PhantomData<UnsafeCell<()>>,
}
pub struct AdcFifo<R> {
    d:  ADC,
    _p: PhantomData<R>,
}
pub struct AdcSelection(u8);
pub struct AdcTempSensor(PhantomData<UnsafeCell<()>>);

pub struct AdcFifoBuilder<R = u16> {
    d:  ADC,
    _p: PhantomData<R>,
}

pub trait AdcSelector {
    fn channel(&self) -> AdcChannel;
}

impl AdcPin {
    #[inline]
    pub fn temp_sensor() -> AdcTempSensor {
        prepare_adc();
        unsafe { ADC::steal() }
            .cs()
            .modify(|_, r| r.ts_en().set_bit().en().set_bit());
        AdcTempSensor(PhantomData)
    }
    pub fn new(p: Pin<Input>) -> Result<AdcPin, PinInvalidError> {
        prepare_adc();
        // NOTE(sf): These never change based on the board config, so it can be
        //           here.
        let i = match &p.i {
            PinID::Pin26 => AdcChannel::Chan0,
            PinID::Pin27 => AdcChannel::Chan1,
            PinID::Pin28 => AdcChannel::Chan2,
            PinID::Pin29 => AdcChannel::Chan3,
            _ => return Err(PinInvalidError),
        };
        unsafe { IO_BANK0::steal() }
            .gpio(p.i as usize)
            .gpio_ctrl()
            .modify(|_, r| r.oeover().disable());
        p.i.ctrl().modify(|_, r| r.ie().set_bit());
        unsafe { ADC::steal() }.cs().modify(|_, r| r.en().set_bit());
        Ok(AdcPin { i, _p: PhantomData })
    }

    #[inline]
    pub fn wait_ready(&self) {
        let d = unsafe { ADC::steal() };
        // ready      - bit 8
        // start_many - bit 3
        //
        // Pull down 3
        //   1<<5 == 32 + 1 == 33 == 0x21
        // If pos 5 (32) or 1 (1) is set, this will return > 0.
        // Anything set in-between will return 0.
        while (d.cs().read().bits() >> 3) & 0x21 == 0 {
            nop();
        }
    }
    #[inline]
    pub fn read(&self) -> u16 {
        unsafe { ADC::steal() }.result().read().result().bits()
    }
    #[inline]
    pub fn is_ready(&self) -> bool {
        unsafe { ADC::steal() }.cs().read().ready().bit_is_set()
    }
    #[inline]
    pub fn read_block(&self) -> u16 {
        self.wait_ready();
        unsafe { ADC::steal() }
            .cs()
            .modify(|_, r| unsafe { r.ainsel().bits(self.i as u8).start_once().set_bit() });
        self.wait_ready();
        self.read()
    }
    #[inline(always)]
    pub fn stop_free_running(&mut self) {
        self.set_free_running(false)
    }
    #[inline(always)]
    pub fn start_free_running(&mut self) {
        self.set_free_running(true)
    }
    #[inline]
    pub fn set_free_running(&mut self, en: bool) {
        let d = unsafe { ADC::steal() };
        if en {
            d.cs()
                .modify(|_, r| unsafe { r.ainsel().bits(self.i as u8).start_many().set_bit() });
        } else {
            d.cs().modify(|_, r| r.start_many().clear_bit());
        }
    }
}
impl AdcFifo<u8> {
    #[inline]
    pub fn read(&mut self) -> u8 {
        self.d.fifo().read().val().bits() as u8
    }
}
impl AdcFifo<u16> {
    #[inline]
    pub fn read(&mut self) -> u16 {
        self.d.fifo().read().val().bits()
    }
}
impl AdcTempSensor {
    #[inline]
    pub fn close(&mut self) {
        unsafe { ADC::steal() }.cs().modify(|_, r| r.ts_en().clear_bit());
    }
    #[inline]
    pub fn read(&self) -> u8 {
        self.wait_ready();
        let d = unsafe { ADC::steal() };
        d.cs()
            .modify(|_, r| unsafe { r.ainsel().bits(AdcChannel::Chan4 as u8).start_once().set_bit() });
        self.wait_ready();
        d.result().read().result().bits() as u8
    }
    #[inline]
    pub fn wait_ready(&self) {
        let d = unsafe { ADC::steal() };
        // ready      - bit 8
        // start_many - bit 3
        //
        // Pull down 3
        //   1<<5 == 32 + 1 == 33 == 0x21
        // If pos 5 (32) or 1 (1) is set, this will return > 0.
        // Anything set in-between will return 0.
        while (d.cs().read().bits() >> 3) & 0x21 == 0 {
            //spin_loop();
            nop();
        }
    }
}
impl<R> AdcFifo<R> {
    pub fn close(self) {
        self.d
            .cs()
            .modify(|_, r| unsafe { r.start_many().clear_bit().rrobin().bits(0).ainsel().bits(0) });
        self.d.inte().modify(|_, r| r.fifo().clear_bit());
        while self.d.cs().read().ready().bit_is_clear() {
            nop();
        }
        while self.len() > 0 {
            let _ = self.d.result().read().result().bits();
        }
        self.d
            .fcs()
            .modify(|_, r| unsafe { r.en().clear_bit().thresh().bits(0).dreq_en().clear_bit() });
        self.d.div().modify(|_, r| unsafe { r.int().bits(0).frac().bits(0) });
    }
    #[inline]
    pub fn clear(&mut self) {
        while self.len() > 0 {
            let _ = self.read_sample();
        }
    }
    #[inline]
    pub fn len(&self) -> u8 {
        self.d.fcs().read().level().bits()
    }
    #[inline(always)]
    pub fn pause(&mut self) {
        self.state(true);
    }
    #[inline(always)]
    pub fn resume(&mut self) {
        self.state(false);
    }
    #[inline]
    pub fn trigger(&mut self) {
        self.d.cs().modify(|_, r| r.start_once().set_bit())
    }
    #[inline]
    pub fn wait_interrupt(&self) {
        while self.d.intr().read().fifo().bit_is_clear() {
            nop();
        }
    }
    #[inline]
    pub fn is_ready(&self) -> bool {
        self.d.cs().read().ready().bit_is_set()
    }
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.d.cs().read().start_many().bit_is_clear()
    }
    #[inline]
    pub fn is_overrun(&self) -> bool {
        let r = self.d.fcs().read().over().bit_is_set();
        if r {
            self.d.fcs().modify(|_, r| r.over().clear_bit_by_one());
        }
        r
    }
    #[inline]
    pub fn is_underrun(&self) -> bool {
        let r = self.d.fcs().read().under().bit_is_set();
        if r {
            self.d.fcs().modify(|_, r| r.under().clear_bit_by_one());
        }
        r
    }
    #[inline]
    pub fn read_sample(&mut self) -> u16 {
        self.d.result().read().result().bits()
    }
    #[inline]
    pub fn state(&mut self, paused: bool) {
        self.d.cs().modify(|_, r| r.start_many().bit(!paused))
    }
}
impl AdcFifoBuilder<u16> {
    #[inline(always)]
    pub fn new() -> AdcFifoBuilder<u16> {
        AdcFifoBuilder {
            d:  unsafe { ADC::steal() },
            _p: PhantomData,
        }
    }

    #[inline]
    pub fn shift(self) -> AdcFifoBuilder<u8> {
        self.d.fcs().modify(|_, r| r.shift().set_bit());
        AdcFifoBuilder { d: self.d, _p: PhantomData }
    }
}
impl<R> AdcFifoBuilder<R> {
    #[inline(always)]
    pub fn start(self) -> AdcFifo<R> {
        self.start_paused(false)
    }
    #[inline]
    pub fn dma(self) -> AdcFifoBuilder<R> {
        self.d
            .fcs()
            .modify(|_, r| unsafe { r.dreq_en().set_bit().thresh().bits(1) });
        self
    }
    #[inline]
    pub fn interrupt(self, v: u8) -> AdcFifoBuilder<R> {
        self.d.inte().modify(|_, r| r.fifo().set_bit());
        self.d.fcs().modify(|_, r| unsafe { r.thresh().bits(v) });
        self
    }
    #[inline]
    pub fn div(self, i: u16, f: u8) -> AdcFifoBuilder<R> {
        self.d.div().modify(|_, r| unsafe { r.int().bits(i).frac().bits(f) });
        self
    }
    #[inline]
    pub fn start_paused(self, paused: bool) -> AdcFifo<R> {
        self.d.fcs().modify(|_, r| r.en().set_bit());
        self.d.cs().modify(|_, r| r.start_once().bit(!paused));
        AdcFifo { d: self.d, _p: PhantomData }
    }
    #[inline]
    pub fn channel(self, pin: &AdcPin) -> AdcFifoBuilder<R> {
        self.d.cs().modify(|_, r| unsafe { r.ainsel().bits(pin.i as u8) });
        self
    }
    #[inline]
    pub fn selection(self, v: impl Into<AdcSelection>) -> AdcFifoBuilder<R> {
        let s: AdcSelection = v.into();
        self.d.cs().modify(|_, r| unsafe { r.rrobin().bits(s.0) });
        self
    }
}

impl AdcSelector for AdcPin {
    #[inline(always)]
    fn channel(&self) -> AdcChannel {
        self.i
    }
}
impl AdcSelector for AdcTempSensor {
    #[inline(always)]
    fn channel(&self) -> AdcChannel {
        AdcChannel::Chan4
    }
}

impl Copy for AdcChannel {}
impl Clone for AdcChannel {
    #[inline(always)]
    fn clone(&self) -> AdcChannel {
        *self
    }
}

impl<A: AdcSelector> From<&A> for AdcSelection {
    #[inline(always)]
    fn from(v: &A) -> AdcSelection {
        AdcSelection(1 << (v.channel() as u8))
    }
}
impl<A: AdcSelector, B: AdcSelector> From<(&A, &B)> for AdcSelection {
    #[inline(always)]
    fn from(v: (&A, &B)) -> AdcSelection {
        AdcSelection(1 << (v.0.channel() as u8) | 1 << (v.1.channel() as u8))
    }
}
impl<A: AdcSelector, B: AdcSelector, C: AdcSelector> From<(&A, &B, &C)> for AdcSelection {
    #[inline(always)]
    fn from(v: (&A, &B, &C)) -> AdcSelection {
        AdcSelection(1 << (v.0.channel() as u8) | 1 << (v.1.channel() as u8) | 1 << (v.2.channel() as u8))
    }
}
impl<A: AdcSelector, B: AdcSelector, C: AdcSelector, D: AdcSelector> From<(&A, &B, &C, &D)> for AdcSelection {
    #[inline(always)]
    fn from(v: (&A, &B, &C, &D)) -> AdcSelection {
        AdcSelection(1 << (v.0.channel() as u8) | 1 << (v.1.channel() as u8) | 1 << (v.2.channel() as u8) | 1 << (v.3.channel() as u8))
    }
}
impl<A: AdcSelector, B: AdcSelector, C: AdcSelector, D: AdcSelector, E: AdcSelector> From<(&A, &B, &C, &D, &E)> for AdcSelection {
    #[inline(always)]
    fn from(v: (&A, &B, &C, &D, &E)) -> AdcSelection {
        AdcSelection(1 << (v.0.channel() as u8) | 1 << (v.1.channel() as u8) | 1 << (v.2.channel() as u8) | 1 << (v.3.channel() as u8) | 1 << (v.4.channel() as u8))
    }
}

impl<R: DmaWord> DmaReader<R> for AdcFifo<R> {
    #[inline]
    fn rx_req(&self) -> Option<u8> {
        Some(0x24)
    }
    #[inline]
    fn rx_info(&self) -> (u32, u32) {
        (self.d.fifo().as_ptr() as u32, u32::MAX)
    }
    #[inline]
    fn rx_incremented(&self) -> bool {
        false
    }
}
impl<R: DmaWord> DmaReader<R> for &AdcFifo<R> {
    #[inline]
    fn rx_req(&self) -> Option<u8> {
        Some(0x24)
    }
    #[inline]
    fn rx_info(&self) -> (u32, u32) {
        (self.d.fifo().as_ptr() as u32, u32::MAX)
    }
    #[inline]
    fn rx_incremented(&self) -> bool {
        false
    }
}
impl<R: DmaWord> DmaReader<R> for &mut AdcFifo<R> {
    #[inline]
    fn rx_req(&self) -> Option<u8> {
        Some(0x24)
    }
    #[inline]
    fn rx_info(&self) -> (u32, u32) {
        (self.d.fifo().as_ptr() as u32, u32::MAX)
    }
    #[inline]
    fn rx_incremented(&self) -> bool {
        false
    }
}

fn prepare_adc() {
    with(|x| {
        let v = READY.borrow_mut(x);
        if !*v {
            prepare_adc_inner();
            *v = true;
        }
    })
}
#[inline]
fn prepare_adc_inner() {
    // Init the ADC clock.
    let c = unsafe { CLOCKS::steal() };
    while c.clk_adc_ctrl().read().enable().bit_is_set() {
        nop();
    }
    delay(100);
    c.clk_adc_div().modify(|_, r| unsafe { r.bits(DIV) });
    c.clk_adc_ctrl()
        .modify(|_, r| r.auxsrc().rosc_clksrc_ph().enable().set_bit());
    while c.clk_adc_ctrl().read().enable().bit_is_clear() {
        nop();
    }
    let r = unsafe { RESETS::steal() };
    r.reset().modify(|_, r| r.adc().set_bit());
    r.reset().modify(|_, r| r.adc().clear_bit());
    while r.reset_done().read().adc().bit_is_clear() {
        nop();
    }
    let d = unsafe { ADC::steal() };
    d.cs().write(|r| r.en().set_bit());
    while d.cs().read().ready().bit_is_clear() {
        nop();
    }
}
