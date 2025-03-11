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

use core::convert::TryFrom;
use core::default::Default;
use core::fmt::{self, Debug, Formatter, Write};
use core::marker::Send;
use core::option::Option::{self, None, Some};
use core::ptr::NonNull;
use core::result::Result::{self, Err, Ok};
use core::unreachable;

use crate::Pico;
use crate::asm::nop;
use crate::dma::{DmaReader, DmaWriter};
use crate::pac::uart0::RegisterBlock;
use crate::pac::{RESETS, UART0, UART1};
use crate::pin::{PinFunction, PinID, UartID, pins_uart};

#[repr(u8)]
pub enum UartBits {
    Five  = 0x0u8,
    Six   = 0x1u8,
    Seven = 0x2u8,
    Eight = 0x3u8,
}
pub enum UartError {
    InvalidPins,
    InvalidBaudRate,
    ReadBreak,
    ReadOverrun,
    ReadInvalid,
    WouldBlock,
}
pub enum UartParity {
    None,
    Odd,
    Even,
}
#[repr(u8)]
pub enum UartStopBits {
    One = 0x0u8,
    Two = 0x1u8,
}
pub enum UartWatermark {
    Bytes4,
    Bytes8,
    Bytes16,
    Bytes24,
    Bytes28,
}

pub struct Uart {
    dev: NonNull<RegisterBlock>,
}
pub struct UartDev {
    pub tx:  PinID,
    pub rx:  PinID,
    pub cts: Option<PinID>,
    pub rts: Option<PinID>,
}
pub struct UartConfig {
    pub parity:    UartParity,
    pub data_bits: UartBits,
    pub stop_bits: UartStopBits,
}

impl Uart {
    pub fn new(p: &Pico, baudrate: u32, cfg: UartConfig, d: UartDev) -> Result<Uart, UartError> {
        let (i, f) = calc_dvs(baudrate, p.system_freq())?;
        let v = d.device().ok_or(UartError::InvalidPins)?;
        unsafe {
            let t = &*v;
            t.uartibrd().write(|r| r.baud_divint().bits(i));
            t.uartfbrd().write(|r| r.baud_divfrac().bits(f as u8));
            t.uartlcr_h().modify(|_, r| r);
            t.uartlcr_h().write(|r| {
                r.fen().set_bit();
                match cfg.parity {
                    UartParity::None => r.pen().bit(false),
                    UartParity::Odd => r.eps().clear_bit(),
                    UartParity::Even => r.eps().set_bit(),
                };
                r.wlen().bits(cfg.data_bits as u8).stp2().bit((cfg.stop_bits as u8) == 1)
            });
            t.uartcr().write(|r| {
                r.uarten()
                    .set_bit()
                    .txe()
                    .set_bit()
                    .rxe()
                    .set_bit()
                    .ctsen()
                    .bit(d.cts.is_some())
                    .rtsen()
                    .bit(d.rts.is_some())
            });
            t.uartdmacr().write(|r| {
                r.txdmae().set_bit();
                r.rxdmae().set_bit()
            })
        }
        d.rx.set_input();
        d.rx.set_function(PinFunction::Uart);
        d.tx.set_output();
        d.tx.set_function(PinFunction::Uart);
        if let Some(x) = d.cts.as_ref() {
            x.set_input();
            x.set_function(PinFunction::Uart);
        }
        if let Some(x) = d.rts.as_ref() {
            x.set_output();
            x.set_function(PinFunction::Uart);
        }
        Ok(Uart {
            dev: unsafe { NonNull::new_unchecked(v as *mut RegisterBlock) },
        })
    }

