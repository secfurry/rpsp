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

use core::convert::{From, TryFrom};
use core::default::Default;
use core::fmt::{self, Debug, Formatter};
use core::iter::Iterator;
use core::marker::{PhantomData, Send};
use core::ops::{Deref, DerefMut};
use core::option::Option::{self, None, Some};
use core::ptr::NonNull;
use core::result::Result::{self, Err, Ok};
use core::{cmp, matches};

use crate::Pico;
use crate::asm::nop;
use crate::dma::{DmaReader, DmaWriter};
use crate::pac::spi0::RegisterBlock;
use crate::pac::{RESETS, SPI0, SPI1};
use crate::pin::{PinFunction, PinID, SpiID, pins_spi};

pub enum SpiError {
    WouldBlock,
    InvalidPins,
    InvalidFrequency,
}
pub enum SpiPhase {
    First,
    Second,
}
#[repr(u8)]
pub enum SpiFormat {
    Motorola              = 0x00u8,
    TexasInstruments      = 0x01u8,
    NationalSemiconductor = 0x10u8,
}
pub enum SpiPolarity {
    Low,
    High,
}

pub enum SpiBus<'a> {
    Owned(Spi),
    Shared(&'a mut Spi),
    Duplicated((Spi, PhantomData<&'a Spi>)),
}

pub struct Spi {
    dev: NonNull<RegisterBlock>,
}
pub struct SpiDev {
    pub tx:  PinID,
    pub sck: PinID,
    pub cs:  Option<PinID>,
    pub rx:  Option<PinID>,
}
pub struct SpiConfig {
    pub bits:     u8,
    pub phase:    SpiPhase,
    pub format:   SpiFormat,
    pub primary:  bool,
    pub polarity: SpiPolarity,
}

pub trait SpiIO<T: Default> {
    fn write(&mut self, b: &[T]);
    fn recv_single(&mut self) -> Option<T>;
    fn transfer_single(&mut self, v: T) -> T;
    fn read_with(&mut self, v: T, b: &mut [T]);
    fn transfer_in_place(&mut self, b: &mut [T]);
    fn send_single(&mut self, v: T) -> Result<(), SpiError>;
    fn transfer(&mut self, input: &[T], out: &mut [T]) -> usize;

    #[inline(always)]
    fn read_single(&mut self) -> T {
        self.transfer_single(T::default())
    }
    #[inline(always)]
    fn read(&mut self, b: &mut [T]) {
        self.read_with(T::default(), b)
    }
    #[inline(always)]
    fn write_single(&mut self, v: T) {
        let _ = self.transfer_single(v);
    }
}
pub trait SpiByte: SpiIO<u8> {}
pub trait SpiShort: SpiIO<u16> {}

impl Spi {
    pub fn new(p: &Pico, baudrate: u32, cfg: SpiConfig, d: SpiDev) -> Result<Spi, SpiError> {
        let (b, mut k) = (p.system_freq(), u8::MAX);
        for i in (2u32..=0xFEu32).step_by(2) {
            if b < ((i + 2) * 0x100u32).saturating_mul(baudrate) {
                k = i as u8;
                break;
            }
        }
        if k == u8::MAX {
            return Err(SpiError::InvalidFrequency);
        }
        let mut j = 0u8;
        for i in (1u8..=0xFFu8).rev() {
            if b / (k as u32 * i as u32) > baudrate {
                j = i;
                break;
            }
        }
        let v = d.device().ok_or(SpiError::InvalidPins)?;
        unsafe {
            let t = &*v;
            t.sspcpsr().write(|r| r.cpsdvsr().bits(k));
            let f = cfg.format as u8;
            t.sspcr0().modify(|_, r| {
                r.scr().bits(j).dss().bits(cfg.bits - 1).frf().bits(f);
                if f == 0 {
                    r.spo()
                        .bit(matches!(cfg.polarity, SpiPolarity::High))
                        .sph()
                        .bit(matches!(cfg.phase, SpiPhase::Second));
                }
                r
            });
            t.sspcr1().modify(|_, r| r.ms().bit(!cfg.primary));
            t.sspdmacr().modify(|_, r| r.txdmae().set_bit().rxdmae().set_bit());
            t.sspcr1().modify(|_, r| r.sse().set_bit());
        }
        d.tx.set_output();
        d.tx.set_function(PinFunction::Spi);
        if let Some(x) = d.rx.as_ref() {
            x.set_input();
            x.set_function(PinFunction::Spi);
        }
        if cfg.primary {
            d.sck.set_output();
        } else {
            d.sck.set_input();
        }
        d.sck.set_function(PinFunction::Spi);
        if let Some(x) = d.cs.as_ref() {
            if cfg.primary {
                x.set_output();
            } else {
                x.set_input();
            }
            x.set_function(PinFunction::Spi);
        }
        Ok(Spi {
            dev: unsafe { NonNull::new_unchecked(v as *mut RegisterBlock) },
        })
    }

    #[inline]
    pub fn flush(&mut self) {
        while self.is_busy() {
            nop();
        }
    }
    #[inline]
    pub fn close(&mut self) {
        self.ptr().sspcr1().modify(|_, r| r.sse().clear_bit());
    }
    #[inline]
    pub fn is_busy(&self) -> bool {
        self.ptr().sspsr().read().bsy().bit_is_set()
    }
    #[inline]
    pub fn is_writable(&self) -> bool {
        self.ptr().sspsr().read().tnf().bit_is_set()
    }
    #[inline]
    pub fn is_readable(&self) -> bool {
        self.ptr().sspsr().read().rne().bit_is_set()
    }

    #[inline(always)]
    fn ptr(&self) -> &RegisterBlock {
        unsafe { self.dev.as_ref() }
    }
}
impl SpiDev {
    #[inline]
    pub fn new(tx: PinID, sck: PinID) -> Result<SpiDev, SpiError> {
        let d = SpiDev { tx, sck, cs: None, rx: None };
        d.id().ok_or(SpiError::InvalidPins)?;
        Ok(d)
    }
    #[inline]
    pub fn new_rx(tx: PinID, sck: PinID, rx: PinID) -> Result<SpiDev, SpiError> {
        let d = SpiDev { tx, sck, cs: None, rx: Some(rx) };
        d.id().ok_or(SpiError::InvalidPins)?;
        Ok(d)
    }
    #[inline]
    pub fn new_cs(tx: PinID, sck: PinID, cs: PinID, rx: PinID) -> Result<SpiDev, SpiError> {
        let d = SpiDev {
            tx,
            sck,
            cs: Some(cs),
            rx: Some(rx),
        };
        d.id().ok_or(SpiError::InvalidPins)?;
        Ok(d)
    }

    #[inline(always)]
    fn id(&self) -> Option<SpiID> {
        pins_spi(&self.tx, &self.sck, self.rx.as_ref(), self.cs.as_ref())
    }
    fn device(&self) -> Option<*const RegisterBlock> {
        let v = match self.id() {
            None => return None,
            Some(v) => v,
        };
        let r = unsafe { RESETS::steal() };
        match v {
            SpiID::Spi0 => {
                r.reset().modify(|_, r| r.spi0().set_bit());
                r.reset().modify(|_, r| r.spi0().clear_bit());
                while r.reset_done().read().spi0().bit_is_clear() {
                    nop();
                }
                Some(SPI0::PTR)
            },
            SpiID::Spi1 => {
                r.reset().modify(|_, r| r.spi1().set_bit());
                r.reset().modify(|_, r| r.spi1().clear_bit());
                while r.reset_done().read().spi1().bit_is_clear() {
                    nop();
                }
                Some(SPI1::PTR)
            },
        }
    }
}
impl SpiConfig {
    pub const DEFAULT_BAUD_RATE: u32 = 3_000_000u32;

    #[inline(always)]
    pub const fn new() -> SpiConfig {
        SpiConfig {
            bits:     8u8,
            phase:    SpiPhase::First,
            format:   SpiFormat::Motorola,
            primary:  true,
            polarity: SpiPolarity::Low,
        }
    }

    #[inline]
    pub const fn bits(mut self, v: u8) -> SpiConfig {
        self.bits = v;
        self
    }
    #[inline]
    pub const fn primary(mut self, p: bool) -> SpiConfig {
        self.primary = p;
        self
    }
    #[inline]
    pub const fn phase(mut self, p: SpiPhase) -> SpiConfig {
        self.phase = p;
        self
    }
    #[inline]
    pub const fn format(mut self, f: SpiFormat) -> SpiConfig {
        self.format = f;
        self
    }
    #[inline]
    pub const fn polarity(mut self, p: SpiPolarity) -> SpiConfig {
        self.polarity = p;
        self
    }
}

impl SpiIO<u8> for Spi {
    fn write(&mut self, b: &[u8]) {
        let p = self.ptr();
        for i in b {
            while p.sspsr().read().tnf().bit_is_clear() {
                nop();
            }
            p.sspdr().write(|r| unsafe { r.data().bits(*i as _) });
            while p.sspsr().read().rne().bit_is_clear() {
                nop();
            }
            let _ = p.sspdr().read().data().bits();
        }
    }
    #[inline]
    fn recv_single(&mut self) -> Option<u8> {
        if self.is_readable() { Some(self.ptr().sspdr().read().data().bits() as _) } else { None }
    }
    fn transfer_single(&mut self, v: u8) -> u8 {
        let p = self.ptr();
        while p.sspsr().read().tnf().bit_is_clear() {
            nop();
        }
        p.sspdr().write(|r| unsafe { r.data().bits(v as _) });
        while p.sspsr().read().rne().bit_is_clear() {
            nop();
        }
        p.sspdr().read().data().bits() as _
    }
    fn read_with(&mut self, v: u8, b: &mut [u8]) {
        let p = self.ptr();
        for i in b {
            while p.sspsr().read().tnf().bit_is_clear() {
                nop();
            }
            p.sspdr().write(|r| unsafe { r.data().bits(v as _) });
            while p.sspsr().read().rne().bit_is_clear() {
                nop();
            }
            *i = p.sspdr().read().data().bits() as _;
        }
    }
    fn transfer_in_place(&mut self, b: &mut [u8]) {
        let p = self.ptr();
        for i in b {
            while p.sspsr().read().tnf().bit_is_clear() {
                nop();
            }
            p.sspdr().write(|r| unsafe { r.data().bits(*i as _) });
            while p.sspsr().read().rne().bit_is_clear() {
                nop();
            }
            *i = p.sspdr().read().data().bits() as _;
        }
    }
    #[inline]
    fn send_single(&mut self, v: u8) -> Result<(), SpiError> {
        if !self.is_writable() {
            return Err(SpiError::WouldBlock);
        }
        self.ptr().sspdr().write(|r| unsafe { r.data().bits(v as _) });
        Ok(())
    }
    fn transfer(&mut self, input: &[u8], out: &mut [u8]) -> usize {
        let n = cmp::min(out.len(), input.len());
        let p = self.ptr();
        for i in 0..n {
            while p.sspsr().read().tnf().bit_is_clear() {
                nop();
            }
            p.sspdr().write(|r| unsafe { r.data().bits(input[i] as _) });
            while p.sspsr().read().rne().bit_is_clear() {
                nop();
            }
            out[i] = p.sspdr().read().data().bits() as _;
        }
        n
    }
}
impl SpiIO<u16> for Spi {
    fn write(&mut self, b: &[u16]) {
        let p = self.ptr();
        for i in b {
            while p.sspsr().read().tnf().bit_is_clear() {
                nop();
            }
            p.sspdr().write(|r| unsafe { r.data().bits(*i) });
            while p.sspsr().read().rne().bit_is_clear() {
                nop();
            }
            let _ = p.sspdr().read().data().bits();
        }
    }
    #[inline]
    fn recv_single(&mut self) -> Option<u16> {
        if self.is_readable() { Some(self.ptr().sspdr().read().data().bits() as _) } else { None }
    }
    fn transfer_single(&mut self, v: u16) -> u16 {
        let p = self.ptr();
        while p.sspsr().read().tnf().bit_is_clear() {
            nop();
        }
        p.sspdr().write(|r| unsafe { r.data().bits(v) });
        while p.sspsr().read().rne().bit_is_clear() {
            nop();
        }
        p.sspdr().read().data().bits()
    }
    fn read_with(&mut self, v: u16, b: &mut [u16]) {
        let p = self.ptr();
        for i in b {
            while p.sspsr().read().tnf().bit_is_clear() {
                nop();
            }
            p.sspdr().write(|r| unsafe { r.data().bits(v) });
            while p.sspsr().read().rne().bit_is_clear() {
                nop();
            }
            *i = p.sspdr().read().data().bits();
        }
    }
    fn transfer_in_place(&mut self, b: &mut [u16]) {
        let p = self.ptr();
        for i in b {
            while p.sspsr().read().tnf().bit_is_clear() {
                nop();
            }
            p.sspdr().write(|r| unsafe { r.data().bits(*i) });
            while p.sspsr().read().rne().bit_is_clear() {
                nop();
            }
            *i = p.sspdr().read().data().bits();
        }
    }
    #[inline]
    fn send_single(&mut self, v: u16) -> Result<(), SpiError> {
        if !self.is_writable() {
            return Err(SpiError::WouldBlock);
        }
        self.ptr().sspdr().write(|r| unsafe { r.data().bits(v) });
        Ok(())
    }
    fn transfer(&mut self, input: &[u16], out: &mut [u16]) -> usize {
        let n = cmp::min(out.len(), input.len());
        let p = self.ptr();
        for i in 0..n {
            while p.sspsr().read().tnf().bit_is_clear() {
                nop();
            }
            p.sspdr().write(|r| unsafe { r.data().bits(input[i]) });
            while p.sspsr().read().rne().bit_is_clear() {
                nop();
            }
            out[i] = p.sspdr().read().data().bits();
        }
        n
    }
}

impl Default for SpiConfig {
    #[inline(always)]
    fn default() -> SpiConfig {
        SpiConfig::new()
    }
}

impl Default for SpiFormat {
    #[inline(always)]
    fn default() -> SpiFormat {
        SpiFormat::Motorola
    }
}

impl TryFrom<(PinID, PinID)> for SpiDev {
    type Error = SpiError;

    #[inline(always)]
    fn try_from(v: (PinID, PinID)) -> Result<SpiDev, SpiError> {
        SpiDev::new(v.0, v.1)
    }
}
impl TryFrom<(PinID, PinID, PinID)> for SpiDev {
    type Error = SpiError;

    #[inline(always)]
    fn try_from(v: (PinID, PinID, PinID)) -> Result<SpiDev, SpiError> {
        SpiDev::new_rx(v.0, v.1, v.2)
    }
}
impl TryFrom<(PinID, PinID, PinID, PinID)> for SpiDev {
    type Error = SpiError;

    #[inline(always)]
    fn try_from(v: (PinID, PinID, PinID, PinID)) -> Result<SpiDev, SpiError> {
        SpiDev::new_cs(v.0, v.1, v.2, v.3)
    }
}

impl Deref for SpiBus<'_> {
    type Target = Spi;

    #[inline(always)]
    fn deref(&self) -> &Spi {
        match self {
            SpiBus::Owned(v) => &v,
            SpiBus::Shared(v) => v,
            SpiBus::Duplicated((v, _)) => &v,
        }
    }
}
impl DerefMut for SpiBus<'_> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Spi {
        match self {
            SpiBus::Owned(v) => v,
            SpiBus::Shared(v) => v,
            SpiBus::Duplicated((v, _)) => v,
        }
    }
}
impl<'a> From<Spi> for SpiBus<'a> {
    #[inline(always)]
    fn from(v: Spi) -> SpiBus<'a> {
        SpiBus::Owned(v)
    }
}
impl<'a> From<&'a Spi> for SpiBus<'a> {
    #[inline(always)]
    fn from(v: &'a Spi) -> SpiBus<'a> {
        SpiBus::Duplicated((Spi { dev: v.dev }, PhantomData))
    }
}
impl<'a> From<&'a mut Spi> for SpiBus<'a> {
    #[inline(always)]
    fn from(v: &'a mut Spi) -> SpiBus<'a> {
        SpiBus::Shared(v)
    }
}

