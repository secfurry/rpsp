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
use core::default::Default;
use core::marker::Copy;
use core::matches;
use core::option::Option::{None, Some};

use crate::pin::PinID;
use crate::pio::state::{Stopped, Uninit};
use crate::pio::{Handle, State};

pub enum Fifo {
    Tx,
    Rx,
    Both,
}
pub enum Shift {
    Left,
    Right,
}
pub enum Source {
    Tx,
    Rx,
}

pub struct Config {
    /// Program start addess.
    pub origin:                u8,
    /// FIFO join allocations. If not set to 'Both', only 'Tx' or 'Rx' can be
    /// enabled, depending on the set selection.
    pub fifo:                  Fifo,
    /// Source to use when checking status.
    pub status_src:            Source,
    /// Comparision level to use when checking the 'status_src' source.
    pub status_level:          u8,
    /// Clock Divisor top integer representation.
    /// Divisor formula is sys_freq/( div_int + (div_frac/256) )
    pub clock_div_int:         u16,
    /// Clock Divisor bottom fraction representation.
    /// Divisor formula is sys_freq/( div_int + (div_frac/256) )
    pub clock_div_frac:        u8,
    /// When this address is reached, the program will jump to 'wrap_bottom'.
    pub wrap_top:              u8,
    /// Address to jump to when 'wrap_top' is reached.
    pub wrap_bottom:           u8,
    /// If true, Side-Set MSB (Most Significant Bit) is used as an Enable Flag
    /// otherwise (or false), Side-Set MSB is treated as a Side-Set data bit.
    pub sideset_as_enable:     bool,
    /// If true, Side-Set data affects PINDIR (Pin Direction) instructions
    /// otherwise (or false), Side-Set data affects Pin values.
    pub sideset_as_directions: bool,
    /// Pin that triggers a JMP instruction.
    pub jump_pin:              PinID,
    /// Pins affected by Side-Sets. 'sideset_pin' is the first Pin and
    /// 'sideset_pin_count' stores the count. must be consecutive.
    pub sideset_pin:           PinID,
    pub sideset_pin_count:     u8,
    /// Pins affected by SET instructions. 'set_pin' is the first Pin and
    /// 'set_pin_count' stores the count. must be consecutive.
    pub set_pin:               PinID,
    pub set_pin_count:         u8,
    /// Pins affected by OUT instructions. 'output_pin' is the first Pin and
    /// 'output_pin_count' stores the count. must be consecutive.
    pub output_pin:            PinID,
    pub output_pin_count:      u8,
    /// Pins affected by IN instructions. 'input_pin' is the first Pin. The
    /// count for this isn't stored.
    pub input_pin:             PinID,
    /// Input source configuration.
    /// If true, automatically push when the shift register is full.
    pub push_auto:             bool,
    /// Shift Input Direction
    pub push_shift:            Shift,
    /// Number of bits shifted before an autopush.
    pub push_threshold:        u8,
    /// Output source configuration.
    /// If true, automatically pull when the shift register is full.
    pub pull_auto:             bool,
    /// Shift Output Direction
    pub pull_shift:            Shift,
    /// Number of bits shifted before an autopull.
    pub pull_threshold:        u8,
    /// Constantly send the last OUT/SET instruction result to the pins.
    pub sticky_output:         bool,
    /// If true, the bit position in 'inline_out_bit' is treated as a 'write
    /// enable' when 'sticky_output' is true.
    pub inline_out_enable:     bool,
    /// The bit position to use for 'write enable' when 'inline_out_enable' is
    /// true.
    pub inline_out_bit:        u8,
}