    #[inline]
    pub fn close(&mut self) {
        self.ptr().uartcr().write(|r| {
            r.uarten()
                .clear_bit()
                .txe()
                .clear_bit()
                .rxe()
                .clear_bit()
                .ctsen()
                .clear_bit()
                .rtsen()
                .clear_bit()
        })
    }
    #[inline]
    pub fn is_busy(&self) -> bool {
        self.ptr().uartfr().read().busy().bit_is_set()
    }
    #[inline]
    pub fn is_writable(&self) -> bool {
        self.ptr().uartfr().read().txff().bit_is_clear()
    }
    #[inline]
    pub fn is_readable(&self) -> bool {
        self.ptr().uartfr().read().rxfe().bit_is_clear()
    }
    #[inline]
    pub fn set_fifos(&mut self, en: bool) {
        self.ptr().uartlcr_h().modify(|_, r| r.fen().bit(en))
    }
    #[inline]
    pub fn set_tx_interrupt(&mut self, en: bool) {
        if en {
            self.ptr().uartifls().modify(|_, r| unsafe { r.txiflsel().bits(0x2) });
        }
        self.ptr().uartimsc().modify(|_, r| r.txim().bit(en))
    }
    #[inline]
    pub fn set_rx_interrupt(&mut self, en: bool) {
        self.ptr().uartimsc().modify(|_, r| {
            r.rxim().bit(en);
            r.rtim().bit(en)
        })
    }
    pub fn write_full(&mut self, b: &[u8]) -> usize {
        let mut n = 0;
        while n < b.len() {
            n += match self.write(&b[n..]) {
                Ok(n) => n,
                Err(UartError::WouldBlock) => continue,
                Err(_) => unreachable!(),
            }
        }
        n
    }
    #[inline]
    pub fn flush(&mut self) -> Result<(), UartError> {
        if self.ptr().uartfr().read().busy().bit_is_set() { Err(UartError::WouldBlock) } else { Ok(()) }
    }
    #[inline]
    pub fn set_tx_watermark(&mut self, w: UartWatermark) {
        self.ptr()
            .uartifls()
            .modify(|_, r| unsafe { r.txiflsel().bits(w.bits_tx()) })
    }
    #[inline]
    pub fn set_rx_watermark(&mut self, w: UartWatermark) {
        self.ptr()
            .uartifls()
            .modify(|_, r| unsafe { r.rxiflsel().bits(w.bits_rx()) })
    }
    pub fn write(&mut self, b: &[u8]) -> Result<usize, UartError> {
        let mut n = 0usize;
        let p = self.ptr();
        for i in b {
            if !self.is_writable() {
                return if n == 0 { Err(UartError::WouldBlock) } else { Ok(n) };
            }
            p.uartdr().write(|r| unsafe { r.data().bits(*i) });
            n += 1;
        }
        Ok(n)
    }
    pub fn read(&mut self, b: &mut [u8]) -> Result<usize, UartError> {
        let mut n = 0usize;
        let p = self.ptr();
        while n < b.len() {
            if !self.is_readable() {
                return if n == 0 { Err(UartError::WouldBlock) } else { Ok(n) };
            }
            let v = p.uartdr().read().bits();
            match v {
                _ if (v >> 0xB) & 1 != 0 => return Err(UartError::ReadOverrun),
                _ if (v >> 0xA) & 1 != 0 => return Err(UartError::ReadBreak),
                _ if (v >> 0x9) & 1 != 0 => return Err(UartError::ReadInvalid),
                _ if (v >> 0x8) & 1 != 0 => return Err(UartError::ReadInvalid),
                _ => (),
            }
            b[n] = (v & 0xFF) as u8;
            n += 1;
        }
        Ok(n)
    }
    pub fn read_full(&mut self, b: &mut [u8]) -> Result<usize, UartError> {
        let mut n = 0;
        while n < b.len() {
            n += match self.read(&mut b[n..]) {
                Ok(n) => n,
                Err(UartError::WouldBlock) => continue,
                Err(e) => return Err(e),
            };
        }
        Ok(n)
    }

    #[inline(always)]
    fn ptr(&self) -> &RegisterBlock {
        unsafe { self.dev.as_ref() }
    }
}
impl UartDev {
    #[inline]
    pub fn new(tx: PinID, rx: PinID) -> Result<UartDev, UartError> {
        let d = UartDev { tx, rx, cts: None, rts: None };
        d.id().ok_or(UartError::InvalidPins)?;
        Ok(d)
    }
    #[inline]
    pub fn new_cts(tx: PinID, rx: PinID, cts: PinID, rts: PinID) -> Result<UartDev, UartError> {
        let d = UartDev {
            tx,
            rx,
            cts: Some(cts),
            rts: Some(rts),
        };
        d.id().ok_or(UartError::InvalidPins)?;
        Ok(d)
    }

    #[inline(always)]
    fn id(&self) -> Option<UartID> {
        pins_uart(&self.tx, &self.rx, self.cts.as_ref(), self.rts.as_ref())
    }
    fn device(&self) -> Option<*const RegisterBlock> {
        let v = match self.id() {
            None => return None,
            Some(v) => v,
        };
        let r = unsafe { RESETS::steal() };
        match v {
            UartID::Uart0 => {
                r.reset().modify(|_, r| r.uart0().set_bit());
                r.reset().modify(|_, r| r.uart0().clear_bit());
                while r.reset_done().read().uart0().bit_is_clear() {
                    nop();
                }
                Some(UART0::PTR)
            },
            UartID::Uart1 => {
                r.reset().modify(|_, r| r.uart1().set_bit());
                r.reset().modify(|_, r| r.uart1().clear_bit());
                while r.reset_done().read().uart1().bit_is_clear() {
                    nop();
                }
                Some(UART1::PTR)
            },
        }
    }
}
impl UartConfig {
    pub const DEFAULT_BAUDRATE: u32 = 115_200u32;

