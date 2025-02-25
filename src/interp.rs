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

use core::convert::{From, Into};
use core::default::Default;
use core::marker::PhantomData;

use crate::pac::SIO;

pub struct Num0;
pub struct Num1;
pub struct Lane0;
pub struct Lane1;
pub struct LaneConfig {
    pub shift:        u8,
    pub clamp:        bool,
    pub blend:        bool,
    pub signed:       bool,
    pub msb_mask:     u8,
    pub lsb_mask:     u8,
    pub msb_force:    u8,
    pub input_cross:  bool,
    pub result_cross: bool,
    pub add_with_raw: bool,
}
pub struct Interpoler<S: InterpolerSlot> {
    lane0: Lane<S, Lane0>,
    lane1: Lane<S, Lane1>,
    _p:    PhantomData<S>,
}
pub struct Lane<S: InterpolerSlot, N: InterpolerSlotLane>(PhantomData<*const (S, N)>);

pub trait InterpolerSlot {}
pub trait InterpolerSlotLane {}

pub type Interpoler0 = Interpoler<Num0>;
pub type Interpoler1 = Interpoler<Num1>;

impl LaneConfig {
    #[inline(always)]
    pub const fn new() -> LaneConfig {
        LaneConfig {
            shift:        0,
            clamp:        false,
            blend:        false,
            signed:       false,
            msb_mask:     0x1F,
            lsb_mask:     0,
            msb_force:    0,
            input_cross:  false,
            result_cross: false,
            add_with_raw: false,
        }
    }

    #[inline]
    pub const fn as_ctrl(&self) -> u32 {
        (if self.clamp { 0x400000u32 } else { 0u32 })
            | (if self.blend { 0x200000u32 } else { 0u32 })
            | ((self.msb_force as u32) << 19)
            | (if self.add_with_raw { 0x40000u32 } else { 0u32 })
            | (if self.result_cross { 0x20000u32 } else { 0u32 })
            | (if self.input_cross { 0x10000u32 } else { 0u32 })
            | (if self.signed { 0x8000u32 } else { 0u32 })
            | ((self.msb_mask as u32) << 10)
            | ((self.lsb_mask as u32) << 5)
            | (self.shift as u32)
    }
    #[inline]
    pub const fn shift(mut self, v: u8) -> LaneConfig {
        self.shift = v;
        self
    }
    #[inline]
    pub const fn clamp(mut self, v: bool) -> LaneConfig {
        self.clamp = v;
        self
    }
    #[inline]
    pub const fn blend(mut self, v: bool) -> LaneConfig {
        self.blend = v;
        self
    }
    #[inline]
    pub const fn signed(mut self, v: bool) -> LaneConfig {
        self.signed = v;
        self
    }
    #[inline]
    pub const fn msb_mask(mut self, v: u8) -> LaneConfig {
        self.msb_mask = v;
        self
    }
    #[inline]
    pub const fn lsb_mask(mut self, v: u8) -> LaneConfig {
        self.lsb_mask = v;
        self
    }
    #[inline]
    pub const fn msb_force(mut self, v: u8) -> LaneConfig {
        self.msb_force = v;
        self
    }
    #[inline]
    pub const fn input_cross(mut self, v: bool) -> LaneConfig {
        self.input_cross = v;
        self
    }
    #[inline]
    pub const fn result_cross(mut self, v: bool) -> LaneConfig {
        self.result_cross = v;
        self
    }
    #[inline]
    pub const fn add_with_raw(mut self, v: bool) -> LaneConfig {
        self.add_with_raw = v;
        self
    }
}
impl Interpoler<Num0> {
    #[inline(always)]
    pub const fn get() -> Interpoler0 {
        Interpoler {
            lane0: Lane(PhantomData),
            lane1: Lane(PhantomData),
            _p:    PhantomData,
        }
    }

    #[inline]
    pub fn peek(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_peek_full().read().bits()
    }
    #[inline]
    pub fn base(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_base2().read().bits()
    }
    #[inline]
    pub fn pop(&mut self) -> u32 {
        unsafe { SIO::steal() }.interp0_pop_full().read().bits()
    }
    #[inline]
    pub fn base_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp0_base2().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn base_set_both(&mut self, v: u32) {
        unsafe { SIO::steal().interp0_base_1and0().write(|r| r.bits(v)) }
    }
}
impl Interpoler<Num1> {
    #[inline(always)]
    pub const fn get() -> Interpoler1 {
        Interpoler {
            lane0: Lane(PhantomData),
            lane1: Lane(PhantomData),
            _p:    PhantomData,
        }
    }