impl DmaReader<u8> for Spi {
    #[inline]
    fn rx_req(&self) -> Option<u8> {
        Some(if self.dev.as_ptr().addr() == SPI0::PTR.addr() { 0x11 } else { 0x13 })
    }
    #[inline(always)]
    fn rx_info(&self) -> (u32, u32) {
        (self.ptr().sspdr().as_ptr() as u32, u32::MAX)
    }
    #[inline(always)]
    fn rx_incremented(&self) -> bool {
        false
    }
}
impl DmaReader<u16> for Spi {
    #[inline]
    fn rx_req(&self) -> Option<u8> {
        Some(if self.dev.as_ptr().addr() == SPI0::PTR.addr() { 0x11 } else { 0x13 })
    }
    #[inline(always)]
    fn rx_info(&self) -> (u32, u32) {
        (self.ptr().sspdr().as_ptr() as u32, u32::MAX)
    }
    #[inline(always)]
    fn rx_incremented(&self) -> bool {
        false
    }
}

impl DmaWriter<u8> for Spi {
    #[inline]
    fn tx_req(&self) -> Option<u8> {
        Some(if self.dev.as_ptr().addr() == SPI0::PTR.addr() { 0x10 } else { 0x12 })
    }
    #[inline]
    fn tx_info(&self) -> (u32, u32) {
        (self.ptr().sspdr().as_ptr() as u32, u32::MAX)
    }
    #[inline(always)]
    fn tx_incremented(&self) -> bool {
        false
    }
}
impl DmaWriter<u16> for Spi {
    #[inline]
    fn tx_req(&self) -> Option<u8> {
        Some(if self.dev.as_ptr().addr() == SPI0::PTR.addr() { 0x10 } else { 0x12 })
    }
    #[inline]
    fn tx_info(&self) -> (u32, u32) {
        (self.ptr().sspdr().as_ptr() as u32, u32::MAX)
    }
    #[inline(always)]
    fn tx_incremented(&self) -> bool {
        false
    }
}

unsafe impl Send for Spi {}

#[cfg(feature = "debug")]
impl Debug for SpiError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SpiError::WouldBlock => f.write_str("WouldBlock"),
            SpiError::InvalidPins => f.write_str("InvalidPins"),
            SpiError::InvalidFrequency => f.write_str("InvalidFrequency"),
        }
    }
}
#[cfg(not(feature = "debug"))]
impl Debug for SpiError {
    #[inline(always)]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}
