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

#![no_std]
#![no_main]
#![no_implicit_prelude]
#![feature(never_type, unchecked_shifts)]

extern crate cortex_m;
extern crate cortex_m_rt;
extern crate rp2040_hal_macros;

#[unsafe(link_section = ".boot2")]
#[unsafe(no_mangle)]
#[used]
pub static BOOT2_FIRMWARE: [u8; 256] = *include_bytes!("../bin/rp2040_pico_boot2.bin");

// Save the extra imports
pub use cortex_m::asm;
pub use cortex_m_rt::{ExceptionFrame, exception};
pub use rp2040_hal_macros::entry;

#[cfg_attr(rustfmt, rustfmt_skip)]
#[cfg(feature = "cyw")]
pub mod cyw;

pub mod atomic;
pub mod clock;
pub mod cores;
pub mod dma;
pub mod fifo;
pub mod i2c;
pub mod int;
pub mod interp;
pub mod io;
pub mod locks;
mod pico;
pub mod pin;
pub mod pio;
pub mod rand;
pub mod spi;
pub mod sys;
pub mod time;
pub mod uart;
pub mod watchdog;

pub use pico::*;

#[cfg_attr(rustfmt, rustfmt_skip)]
#[cfg(feature = "debug")]
pub use self::debug::uart_debug;

mod pac {
    // NOTE(sf): It looks cleaner this way instead of 'pub extern'
    extern crate rp2040_pac;

    pub use rp2040_pac::*;
}
#[cfg(feature = "debug")]
mod debug {
    extern crate core;

    use core::cell::UnsafeCell;
    use core::marker::Sync;
    use core::option::Option::{self, None};

    use crate::Board;
    use crate::pin::PinID;
    use crate::uart::{Uart, UartConfig, UartDev};

    static DEBUG: DebugPort = DebugPort(UnsafeCell::new(None));

    struct DebugPort(UnsafeCell<Option<Uart>>);

    impl DebugPort {
        #[inline]
        fn new() -> Uart {
            Uart::new(
                &Board::get(),
                UartConfig::DEFAULT_BAUDRATE,
                UartConfig::new(),
                UartDev::new(PinID::Pin0, PinID::Pin1).unwrap(),
            )
            .unwrap()
        }

        #[inline]
        fn port(&self) -> &mut Uart {
            unsafe { &mut *self.0.get() }.get_or_insert_with(DebugPort::new)
        }
    }

    unsafe impl Sync for DebugPort {}

    #[inline]
    pub fn uart_debug<'a>() -> &'a mut Uart {
        DEBUG.port()
    }

    #[macro_export]
    macro_rules! debug {
        ($dst:expr, $($arg:tt)*) => {{
            let _ = core::writeln!($dst, $($arg)*);
        }};
    }
}