    #[inline]
    pub fn peek(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_peek_full().read().bits()
    }
    #[inline]
    pub fn base(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_base2().read().bits()
    }
    #[inline]
    pub fn pop(&mut self) -> u32 {
        unsafe { SIO::steal() }.interp1_pop_full().read().bits()
    }
    #[inline]
    pub fn base_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp1_base2().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn base_set_both(&mut self, v: u32) {
        unsafe { SIO::steal().interp1_base_1and0().write(|r| r.bits(v)) }
    }
}
impl Lane<Num0, Lane0> {
    #[inline]
    pub fn peek(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_peek_lane0().read().bits()
    }
    #[inline]
    pub fn base(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_base0().read().bits()
    }
    #[inline]
    pub fn ctrl(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_ctrl_lane0().read().bits()
    }
    #[inline]
    pub fn read(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_accum0_add().read().bits()
    }
    #[inline]
    pub fn pop(&mut self) -> u32 {
        unsafe { SIO::steal() }.interp0_pop_lane0().read().bits()
    }
    #[inline]
    pub fn add(&mut self, v: u32) {
        unsafe { SIO::steal().interp0_accum0_add().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn accumulator(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_accum0().read().bits()
    }
    #[inline]
    pub fn base_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp0_base0().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn accumulator_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp0_accum0_add().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn ctrl_set(&mut self, v: impl Into<u32>) {
        unsafe { SIO::steal().interp0_ctrl_lane0().write(|r| r.bits(v.into())) }
    }
}
impl Lane<Num0, Lane1> {
    #[inline]
    pub fn peek(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_peek_lane1().read().bits()
    }
    #[inline]
    pub fn base(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_base1().read().bits()
    }
    #[inline]
    pub fn ctrl(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_ctrl_lane1().read().bits()
    }
    #[inline]
    pub fn read(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_accum1_add().read().bits()
    }
    #[inline]
    pub fn pop(&mut self) -> u32 {
        unsafe { SIO::steal() }.interp0_pop_lane1().read().bits()
    }
    #[inline]
    pub fn add(&mut self, v: u32) {
        unsafe { SIO::steal().interp0_accum1_add().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn accumulator(&self) -> u32 {
        unsafe { SIO::steal() }.interp0_accum1().read().bits()
    }
    #[inline]
    pub fn base_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp0_base1().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn accumulator_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp0_accum1_add().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn ctrl_set(&mut self, v: impl Into<u32>) {
        unsafe { SIO::steal().interp0_ctrl_lane1().write(|r| r.bits(v.into())) }
    }
}
impl Lane<Num1, Lane0> {
    #[inline]
    pub fn peek(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_peek_lane0().read().bits()
    }
    #[inline]
    pub fn base(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_base0().read().bits()
    }
    #[inline]
    pub fn ctrl(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_ctrl_lane0().read().bits()
    }
    #[inline]
    pub fn read(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_accum0_add().read().bits()
    }
    #[inline]
    pub fn pop(&mut self) -> u32 {
        unsafe { SIO::steal() }.interp1_pop_lane0().read().bits()
    }
    #[inline]
    pub fn add(&mut self, v: u32) {
        unsafe { SIO::steal().interp1_accum0_add().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn accumulator(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_accum0().read().bits()
    }
    #[inline]
    pub fn base_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp1_base0().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn accumulator_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp1_accum0_add().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn ctrl_set(&mut self, v: impl Into<u32>) {
        unsafe { SIO::steal().interp1_ctrl_lane0().write(|r| r.bits(v.into())) }
    }
}
impl Lane<Num1, Lane1> {
    #[inline]
    pub fn peek(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_peek_lane1().read().bits()
    }
    #[inline]
    pub fn base(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_base1().read().bits()
    }
    #[inline]
    pub fn ctrl(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_ctrl_lane1().read().bits()
    }
    #[inline]
    pub fn read(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_accum1_add().read().bits()
    }
    #[inline]
    pub fn pop(&mut self) -> u32 {
        unsafe { SIO::steal() }.interp1_pop_lane1().read().bits()
    }
    #[inline]
    pub fn add(&mut self, v: u32) {
        unsafe { SIO::steal().interp1_accum1_add().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn accumulator(&self) -> u32 {
        unsafe { SIO::steal() }.interp1_accum1().read().bits()
    }
    #[inline]
    pub fn base_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp1_base1().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn accumulator_set(&mut self, v: u32) {
        unsafe { SIO::steal().interp1_accum1_add().write(|r| r.bits(v)) }
    }
    #[inline]
    pub fn ctrl_set(&mut self, v: impl Into<u32>) {
        unsafe { SIO::steal().interp1_ctrl_lane1().write(|r| r.bits(v.into())) }
    }
}
impl<S: InterpolerSlot> Interpoler<S> {
    #[inline(always)]
    pub const fn lane0(&mut self) -> &mut Lane<S, Lane0> {
        &mut self.lane0
    }
    #[inline(always)]
    pub const fn lane1(&mut self) -> &mut Lane<S, Lane1> {
        &mut self.lane1
    }
}

impl InterpolerSlot for Num0 {}
impl InterpolerSlot for Num1 {}
impl InterpolerSlotLane for Lane0 {}
impl InterpolerSlotLane for Lane1 {}

impl Default for LaneConfig {
    #[inline(always)]
    fn default() -> LaneConfig {
        LaneConfig::new()
    }
}

impl From<LaneConfig> for u32 {
    #[inline(always)]
    fn from(v: LaneConfig) -> u32 {
        v.as_ctrl()
    }
}
impl From<&LaneConfig> for u32 {
    #[inline(always)]
    fn from(v: &LaneConfig) -> u32 {
        v.as_ctrl()
    }
}
impl From<&mut LaneConfig> for u32 {
    #[inline(always)]
    fn from(v: &mut LaneConfig) -> u32 {
        v.as_ctrl()
    }
}
