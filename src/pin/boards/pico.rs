// AUTOMATICALLY GENERATED: DO NOT EDIT!
//
// Use the boards/generate.py script to generate this file.
//

#![no_implicit_prelude]
#![cfg(feature = "pico")]

extern crate core;

use core::option::Option::{self, None, Some};

use crate::pin::pwm::PwmID;
use crate::pin::{I2cID, SpiID, UartID};

/// Pins for "Raspberry Pi Pico[W]"
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
    Pin8 = 0x8u8,
    Pin9 = 0x9u8,
    Pin10 = 0xAu8,
    Pin11 = 0xBu8,
    Pin12 = 0xCu8,
    Pin13 = 0xDu8,
    Pin14 = 0xEu8,
    Pin15 = 0xFu8,
    Pin16 = 0x10u8,
    Pin17 = 0x11u8,
    Pin18 = 0x12u8,
    Pin19 = 0x13u8,
    Pin20 = 0x14u8,
    Pin21 = 0x15u8,
    Pin22 = 0x16u8,
    /// Pin23 has different functions based on Pico/PicoW
    /// - Pico  : RT6150B-33GQW Power-Select
    /// - PicoW : Power enable for the Cyw Wireless chip
    Pin23 = 0x17u8,
    /// Pin24 has different functions based on Pico/PicoW
    /// - Pico  : VBUS Sense
    /// - PicoW : Cyw SPI Data/IRQ
    Pin24 = 0x18u8,
    /// Pin25 has different functions based on Pico/PicoW
    /// - Pico  : User LED
    /// - PicoW : Cyw SPI chip select
    Pin25 = 0x19u8,
    /// ADC Pin0
    Pin26 = 0x1Au8,
    /// ADC Pin1
    Pin27 = 0x1Bu8,
    /// ADC Pin2
    Pin28 = 0x1Cu8,
    /// Pin29 has different functions based on Pico/PicoW
    /// - Pico  : VSYS read pin, technically ADC Pin3
    /// - PicoW : Cyw SPI clock
    Pin29 = 0x1Du8,
}

