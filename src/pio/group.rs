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

use core::ops::{Deref, DerefMut};

use crate::pio::state::{Running, Stopped};
use crate::pio::{PioState, PioStateOccupied, State};

pub struct StateGroup2<'a, S: PioState>(State<'a, S>, State<'a, S>);
pub struct StateGroup3<'a, S: PioState>(State<'a, S>, State<'a, S>, State<'a, S>);
pub struct StateGroup4<'a, S: PioState>(State<'a, S>, State<'a, S>, State<'a, S>, State<'a, S>);

impl<'a> StateGroup2<'a, Running> {
    #[inline]
    pub fn stop(self) -> StateGroup2<'a, Stopped> {
        self.0.ctrl(self.mask(), true);
        StateGroup2(self.0.stopped(), self.1.stopped())
    }
}
impl<'a> StateGroup3<'a, Running> {
    #[inline]
    pub fn stop(self) -> StateGroup3<'a, Stopped> {
        self.0.ctrl(self.mask(), true);
        StateGroup3(self.0.stopped(), self.1.stopped(), self.2.stopped())
    }
}
impl<'a> StateGroup4<'a, Running> {
    #[inline]
    pub fn stop(self) -> StateGroup4<'a, Stopped> {
        self.0.ctrl(self.mask(), true);
        StateGroup4(
            self.0.stopped(),
            self.1.stopped(),
            self.2.stopped(),
            self.3.stopped(),
        )
    }
}
impl<'a> StateGroup2<'a, Stopped> {
    #[inline]
    pub fn start(self) -> StateGroup2<'a, Running> {
        self.0.ctrl(self.mask(), false);
        StateGroup2(self.0.started(), self.1.started())
    }
}
impl<'a> StateGroup3<'a, Stopped> {
    #[inline]
    pub fn start(self) -> StateGroup3<'a, Running> {
        self.0.ctrl(self.mask(), false);
        StateGroup3(self.0.started(), self.1.started(), self.2.started())
    }
}
impl<'a> StateGroup4<'a, Stopped> {
    #[inline]
    pub fn start(self) -> StateGroup4<'a, Running> {
        self.0.ctrl(self.mask(), false);
        StateGroup4(
            self.0.started(),
            self.1.started(),
            self.2.started(),
            self.3.started(),
        )
    }
}
impl<'a, S: PioState> StateGroup2<'a, S> {
    #[inline]
    pub fn free(self) -> (State<'a, S>, State<'a, S>) {
        (self.0, self.1)
    }
    #[inline]
    pub fn add(self, other: State<'a, S>) -> StateGroup3<'a, S> {
        StateGroup3(self.0, self.1, other)
    }

    #[inline]
    pub(super) fn new(state1: State<'a, S>, state2: State<'a, S>) -> StateGroup2<'a, S> {
        StateGroup2(state1, state2)
    }

    #[inline]
    fn mask(&self) -> u32 {
        unsafe { 1u32.unchecked_shl(self.0.m.idx as u32) | 1u32.unchecked_shl(self.1.m.idx as u32) }
    }
}
impl<'a, S: PioState> StateGroup3<'a, S> {
    #[inline]
    pub fn pop(self) -> (StateGroup2<'a, S>, State<'a, S>) {
        (StateGroup2(self.0, self.1), self.2)
    }
    #[inline]
    pub fn add(self, other: State<'a, S>) -> StateGroup4<'a, S> {
        StateGroup4(self.0, self.1, self.2, other)
    }
    #[inline]
    pub fn free(self) -> (State<'a, S>, State<'a, S>, State<'a, S>) {
        (self.0, self.1, self.2)
    }

    #[inline]
    fn mask(&self) -> u32 {
        unsafe { 1u32.unchecked_shl(self.0.m.idx as u32) | 1u32.unchecked_shl(self.1.m.idx as u32) | 1u32.unchecked_shl(self.2.m.idx as u32) }
    }
}
impl<'a, S: PioState> StateGroup4<'a, S> {
    #[inline]
    pub fn pop(self) -> (StateGroup3<'a, S>, State<'a, S>) {
        (StateGroup3(self.0, self.1, self.2), self.3)
    }
    #[inline]
    pub fn free(self) -> (State<'a, S>, State<'a, S>, State<'a, S>, State<'a, S>) {
        (self.0, self.1, self.2, self.3)
    }

    #[inline]
    fn mask(&self) -> u32 {
        unsafe { 1u32.unchecked_shl(self.0.m.idx as u32) | 1u32.unchecked_shl(self.1.m.idx as u32) | 1u32.unchecked_shl(self.2.m.idx as u32) | 1u32.unchecked_shl(self.3.m.idx as u32) }
    }
}
impl<'a, S: PioStateOccupied> StateGroup2<'a, S> {
    #[inline]
    pub fn sync(&mut self) {
        self.0.m.ctrl(unsafe { self.mask().unchecked_shl(8) }, false)
    }
}
impl<'a, S: PioStateOccupied> StateGroup3<'a, S> {
    #[inline]
    pub fn sync(&mut self) {
        self.0.m.ctrl(unsafe { self.mask().unchecked_shl(8) }, false)
    }
}
impl<'a, S: PioStateOccupied> StateGroup4<'a, S> {
    #[inline]
    pub fn sync(&mut self) {
        self.0.m.ctrl(unsafe { self.mask().unchecked_shl(8) }, false)
    }
}

impl<'a, S: PioState> Deref for StateGroup2<'a, S> {
    type Target = State<'a, S>;

    #[inline]
    fn deref(&self) -> &State<'a, S> {
        &self.0
    }
}
impl<'a, S: PioState> DerefMut for StateGroup2<'a, S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut State<'a, S> {
        &mut self.0
    }
}

impl<'a, S: PioState> Deref for StateGroup3<'a, S> {
    type Target = State<'a, S>;

    #[inline]
    fn deref(&self) -> &State<'a, S> {
        &self.0
    }
}
impl<'a, S: PioState> DerefMut for StateGroup3<'a, S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut State<'a, S> {
        &mut self.0
    }
}

impl<'a, S: PioState> Deref for StateGroup4<'a, S> {
    type Target = State<'a, S>;

    #[inline]
    fn deref(&self) -> &State<'a, S> {
        &self.0
    }
}
impl<'a, S: PioState> DerefMut for StateGroup4<'a, S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut State<'a, S> {
        &mut self.0
    }
}
