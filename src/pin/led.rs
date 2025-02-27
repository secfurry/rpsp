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
use core::ops::{Deref, DerefMut};

use crate::Pico;
use crate::pin::gpio::Output;
use crate::pin::pwm::PwmPin;
use crate::pin::{Pin, PinID};

pub struct Led(Pin<Output>);
pub struct LedPwm(PwmPin<Output>);

impl Led {
    #[inline(always)]
    pub fn get(p: &Pico, i: PinID) -> Led {
        Pin::get(p, i).into()
    }

    #[inline(always)]
    pub fn on(&self) {
        self.0.high()
    }
    #[inline(always)]
    pub fn off(&self) {
        self.0.low()
    }
}
impl LedPwm {
    #[inline(always)]
    pub fn get(p: &Pico, i: PinID) -> LedPwm {
        Pin::get(p, i).into_pwm().into()
    }

    #[inline(always)]
    pub fn on(&self) {
        self.0.high()
    }
    #[inline(always)]
    pub fn off(&self) {
        self.0.low()
    }
    #[inline(always)]
    pub fn brightness(&self, p: u8) {
        self.0.set_duty((self.0.get_max_duty() / 100u16) * (p as u16))
    }
}

impl Deref for Led {
    type Target = Pin<Output>;

    #[inline(always)]
    fn deref(&self) -> &Pin<Output> {
        &self.0
    }
}
impl DerefMut for Led {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Pin<Output> {
        &mut self.0
    }
}
impl From<Pin<Output>> for Led {
    #[inline(always)]
    fn from(v: Pin<Output>) -> Led {
        Led(v)
    }
}

impl Deref for LedPwm {
    type Target = PwmPin<Output>;

    #[inline(always)]
    fn deref(&self) -> &PwmPin<Output> {
        &self.0
    }
}
impl DerefMut for LedPwm {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut PwmPin<Output> {
        &mut self.0
    }
}
impl From<PwmPin<Output>> for LedPwm {
    #[inline(always)]
    fn from(v: PwmPin<Output>) -> LedPwm {
        LedPwm(v)
    }
}
