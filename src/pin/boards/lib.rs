// AUTOMATICALLY GENERATED: DO NOT EDIT!
//
// Use the boards/generate.py script to update this file.
//

#![no_implicit_prelude]
#![cfg_attr(rustfmt, rustfmt_skip)]

#[cfg(feature = "pico")]
pub mod pico;
#[cfg(feature = "pico")]
pub use pico as pins;

#[cfg(feature = "tiny2040")]
pub mod tiny2040;
#[cfg(feature = "tiny2040")]
pub use tiny2040 as pins;

#[cfg(feature = "xiao2040")]
pub mod xiao2040;
#[cfg(feature = "xiao2040")]
pub use xiao2040 as pins;