#[inline(always)]
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
        PinID::Pin8 => PwmID::Pwm4A,
        PinID::Pin9 => PwmID::Pwm4B,
        PinID::Pin10 => PwmID::Pwm5A,
        PinID::Pin11 => PwmID::Pwm5B,
        PinID::Pin12 => PwmID::Pwm6A,
        PinID::Pin13 => PwmID::Pwm6B,
        PinID::Pin14 => PwmID::Pwm7A,
        PinID::Pin15 => PwmID::Pwm7B,
        PinID::Pin16 => PwmID::Pwm0A,
        PinID::Pin17 => PwmID::Pwm0B,
        PinID::Pin18 => PwmID::Pwm1A,
        PinID::Pin19 => PwmID::Pwm1B,
        PinID::Pin20 => PwmID::Pwm2A,
        PinID::Pin21 => PwmID::Pwm2B,
        PinID::Pin22 => PwmID::Pwm3A,
        PinID::Pin23 => PwmID::Pwm3B,
        PinID::Pin24 => PwmID::Pwm4A,
        PinID::Pin25 => PwmID::Pwm4B,
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
        PinID::Pin8 => I2cID::I2C0,
        PinID::Pin10 => I2cID::I2C1,
        PinID::Pin12 => I2cID::I2C0,
        PinID::Pin14 => I2cID::I2C1,
        PinID::Pin16 => I2cID::I2C0,
        PinID::Pin18 => I2cID::I2C1,
        PinID::Pin20 => I2cID::I2C0,
        PinID::Pin22 => I2cID::I2C1,
        PinID::Pin26 => I2cID::I2C1,
        PinID::Pin28 => I2cID::I2C0,
        _ => return None,
    };
    match (&d, scl) {
        (I2cID::I2C0, PinID::Pin1) => (),
        (I2cID::I2C1, PinID::Pin3) => (),
        (I2cID::I2C0, PinID::Pin5) => (),
        (I2cID::I2C1, PinID::Pin7) => (),
        (I2cID::I2C0, PinID::Pin9) => (),
        (I2cID::I2C1, PinID::Pin11) => (),
        (I2cID::I2C0, PinID::Pin13) => (),
        (I2cID::I2C1, PinID::Pin15) => (),
        (I2cID::I2C0, PinID::Pin17) => (),
        (I2cID::I2C1, PinID::Pin19) => (),
        (I2cID::I2C0, PinID::Pin21) => (),
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
        PinID::Pin11 => SpiID::Spi1,
        PinID::Pin15 => SpiID::Spi1,
        PinID::Pin19 => SpiID::Spi0,
        PinID::Pin27 => SpiID::Spi1,
        _ => return None,
    };
    match (&d, sck) {
        (SpiID::Spi0, PinID::Pin2) => (),
        (SpiID::Spi0, PinID::Pin6) => (),
        (SpiID::Spi1, PinID::Pin10) => (),
        (SpiID::Spi1, PinID::Pin14) => (),
        (SpiID::Spi0, PinID::Pin18) => (),
        (SpiID::Spi0, PinID::Pin22) => (),
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
        (SpiID::Spi1, Some(PinID::Pin8)) => (),
        (SpiID::Spi1, Some(PinID::Pin12)) => (),
        (SpiID::Spi0, Some(PinID::Pin16)) => (),
        (SpiID::Spi0, Some(PinID::Pin20)) => (),
        (SpiID::Spi1, Some(PinID::Pin28)) => (),
        (..) => return None,
    }
    match (&d, cs) {
        (_, None) => (),
        (SpiID::Spi0, Some(PinID::Pin1)) => (),
        (SpiID::Spi0, Some(PinID::Pin5)) => (),
        (SpiID::Spi0, Some(PinID::Pin9)) => (),
        (SpiID::Spi0, Some(PinID::Pin13)) => (),
        (SpiID::Spi0, Some(PinID::Pin17)) => (),
        (SpiID::Spi0, Some(PinID::Pin21)) => (),
        (..) => return None,
    }
    Some(d)
}
#[inline]
pub(crate) fn pins_uart(tx: &PinID, rx: &PinID, cts: Option<&PinID>, rts: Option<&PinID>) -> Option<UartID> {
    let d = match tx {
        PinID::Pin0 => UartID::Uart0,
        PinID::Pin4 => UartID::Uart1,
        PinID::Pin8 => UartID::Uart1,
        PinID::Pin12 => UartID::Uart0,
        PinID::Pin16 => UartID::Uart0,
        PinID::Pin20 => UartID::Uart1,
        PinID::Pin28 => UartID::Uart0,
        _ => return None,
    };
    match (&d, rx) {
        (UartID::Uart0, PinID::Pin1) => (),
        (UartID::Uart1, PinID::Pin5) => (),
        (UartID::Uart1, PinID::Pin9) => (),
        (UartID::Uart0, PinID::Pin13) => (),
        (UartID::Uart0, PinID::Pin17) => (),
        (UartID::Uart1, PinID::Pin21) => (),
        (..) => return None,
    }
    if cts.is_none() && rts.is_none() {
        return Some(d);
    }
    match (&d, cts) {
        (_, None) => (),
        (UartID::Uart0, Some(PinID::Pin2)) => (),
        (UartID::Uart1, Some(PinID::Pin6)) => (),
        (UartID::Uart1, Some(PinID::Pin10)) => (),
        (UartID::Uart0, Some(PinID::Pin14)) => (),
        (UartID::Uart0, Some(PinID::Pin18)) => (),
        (UartID::Uart1, Some(PinID::Pin22)) => (),
        (UartID::Uart1, Some(PinID::Pin26)) => (),
        (..) => return None,
    }
    match (&d, rts) {
        (_, None) => (),
        (UartID::Uart0, Some(PinID::Pin3)) => (),
        (UartID::Uart1, Some(PinID::Pin7)) => (),
        (UartID::Uart1, Some(PinID::Pin11)) => (),
        (UartID::Uart0, Some(PinID::Pin15)) => (),
        (UartID::Uart0, Some(PinID::Pin19)) => (),
        (UartID::Uart1, Some(PinID::Pin27)) => (),
        (..) => return None,
    }
    Some(d)
}
