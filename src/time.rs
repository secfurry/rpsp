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
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::From;
use core::default::Default;
use core::marker::Copy;
use core::ops::FnOnce;
use core::option::Option;

const DAYS_IN_YEAR: [u16; 13] = [
    0x0, 0x1F, 0x3B, 0x5A, 0x78, 0x97, 0xB5, 0xD4, 0xF3, 0x111, 0x130, 0x14E, 0x16D,
];

#[repr(u8)]
pub enum Month {
    None      = 0,
    January   = 1,
    February  = 2,
    March     = 3,
    April     = 4,
    May       = 5,
    June      = 6,
    July      = 7,
    August    = 8,
    September = 9,
    October   = 10,
    November  = 11,
    December  = 12,
}
#[repr(u8)]
pub enum Weekday {
    Sunday    = 0,
    Monday    = 1,
    Tuesday   = 2,
    Wednesday = 3,
    Thursday  = 4,
    Friday    = 5,
    Saturday  = 6,
    None      = 7,
}

pub struct Time {
    pub day:     u8,
    pub year:    u16,
    pub mins:    u8,
    pub secs:    u8,
    pub hours:   u8,
    pub month:   Month,
    pub weekday: Weekday,
}

impl Time {
    #[inline(always)]
    pub const fn zero() -> Time {
        Time {
            day:     0u8,
            year:    0u16,
            mins:    0u8,
            secs:    0u8,
            hours:   0u8,
            month:   Month::None,
            weekday: Weekday::None,
        }
    }
    #[inline(always)]
    pub const fn empty() -> Time {
        Time {
            day:     1u8,
            year:    0u16,
            mins:    0u8,
            secs:    0u8,
            hours:   0u8,
            month:   Month::January,
            weekday: Weekday::None,
        }
    }
    #[inline(always)]
    pub const fn new(year: u16, month: Month, day: u8, hours: u8, mins: u8, secs: u8, weekday: Weekday) -> Time {
        Time {
            day,
            mins,
            secs,
            year,
            hours,
            month,
            weekday,
        }
    }

