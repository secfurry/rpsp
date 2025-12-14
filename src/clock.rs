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
use core::fmt::{self, Debug, Formatter};
use core::marker::Sized;
use core::num::{NonZeroU8, NonZeroU16};
use core::option::Option::{self, None, Some};
use core::result::Result;

use crate::i2c::I2cError;
use crate::time::{Month, Time, Weekday};

mod alarm;
mod rtc;
mod sys;

pub use self::alarm::*;
pub use self::rtc::*;
pub use self::sys::*;

pub enum RtcError {
    NotRunning,
    InvalidTime,
    ValueTooLarge,
    I2C(I2cError),
}

pub struct AlarmConfig {
    pub day:     Option<NonZeroU8>,
    pub mins:    Option<u8>,
    pub secs:    Option<u8>,
    pub year:    Option<NonZeroU16>,
    pub hours:   Option<u8>,
    pub month:   Month,
    pub weekday: Weekday,
}

pub trait TimeSource {
    type Error: Into<RtcError>;

    fn now(&mut self) -> Result<Time, Self::Error>;
}

impl AlarmConfig {
    #[inline]
    pub const fn new() -> AlarmConfig {
        AlarmConfig {
            day:     None,
            mins:    None,
            secs:    None,
            year:    None,
            hours:   None,
            month:   Month::None,
            weekday: Weekday::None,
        }
    }

    #[inline]
    pub const fn day(mut self, v: u8) -> AlarmConfig {
        self.day = NonZeroU8::new(v);
        self
    }
    #[inline]
    pub const fn mins(mut self, v: u8) -> AlarmConfig {
        self.mins = Some(v);
        self
    }
    #[inline]
    pub const fn secs(mut self, v: u8) -> AlarmConfig {
        self.secs = Some(v);
        self
    }
    #[inline]
    pub const fn hours(mut self, v: u8) -> AlarmConfig {
        self.hours = Some(v);
        self
    }
    #[inline]
    pub const fn year(mut self, v: u16) -> AlarmConfig {
        self.year = NonZeroU16::new(v);
        self
    }
    #[inline]
    pub const fn month(mut self, v: Month) -> AlarmConfig {
        self.month = v;
        self
    }
    #[inline]
    pub const fn weekday(mut self, v: Weekday) -> AlarmConfig {
        self.weekday = v;
        self
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.day.is_none() && self.mins.is_none() && self.secs.is_none() && self.hours.is_none() && self.weekday.is_none()
    }
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.day.is_none_or(|v| v.get() >= 1 && v.get() <= 31) && self.hours.is_none_or(|v| v <= 23) && self.mins.is_none_or(|v| v <= 59) && self.secs.is_none_or(|v| v <= 59)
    }
}

impl From<I2cError> for RtcError {
    #[inline]
    fn from(v: I2cError) -> RtcError {
        RtcError::I2C(v)
    }
}

impl<T: ?Sized + TimeSource> TimeSource for &mut T {
    type Error = T::Error;

    #[inline]
    fn now(&mut self) -> Result<Time, Self::Error> {
        T::now(self)
    }
}

impl Debug for RtcError {
    #[cfg(feature = "debug")]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            RtcError::NotRunning => f.write_str("NotRunning"),
            RtcError::InvalidTime => f.write_str("InvalidTime"),
            RtcError::ValueTooLarge => f.write_str("ValueTooLarge"),
            RtcError::I2C(v) => f.debug_tuple("I2C").field(v).finish(),
        }
    }
    #[cfg(not(feature = "debug"))]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Result::Ok(())
    }
}
