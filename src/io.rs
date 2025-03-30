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

use core::fmt::{self, Debug, Formatter};
use core::marker::Sized;
use core::result::Result::{self, Err, Ok};

pub enum Error<E> {
    // Generic IO
    Read,
    Write,
    Timeout,
    EndOfFile,
    UnexpectedEof,

    // Generic FileSystem
    NoSpace,
    NotAFile,
    NotFound,
    Overflow,
    NotReadable,
    NotWritable,
    NotADirectory,
    NonEmptyDirectory,

    // Generic Options
    InvalidIndex,
    InvalidOptions,

    // Other
    Other(E),
}
pub enum SeekFrom {
    End(i64),
    Start(u64),
    Current(i64),
}

pub trait Seek<E> {
    fn seek(&mut self, s: SeekFrom) -> Result<u64, Error<E>>;

    #[inline]
    fn rewind(&mut self) -> Result<(), Error<E>> {
        self.seek(SeekFrom::Start(0))?;
        Ok(())
    }
    #[inline]
    fn stream_position(&mut self) -> Result<u64, Error<E>> {
        self.seek(SeekFrom::Current(0))
    }
}
pub trait Read<E> {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, Error<E>>;

    fn read_exact(&mut self, mut b: &mut [u8]) -> Result<(), Error<E>> {
        while !b.is_empty() {
            match self.read(b) {
                Ok(0) => break,
                Ok(n) => b = &mut b[n..],
                Err(e) => return Err(e),
            }
        }
        if b.is_empty() { Ok(()) } else { Err(Error::UnexpectedEof) }
    }
}
pub trait Write<E> {
    fn flush(&mut self) -> Result<(), Error<E>>;
    fn write(&mut self, b: &[u8]) -> Result<usize, Error<E>>;

    fn write_all(&mut self, mut b: &[u8]) -> Result<(), Error<E>> {
        while !b.is_empty() {
            match self.write(b) {
                Ok(0) => return Err(Error::EndOfFile),
                Ok(n) => b = &b[n..],
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    #[cfg(feature = "debug")]
    fn write_fmt(&mut self, fmt: core::fmt::Arguments<'_>) -> Result<(), core::fmt::Error> {
        struct Adapter<'a, E, T: Write<E> + ?Sized + 'a> {
            i: &'a mut T,
            e: Result<(), Error<E>>,
        }
        impl<E, T: Write<E> + ?Sized> core::fmt::Write for Adapter<'_, E, T> {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                match self.i.write_all(s.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        self.e = Err(e);
                        Err(core::fmt::Error)
                    },
                }
            }
        }
        let mut o = Adapter { i: self, e: Ok(()) };
        core::fmt::write(&mut o, fmt)
    }
}

impl<E, T: ?Sized + Seek<E>> Seek<E> for &mut T {
    #[inline(always)]
    fn seek(&mut self, p: SeekFrom) -> Result<u64, Error<E>> {
        T::seek(self, p)
    }
}
impl<E, T: ?Sized + Read<E>> Read<E> for &mut T {
    #[inline(always)]
    fn read(&mut self, b: &mut [u8]) -> Result<usize, Error<E>> {
        T::read(self, b)
    }
}
impl<E, T: ?Sized + Write<E>> Write<E> for &mut T {
    #[inline(always)]
    fn flush(&mut self) -> Result<(), Error<E>> {
        T::flush(self)
    }
    #[inline(always)]
    fn write(&mut self, b: &[u8]) -> Result<usize, Error<E>> {
        T::write(self, b)
    }
}

#[cfg(feature = "debug")]
impl<E: Debug> Debug for Error<E> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Read => f.write_str("Read"),
            Error::Write => f.write_str("Write"),
            Error::Timeout => f.write_str("Timeout"),
            Error::EndOfFile => f.write_str("EndOfFile"),
            Error::UnexpectedEof => f.write_str("UnexpectedEof"),
            Error::NoSpace => f.write_str("NoSpace"),
            Error::NotAFile => f.write_str("NotAFile"),
            Error::NotFound => f.write_str("NotFound"),
            Error::Overflow => f.write_str("Overflow"),
            Error::NotReadable => f.write_str("NotReadable"),
            Error::NotWritable => f.write_str("NotWritable"),
            Error::NotADirectory => f.write_str("NotADirectory"),
            Error::NonEmptyDirectory => f.write_str("NonEmptyDirectory"),
            Error::InvalidIndex => f.write_str("InvalidIndex"),
            Error::InvalidOptions => f.write_str("InvalidOptions"),
            Error::Other(v) => f.debug_tuple("Other").field(v).finish(),
        }
    }
}
#[cfg(not(feature = "debug"))]
impl<E> Debug for Error<E> {
    #[inline(always)]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}