impl Config {
    #[inline]
    pub const fn new() -> Config {
        Config {
            origin:                0u8,
            fifo:                  Fifo::Both,
            status_src:            Source::Tx,
            status_level:          0u8,
            clock_div_int:         1u16,
            clock_div_frac:        0u8,
            wrap_top:              0x1Fu8,
            wrap_bottom:           0u8,
            sideset_as_enable:     false,
            sideset_as_directions: false,
            jump_pin:              PinID::Pin0,
            sideset_pin:           PinID::Pin0,
            sideset_pin_count:     0u8,
            set_pin:               PinID::Pin0,
            set_pin_count:         0u8,
            output_pin:            PinID::Pin0,
            output_pin_count:      0u8,
            input_pin:             PinID::Pin0,
            push_auto:             false,
            push_shift:            Shift::Right,
            push_threshold:        0u8,
            pull_auto:             false,
            pull_shift:            Shift::Right,
            pull_threshold:        0u8,
            sticky_output:         false,
            inline_out_enable:     false,
            inline_out_bit:        0u8,
        }
    }
    #[inline]
    pub const fn new_with(h: &Handle) -> Config {
        Config {
            origin:                h.offset,
            fifo:                  Fifo::Both,
            status_src:            Source::Tx,
            status_level:          0u8,
            clock_div_int:         1u16,
            clock_div_frac:        0u8,
            wrap_top:              h.src.saturating_add(h.offset),
            wrap_bottom:           h.target.saturating_add(h.offset),
            sideset_as_enable:     false,
            sideset_as_directions: false,
            jump_pin:              PinID::Pin0,
            sideset_pin:           PinID::Pin0,
            sideset_pin_count:     0u8,
            set_pin:               PinID::Pin0,
            set_pin_count:         0u8,
            output_pin:            PinID::Pin0,
            output_pin_count:      0u8,
            input_pin:             PinID::Pin0,
            push_auto:             false,
            push_shift:            Shift::Right,
            push_threshold:        0u8,
            pull_auto:             false,
            pull_shift:            Shift::Right,
            pull_threshold:        0u8,
            sticky_output:         false,
            inline_out_enable:     false,
            inline_out_bit:        0u8,
        }
    }
    #[inline]
    pub const fn origin(mut self, addr: u8) -> Config {
        self.origin = addr;
        self
    }
    #[inline]
    pub const fn program(mut self, h: &Handle) -> Config {
        self.set_program(h);
        self
    }
    #[inline]
    pub const fn pull_auto(mut self, en: bool) -> Config {
        self.pull_auto = en;
        self
    }
    #[inline]
    pub const fn push_auto(mut self, en: bool) -> Config {
        self.push_auto = en;
        self
    }
    #[inline]
    pub const fn fifo_alloc(mut self, v: Fifo) -> Config {
        self.fifo = v;
        self
    }
    #[inline]
    pub const fn set_pin(mut self, pin: PinID) -> Config {
        self.set_pin = pin;
        self.set_pin_count = 1u8;
        self
    }
    #[inline]
    pub const fn jump_pin(mut self, pin: PinID) -> Config {
        self.jump_pin = pin;
        self
    }
    #[inline]
    pub const fn pull_shift(mut self, v: Shift) -> Config {
        self.pull_shift = v;
        self
    }
    #[inline]
    pub const fn push_shift(mut self, v: Shift) -> Config {
        self.push_shift = v;
        self
    }
    #[inline]
    pub const fn pull_threshold(mut self, v: u8) -> Config {
        self.pull_threshold = v;
        self
    }
    #[inline]
    pub const fn push_threshold(mut self, v: u8) -> Config {
        self.push_threshold = v;
        self
    }
    #[inline]
    pub const fn input_pin(mut self, pin: PinID) -> Config {
        self.input_pin = pin;
        self
    }
    #[inline]
    pub const fn output_pin(mut self, pin: PinID) -> Config {
        self.output_pin = pin;
        self.output_pin_count = 1u8;
        self
    }
    #[inline]
    pub const fn sticky_output(mut self, en: bool) -> Config {
        self.sticky_output = en;
        self
    }
    #[inline]
    pub const fn clock_div_float(mut self, v: f32) -> Config {
        self.clock_div_int = v as u16;
        self.clock_div_frac = (((self.clock_div_int as f32) - v) * 255f32) as u8;
        self
    }
    #[inline]
    pub const fn sideset_pin(mut self, pin: PinID) -> Config {
        self.sideset_pin = pin;
        self.sideset_pin_count = 1u8;
        self
    }
    #[inline]
    pub const fn set_pins(mut self, pins: &[PinID]) -> Config {
        // set_pin_count has a max of 5.
        self.set_pin_count = if pins.len() > 5 { 5u8 } else { pins.len() as u8 };
        self.set_pin = match pins.first().copied() {
            Some(v) => v,
            None => PinID::Pin0,
        };
        self
    }
    #[inline]
    pub const fn wrap(mut self, top: u8, bottom: u8) -> Config {
        self.wrap_top = top;
        self.wrap_bottom = bottom;
        self
    }
    #[inline]
    pub const fn input_pins(mut self, pins: &[PinID]) -> Config {
        self.input_pin = match pins.first().copied() {
            Some(v) => v,
            None => PinID::Pin0,
        };
        self
    }
    #[inline]
    pub const fn output_pins(mut self, pins: &[PinID]) -> Config {
        self.output_pin_count = pins.len() as u8;
        self.output_pin = match pins.first().copied() {
            Some(v) => v,
            None => PinID::Pin0,
        };
        self
    }
    #[inline]
    pub const fn sideset_as_enable(mut self, en: bool) -> Config {
        self.sideset_as_enable = en;
        self
    }
    #[inline]
    pub const fn clock_div(mut self, int: u16, frac: u8) -> Config {
        self.clock_div_int = int;
        self.clock_div_frac = frac;
        self
    }
    #[inline]
    pub const fn sideset_pins(mut self, pins: &[PinID]) -> Config {
        self.sideset_pin_count = pins.len() as u8;
        self.sideset_pin = match pins.first().copied() {
            Some(v) => v,
            None => PinID::Pin0,
        };
        self
    }
    #[inline]
    pub const fn status(mut self, level: u8, source: Source) -> Config {
        self.status_src = source;
        self.status_level = level;
        self
    }
    #[inline]
    pub const fn sideset_as_pin_directions(mut self, en: bool) -> Config {
        self.sideset_as_directions = en;
        self
    }
    #[inline]
    pub const fn inline_output(mut self, en: bool, bit_pos: u8) -> Config {
        self.inline_out_enable = en;
        self.inline_out_bit = bit_pos;
        self
    }
    #[inline]
    pub const fn pull(mut self, auto: bool, thresh: u8, shift: Shift) -> Config {
        self.pull_auto = auto;
        self.pull_shift = shift;
        self.pull_threshold = thresh;
        self
    }
    #[inline]
    pub const fn push(mut self, auto: bool, thresh: u8, shift: Shift) -> Config {
        self.push_auto = auto;
        self.push_shift = shift;
        self.push_threshold = thresh;
        self
    }

