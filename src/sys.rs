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
use core::marker::Copy;

use crate::pac::VREG_AND_CHIP_RESET;

#[repr(u8)]
pub enum Voltage {
    Volts0_80 = 0x5u8,
    Volts0_85 = 0x6u8,
    Volts0_90 = 0x7u8,
    Volts0_95 = 0x8u8,
    Volts1_00 = 0x9u8,
    Volts1_05 = 0xAu8,
    Volts1_10 = 0xBu8,
    Volts1_15 = 0xCu8,
    Volts1_20 = 0xDu8,
    Volts1_25 = 0xEu8,
    Volts1_30 = 0xFu8,
}

#[inline]
pub fn voltage() -> Voltage {
    match unsafe { VREG_AND_CHIP_RESET::steal() }.vreg().read().vsel().bits() {
        0x6 => Voltage::Volts0_85,
        0x7 => Voltage::Volts0_90,
        0x8 => Voltage::Volts0_95,
        0x9 => Voltage::Volts1_00,
        0xA => Voltage::Volts1_05,
        0xB => Voltage::Volts1_10,
        0xC => Voltage::Volts1_15,
        0xD => Voltage::Volts1_20,
        0xE => Voltage::Volts1_25,
        0xF => Voltage::Volts1_30,
        _ => Voltage::Volts0_80,
    }
}
#[inline]
pub fn set_voltage(v: Voltage) {
    unsafe { VREG_AND_CHIP_RESET::steal() }
        .vreg()
        .write(|r| unsafe { r.vsel().bits(v as u8) });
}

impl Copy for Voltage {}
impl Clone for Voltage {
    #[inline(always)]
    fn clone(&self) -> Voltage {
        *self
    }
}