    #[inline(always)]
    pub const fn new() -> UartConfig {
        UartConfig {
            parity:    UartParity::None,
            data_bits: UartBits::Eight,
            stop_bits: UartStopBits::One,
        }
    }

    #[inline]
    pub const fn data(mut self, d: UartBits) -> UartConfig {
        self.data_bits = d;
        self
    }
    #[inline]
    pub const fn stop(mut self, s: UartStopBits) -> UartConfig {
        self.stop_bits = s;
        self
    }
    #[inline]
    pub const fn parity(mut self, p: UartParity) -> UartConfig {
        self.parity = p;
        self
    }
}
impl UartWatermark {
    #[inline(always)]
    fn bits_tx(&self) -> u8 {
        match self {
            UartWatermark::Bytes4 => 0x4u8,
            UartWatermark::Bytes8 => 0x3u8,
            UartWatermark::Bytes16 => 0x2u8,
            UartWatermark::Bytes24 => 0x1u8,
            UartWatermark::Bytes28 => 0x0u8,
        }
    }
    #[inline(always)]
    fn bits_rx(&self) -> u8 {
        match self {
            UartWatermark::Bytes4 => 0x0u8,
            UartWatermark::Bytes8 => 0x1u8,
            UartWatermark::Bytes16 => 0x2u8,
            UartWatermark::Bytes24 => 0x3u8,
            UartWatermark::Bytes28 => 0x4u8,
        }
    }
}

impl Write for Uart {
    #[inline(always)]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_full(s.as_bytes());
        Ok(())
    }
}

impl Default for UartConfig {
    #[inline(always)]
    fn default() -> UartConfig {
        UartConfig::new()
    }
}

impl TryFrom<(PinID, PinID)> for UartDev {
    type Error = UartError;

    #[inline(always)]
    fn try_from(v: (PinID, PinID)) -> Result<UartDev, UartError> {
        UartDev::new(v.0, v.1)
    }
}
impl TryFrom<(PinID, PinID, PinID, PinID)> for UartDev {
    type Error = UartError;

    #[inline(always)]
    fn try_from(v: (PinID, PinID, PinID, PinID)) -> Result<UartDev, UartError> {
        UartDev::new_cts(v.0, v.1, v.2, v.3)
    }
}

impl DmaReader<u8> for Uart {
    #[inline]
    fn rx_req(&self) -> Option<u8> {
        Some(if self.dev.as_ptr().addr() == UART0::PTR.addr() { 0x15 } else { 0x17 })
    }
    #[inline(always)]
    fn rx_info(&self) -> (u32, u32) {
        (self.ptr().uartdr().as_ptr() as u32, u32::MAX)
    }
    #[inline(always)]
    fn rx_incremented(&self) -> bool {
        false
    }
}
impl DmaWriter<u8> for Uart {
    #[inline]
    fn tx_req(&self) -> Option<u8> {
        Some(if self.dev.as_ptr().addr() == UART0::PTR.addr() { 0x14 } else { 0x16 })
    }
    #[inline(always)]
    fn tx_info(&self) -> (u32, u32) {
        (self.ptr().uartdr().as_ptr() as u32, u32::MAX)
    }
    #[inline(always)]
    fn tx_incremented(&self) -> bool {
        false
    }
}

unsafe impl Send for Uart {}

#[cfg(feature = "debug")]
impl Debug for UartError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            UartError::WouldBlock => f.write_str("WouldBlock"),
            UartError::InvalidPins => f.write_str("InvalidPins"),
            UartError::InvalidBaudRate => f.write_str("InvalidBaudRate"),
            UartError::ReadBreak => f.write_str("ReadBreak"),
            UartError::ReadOverrun => f.write_str("ReadOverrun"),
            UartError::ReadInvalid => f.write_str("ReadInvalid"),
        }
    }
}
#[cfg(not(feature = "debug"))]
impl Debug for UartError {
    #[inline(always)]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

#[inline]
fn calc_dvs(w: u32, f: u32) -> Result<(u16, u16), UartError> {
    let r = f
        .checked_mul(0x8)
        .and_then(|v| v.checked_div(w))
        .ok_or(UartError::InvalidBaudRate)?;
    match (r >> 7, ((r & 0x7F) + 1) / 2) {
        (0, _) => Ok((1u16, 0u16)),
        (x, _) if x >= 0xFFFF => Ok((0xFFFFu16, 0u16)),
        (x, y) => Ok((x as u16, y as u16)),
    }
}