    #[inline]
    pub const fn set_program(&mut self, h: &Handle) {
        self.origin = h.offset;
        self.wrap_top = h.wrap_src_adjusted();
        self.wrap_bottom = h.wrap_target_adjusted();
    }

    pub fn configure<'a>(&self, mut s: State<'a, Uninit>) -> State<'a, Stopped> {
        s.set_state(false);
        let v = s.m.sm();
        unsafe {
            v.sm_clkdiv()
                .write(|r| r.int().bits(self.clock_div_int).frac().bits(self.clock_div_frac));
            v.sm_execctrl().write(|r| {
                r.side_en().bit(self.sideset_as_enable);
                r.side_pindir().bit(self.sideset_as_directions);
                r.jmp_pin().bits(self.jump_pin as u8);
                r.inline_out_en().bit(self.inline_out_enable);
                r.out_en_sel().bits(self.inline_out_bit);
                r.out_sticky().bit(self.sticky_output);
                r.wrap_top().bits(self.wrap_top);
                r.wrap_bottom().bits(self.wrap_bottom);
                r.status_sel().bit(matches!(self.status_src, Source::Rx));
                r.status_n().bits(self.status_level)
            });
            v.sm_shiftctrl().write(|r| {
                match self.fifo {
                    Fifo::Rx => r.fjoin_rx().bit(true).fjoin_tx().bit(false),
                    Fifo::Tx => r.fjoin_rx().bit(false).fjoin_tx().bit(true),
                    Fifo::Both => r.fjoin_rx().bit(false).fjoin_tx().bit(false),
                };
                r.pull_thresh().bits(self.pull_threshold);
                r.push_thresh().bits(self.push_threshold);
                r.out_shiftdir().bit(matches!(self.pull_shift, Shift::Right));
                r.in_shiftdir().bit(matches!(self.push_shift, Shift::Right));
                r.autopull().bit(self.pull_auto);
                r.autopush().bit(self.push_auto)
            });
            v.sm_pinctrl().write(|r| {
                r.sideset_count().bits(self.sideset_pin_count);
                r.set_count().bits(self.set_pin_count);
                r.out_count().bits(self.output_pin_count);
                r.in_base().bits(self.input_pin as u8);
                r.sideset_base().bits(self.sideset_pin as u8);
                r.set_base().bits(self.set_pin as u8);
                r.out_base().bits(self.output_pin as u8)
            });
        }
        // Jump to entrance
        v.sm_instr().write(|r| unsafe { r.sm0_instr().bits(self.origin as u16) });
        // Clear state
        s.restart();
        s.restart_clock();
        s.init()
    }
}

impl Copy for Fifo {}
impl Clone for Fifo {
    #[inline]
    fn clone(&self) -> Fifo {
        *self
    }
}

impl Copy for Source {}
impl Clone for Source {
    #[inline]
    fn clone(&self) -> Source {
        *self
    }
}

impl Copy for Shift {}
impl Clone for Shift {
    #[inline]
    fn clone(&self) -> Shift {
        *self
    }
}

impl Default for Config {
    #[inline]
    fn default() -> Config {
        Config::new()
    }
}
