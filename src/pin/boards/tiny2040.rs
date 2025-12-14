// AUTOMATICALLY GENERATED: DO NOT EDIT!
//
// Use the boards/generate.py script to generate this file.
//

#![no_implicit_prelude]
#![cfg(feature = "tiny2040")]

extern crate core;

use core::option::Option::{self, None, Some};

use crate::pin::pwm::PwmID;
use crate::pin::{I2cID, SpiID, UartID};

/// Pins for "Tiny 2040"
#[repr(u8)]
pub enum PinID {
    Pin0 = 0x0u8,
    Pin1 = 0x1u8,
    Pin2 = 0x2u8,
    Pin3 = 0x3u8,
    Pin4 = 0x4u8,
    Pin5 = 0x5u8,
    Pin6 = 0x6u8,
    Pin7 = 0x7u8,
    /// ADC Pin0
    Pin26 = 0x1Au8,
    /// ADC Pin1
    Pin27 = 0x1Bu8,
    /// ADC Pin2
    Pin28 = 0x1Cu8,
    /// ADC Pin3
    Pin29 = 0x1Du8,
}

#[inline]
pub(crate) fn pins_pwm(pin: &PinID) -> PwmID {
    match pin {
        PinID::Pin0 => PwmID::Pwm0A,
        PinID::Pin1 => PwmID::Pwm0B,
        PinID::Pin2 => PwmID::Pwm1A,
        PinID::Pin3 => PwmID::Pwm1B,
        PinID::Pin4 => PwmID::Pwm2A,
        PinID::Pin5 => PwmID::Pwm2B,
        PinID::Pin6 => PwmID::Pwm3A,
        PinID::Pin7 => PwmID::Pwm3B,
        PinID::Pin26 => PwmID::Pwm5A,
        PinID::Pin27 => PwmID::Pwm5B,
        PinID::Pin28 => PwmID::Pwm6A,
        PinID::Pin29 => PwmID::Pwm6B,
    }
}
#[inline]
pub(crate) fn pins_i2c(sda: &PinID, scl: &PinID) -> Option<I2cID> {
    let d = match sda {
        PinID::Pin0 => I2cID::I2C0,
        PinID::Pin2 => I2cID::I2C1,
        PinID::Pin4 => I2cID::I2C0,
        PinID::Pin6 => I2cID::I2C1,
        PinID::Pin26 => I2cID::I2C1,
        PinID::Pin28 => I2cID::I2C0,
        _ => return None,
    };
    match (&d, scl) {
        (I2cID::I2C0, PinID::Pin1) => (),
        (I2cID::I2C1, PinID::Pin3) => (),
        (I2cID::I2C0, PinID::Pin5) => (),
        (I2cID::I2C1, PinID::Pin7) => (),
        (I2cID::I2C1, PinID::Pin27) => (),
        (..) => return None,
    }
    Some(d)
}
#[inline]
pub(crate) fn pins_spi(tx: &PinID, sck: &PinID, rx: Option<&PinID>, cs: Option<&PinID>) -> Option<SpiID> {
    let d = match tx {
        PinID::Pin3 => SpiID::Spi0,
        PinID::Pin7 => SpiID::Spi0,
        PinID::Pin27 => SpiID::Spi1,
        _ => return None,
    };
    match (&d, sck) {
        (SpiID::Spi0, PinID::Pin2) => (),
        (SpiID::Spi0, PinID::Pin6) => (),
        (SpiID::Spi1, PinID::Pin26) => (),
        (..) => return None,
    }
    if rx.is_none() && cs.is_none() {
        return Some(d);
    }
    match (&d, rx) {
        (_, None) => (),
        (SpiID::Spi0, Some(PinID::Pin0)) => (),
        (SpiID::Spi0, Some(PinID::Pin4)) => (),
        (SpiID::Spi1, Some(PinID::Pin28)) => (),
        (..) => return None,
    }
    match (&d, cs) {
        (_, None) => (),
        (SpiID::Spi0, Some(PinID::Pin1)) => (),
        (SpiID::Spi0, Some(PinID::Pin5)) => (),
        (..) => return None,
    }
    Some(d)
}
#[inline]
pub(crate) fn pins_uart(tx: &PinID, rx: &PinID, cts: Option<&PinID>, rts: Option<&PinID>) -> Option<UartID> {
    let d = match tx {
        PinID::Pin0 => UartID::Uart0,
        PinID::Pin4 => UartID::Uart1,
        PinID::Pin28 => UartID::Uart0,
        _ => return None,
    };
    match (&d, rx) {
        (UartID::Uart0, PinID::Pin1) => (),
        (UartID::Uart1, PinID::Pin5) => (),
        (..) => return None,
    }
    if cts.is_none() && rts.is_none() {
        return Some(d);
    }
    match (&d, cts) {
        (_, None) => (),
        (UartID::Uart0, Some(PinID::Pin2)) => (),
        (UartID::Uart1, Some(PinID::Pin6)) => (),
        (UartID::Uart1, Some(PinID::Pin26)) => (),
        (..) => return None,
    }
    match (&d, rts) {
        (_, None) => (),
        (UartID::Uart0, Some(PinID::Pin3)) => (),
        (UartID::Uart1, Some(PinID::Pin7)) => (),
        (UartID::Uart1, Some(PinID::Pin27)) => (),
        (..) => return None,
    }
    Some(d)
}
