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
#![cfg(feature = "cyw")]

extern crate core;

use core::result::Result::{self, Err, Ok};

use crate::Board;
use crate::cyw::device::Device;
use crate::pin::{PinDirection, PinID, PinState};
use crate::pio::state::Stopped;
use crate::pio::{Config, Pio, PioID, Program, Shift, Slot, State};

mod data;
mod device;

const FREQ: u32 = 50_000_000u32;

#[derive(Debug)]
pub enum CywError {
    Code,
    NoBluetooth,
    InitFailure,
    InvalidFrequency,
}

pub struct Cyw43 {
    dev: Device,
}

impl Cyw43 {
    pub fn new(p: &Board) -> Result<Cyw43, CywError> {
        let m = p.system_freq();
        let (mut f, mut d, mut r) = (1u32, 0u32, 50_000_000u32);
        // Find a nice target frequency around ~50MHz.
        while r >= FREQ {
            r = m / (f + (d / 0x100));
            if r == FREQ {
                d += 1;
            } else {
                f += 1;
            }
            if f > 0xFFFF || d > 0xFF {
                return Err(CywError::InvalidFrequency);
            }
        }
        let c = Program::new(-1, 7, 0, [
            0x6001, //  0: out    pins, 1    side 0
            0x1040, //  1: jmp    x--, 0     side 1
            0xE080, //  2: set    pindirs, 0 side 0
            0xA042, //  3: nop               side 0
            0x5001, //  4: in     pins, 1    side 1
            0x0084, //  5: jmp    y--, 4     side 0
            0x20A0, //  6: wait   1 pin, 0   side 0
            0xC000, //  7: irq    nowait 0   side 0
        ]);
        let mut v = Pio::get(p, PioID::Pio0);
        let i = v.install(&c).or(Err(CywError::Code))?;
        let mut s = Config::new_program(&i)
            .sideset_pin(PinID::Pin29)
            .output_pin(PinID::Pin24)
            .input_pin(PinID::Pin24)
            .set_pin(PinID::Pin24)
            .pull(true, 0, Shift::Left)
            .push(true, 0, Shift::Left)
            .clock_div(f as u16, d as u8)
            .configure(unsafe { v.state_unsafe(Slot::Index0) });
        s.set_pins_direction(PinDirection::Out, &[PinID::Pin24, PinID::Pin29]);
        s.set_pins_state(PinState::Low, &[PinID::Pin24, PinID::Pin29]);
        s.set_pin_sync_bypass(PinID::Pin24);
        Ok(Cyw43 {
            dev: Device::new(p, i.offset(), s, PinID::Pin23, PinID::Pin25),
        })
    }
    #[inline]
    pub fn create(p: &Board, offset: u8, sm: State<'_, Stopped>, pwr: PinID, cs: PinID) -> Cyw43 {
        Cyw43 {
            dev: Device::new(p, offset, sm, pwr, cs),
        }
    }

    pub fn init(&mut self, bluetooth: bool) -> Result<(), CywError> {
        Ok(())
    }
}