    #[inline(always)]
    pub fn is_valid(&self) -> bool {
        self.day >= 1 && self.day <= 31 && self.hours <= 23 && self.mins <= 59 && self.secs <= 59 && !self.month.is_none()
    }
    pub fn from_epoch(&self) -> i64 {
        let (y, v) = norm(self.year as i32, self.month as i32 - 1, 0xC);
        let (s, _) = norm(self.secs as i32, 0, 0x3B9ACA00);
        let (m, s) = norm(self.mins as i32, s, 0x3C);
        let (h, m) = norm(self.hours as i32, m, 0x3C);
        let (d, h) = norm(self.day as i32, h, 0x18);
        let v = v as usize + 1;
        let mut e = since_epoch(y) + DAYS_IN_YEAR[v - 1] as i64;
        if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) && v >= 3 {
            e += 1;
        }
        ((e + d as i64 - 1) * 0x15180) + (h * 0xE10 + m * 0x3C + s) as i64
    }
    #[inline]
    pub fn add_seconds(self, d: i64) -> Time {
        let e = self.from_epoch().wrapping_add(d);
        let (h, m, s) = clock(e);
        let (y, v, d) = date(e);
        let w = match (e.wrapping_add(0x15180) & 0x93A80) / 0x15180 {
            0 => Weekday::Sunday,
            1 => Weekday::Monday,
            2 => Weekday::Tuesday,
            3 => Weekday::Wednesday,
            4 => Weekday::Thursday,
            5 => Weekday::Friday,
            6 => Weekday::Saturday,
            _ => self.weekday,
        };
        Time::new(y, v, d, h, m, s, w)
    }
}
impl Month {
    #[inline(always)]
    pub fn is_none(&self) -> bool {
        match self {
            Month::None => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn map_or<U>(self, default: U, f: impl FnOnce(Month) -> U) -> U {
        match self {
            Month::None => default,
            _ => f(self),
        }
    }
}
impl Weekday {
    #[inline]
    pub fn from_time(t: &Time) -> Weekday {
        if !t.weekday.is_none() {
            return t.weekday;
        }
        match (t.from_epoch().wrapping_add(0x15180) & 0x93A80) / 0x15180 {
            0 => Weekday::Sunday,
            1 => Weekday::Monday,
            2 => Weekday::Tuesday,
            3 => Weekday::Wednesday,
            4 => Weekday::Thursday,
            5 => Weekday::Friday,
            6 => Weekday::Saturday,
            _ => Weekday::None,
        }
    }

    #[inline(always)]
    pub fn is_none(&self) -> bool {
        match self {
            Weekday::None => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn map_or<U>(self, default: U, f: impl FnOnce(Weekday) -> U) -> U {
        match self {
            Weekday::None => default,
            _ => f(self),
        }
    }
}

impl Eq for Time {}
impl Copy for Time {}
impl Clone for Time {
    #[inline(always)]
    fn clone(&self) -> Time {
        Time {
            day:     self.day,
            year:    self.year,
            mins:    self.mins,
            secs:    self.secs,
            hours:   self.hours,
            month:   self.month,
            weekday: self.weekday,
        }
    }
}
impl Default for Time {
    #[inline(always)]
    fn default() -> Time {
        Time::empty()
    }
}
impl PartialEq for Time {
    #[inline(always)]
    fn eq(&self, other: &Time) -> bool {
        self.day == other.day && self.year == other.year && self.mins == other.mins && self.secs == other.secs && self.hours == other.hours && self.month == other.month
    }
}

impl Eq for Month {}
impl Ord for Month {
    #[inline(always)]
    fn cmp(&self, other: &Month) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl Copy for Month {}
impl Clone for Month {
    #[inline(always)]
    fn clone(&self) -> Month {
        *self
    }
}
impl Default for Month {
    #[inline(always)]
    fn default() -> Month {
        Month::January
    }
}
impl From<u8> for Month {
    #[inline]
    fn from(v: u8) -> Month {
        match v {
            0x1 => Month::January,
            0x2 => Month::February,
            0x3 => Month::March,
            0x4 => Month::April,
            0x5 => Month::May,
            0x6 => Month::June,
            0x7 => Month::July,
            0x8 => Month::August,
            0x9 => Month::September,
            0xA => Month::October,
            0xB => Month::November,
            0xC => Month::December,
            _ => Month::None,
        }
    }
}
impl PartialEq for Month {
    #[inline(always)]
    fn eq(&self, other: &Month) -> bool {
        *self as u8 == *other as u8
    }
}
impl PartialOrd for Month {
    #[inline(always)]
    fn partial_cmp(&self, other: &Month) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}

impl Eq for Weekday {}
impl Ord for Weekday {
    #[inline(always)]
    fn cmp(&self, other: &Weekday) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl Copy for Weekday {}
impl Clone for Weekday {
    #[inline(always)]
    fn clone(&self) -> Weekday {
        *self
    }
}
impl Default for Weekday {
    #[inline(always)]
    fn default() -> Weekday {
        Weekday::Sunday
    }
}
impl From<u8> for Weekday {
    #[inline]
    fn from(v: u8) -> Weekday {
        match v {
            0 => Weekday::Sunday,
            1 => Weekday::Monday,
            2 => Weekday::Tuesday,
            3 => Weekday::Wednesday,
            4 => Weekday::Thursday,
            5 => Weekday::Friday,
            6 => Weekday::Saturday,
            _ => Weekday::None,
        }
    }
}
impl PartialEq for Weekday {
    #[inline(always)]
    fn eq(&self, other: &Weekday) -> bool {
        *self as u8 == *other as u8
    }
}
impl PartialOrd for Weekday {
    #[inline(always)]
    fn partial_cmp(&self, other: &Weekday) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}

#[inline]
fn since_epoch(year: i32) -> i64 {
    let mut y = year as i64 + 0x440D116EBF;
    let mut d = 0x23AB1 * (y / 0x190);
    y -= 0x190 * (y / 0x190);
    d += 0x8EAC * (y / 0x64);
    y -= 0x64 * (y / 0x64);
    d += 0x5B5 * (y / 0x4);
    y -= 0x4 * (y / 0x4);
    (d + (0x16D * y)) as i64
}
#[inline]
fn clock(epoch: i64) -> (u8, u8, u8) {
    let mut s = epoch % 0x15180;
    let h = s / 0xE10;
    s -= h * 0xE10;
    let m = s / 0x3C;
    (h as u8, m as u8, (s - (m * 0x3C)) as u8)
}
fn date(epoch: i64) -> (u16, Month, u8) {
    let mut d = epoch / 0x15180;
    let mut y = 0x190 * (d / 0x23AB1);
    d -= 0x23AB1 * (d / 0x23AB1);
    let mut n = d / 0x8EAC;
    n -= n >> 2;
    y += 0x64 * n;
    d -= 0x8EAC * n;
    y += 0x4 * (d / 0x5B5);
    d -= 0x5B5 * (d / 0x5B5);
    let mut n = d / 0x16D;
    n -= n >> 2;
    y += n;
    d -= 0x16D * n;
    let v = ((y as i64).wrapping_sub(0x440D116EBF)) as u16;
    let mut k = d as u16;
    if v % 4 == 0 && (v % 0x64 != 0 || v % 0x190 == 0) {
        if k == 0x3B {
            return (v as u16, Month::February, 29);
        } else if k > 0x3B {
            k -= 1
        }
    }
    let m = (k / 0x1F) as usize;
    let e = DAYS_IN_YEAR[m + 1];
    if k >= e {
        return (v, Month::from((m + 2) as u8), (k - e + 1) as u8);
    }
    (
        v,
        Month::from((m + 1) as u8),
        (k - DAYS_IN_YEAR[m] + 1) as u8,
    )
}
fn norm(hi: i32, low: i32, base: i32) -> (i32, i32) {
    let (mut x, mut y) = (hi, low);
    if y < 0 {
        let n = (-y - 1) / base + 1;
        x -= n;
        y += n * base;
    }
    if y >= base {
        let n = y / base;
        x += n;
        y -= n * base;
    }
    (x, y)
}

#[cfg(feature = "debug")]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, Result, Write};

    use crate::time::{Month, Time, Weekday};

    impl Debug for Time {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_struct("Time")
                .field("day", &self.day)
                .field("year", &self.year)
                .field("mins", &self.mins)
                .field("secs", &self.secs)
                .field("hours", &self.hours)
                .field("month", &self.month)
                .field("weekday", &self.weekday)
                .finish()
        }
    }
    impl Display for Time {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            if !self.weekday.is_none() {
                Display::fmt(&self.weekday, f)?;
                f.write_char(' ')?;
            }
            f.write_fmt(format_args!(
                "{:04}/{:02}/{:02}: {:02}:{:02};{:02}",
                self.year, self.month as u8, self.day, self.hours, self.mins, self.secs
            ))
        }
    }

    impl Debug for Month {
        #[inline(always)]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Month {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                Month::January => f.write_str("January"),
                Month::February => f.write_str("February"),
                Month::March => f.write_str("March"),
                Month::April => f.write_str("April"),
                Month::May => f.write_str("May"),
                Month::June => f.write_str("June"),
                Month::July => f.write_str("July"),
                Month::August => f.write_str("August"),
                Month::September => f.write_str("September"),
                Month::October => f.write_str("October"),
                Month::November => f.write_str("November"),
                Month::December => f.write_str("December"),
                Month::None => f.write_str("None"),
            }
        }
    }

    impl Debug for Weekday {
        #[inline(always)]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Weekday {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                Weekday::Sunday => f.write_str("Sunday"),
                Weekday::Monday => f.write_str("Monday"),
                Weekday::Tuesday => f.write_str("Tuesday"),
                Weekday::Wednesday => f.write_str("Wednesday"),
                Weekday::Thursday => f.write_str("Thursday"),
                Weekday::Friday => f.write_str("Friday"),
                Weekday::Saturday => f.write_str("Saturday"),
                Weekday::None => f.write_str("None"),
            }
        }
    }
}
