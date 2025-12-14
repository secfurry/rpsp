# RPSP

RP2040 Platform Support Package

Rust library for easy methods to program and design programs for RP2040 devices.

## Device Selection

To select the proper pin layout, you must specify the device in the "features"
when declaring this crate as a depdency.

Example for the PiPico

```text
rpsp = { version = "0.2.0", features = [ "pico" ] }
```

### Supported Devices

The following device pinouts are supported:

- Pico / PicoW (`pico`)
- Tiny2040 (`tiny2040`)
- Seed Studio 2040 (`xiao2040`)

If your device has no support, you can still use the `pico` device type, but
__BE CAREFUL OF THE PINS USED__.

I'm open to more devices, I just don't have examples to use ^_^

If you'd like to contribute, pinouts are generated from layout text files in the
`boards/data` directory. These are converted into `.rs` files via the `generate.py`
python script in `boards`. This will write board files into the `src/pin/boards`
directory and updates the `src/pin/boards/lib.rs` with all the tags and modules.
The text format is pretty simple, it takes the Pin number and it's capabilities
for each line.

Documentation is [listed here](boards/README.md).

## Note

You'll need to make sure you have the `flip-link` linker installed before compiling.
To do this, use the command `cargo install flip-link` to install it.

### Additional Cargo Configuration

For best results, create a `.cargo/config.toml` file in your project root directory
and specify somthing like this:

```toml
[target.'cfg(all(target_arch = "arm", target_os = "none"))']
rustflags  = [
    "-C", "linker=flip-link",
    "-C", "link-arg=--nmagic",
    "-C", "link-arg=-Tlink.x",
    "-Z", "trap-unreachable=no",
    "-C", "no-vectorize-loops",
]

[build]
target    = "thumbv6m-none-eabi"
```

Requires the `nightly` version of the compiler to use `"-Z", "trap-unreachable=no",`
and can be removed, but will increase binary size slightly.

Extra bonus points if you add:

```toml
runner    = "probe-rs run --chip RP2040"
```

Under the `rustflags` option. This allows you to flash the firmware on the device
directly from `cargo run`. (Pico debug probe probe and `probe-rs` required. Can be
installed using `cargo install probe-rs-tools`. Pico probe can be made from another
Pico! [see here](https://mcuoneclipse.com/2022/09/17/picoprobe-using-the-raspberry-pi-pico-as-debug-probe/)).

Lastly, these are the recommended profile settings for best program results. These
go inside the `Cargo.toml` file in the project root directory.

```toml
[profile.dev]
debug            = 2
strip            = false
opt-level        = 3
incremental      = false
codegen-units    = 1
overflow-checks  = true
debug-assertions = true

[profile.release]
lto              = "fat"
panic            = "abort"
debug            = false
strip            = true
opt-level        = 3
incremental      = false
codegen-units    = 1
overflow-checks  = false
debug-assertions = false
```

## Usage

_NOTE: The struct `Pico` has been renamed to `Board` to properly reflect the_
_multi-device usage. However it still carries the alias type `Pico` and may be_
_used interchangeably._

To use this library, just import `rpsp::Board` and call `Board::get()`. On the first
call, the device and it's clocks will be initialized and setup fully.

The configuration is automatic and uses the ROSC as the system clock, disables
the XOSC and PLLs and allows for DORMANT sleep, for maximum power savings.

To supply main, you must setup the `main` function with the `#[rpsp::entry]` macro,
which will setup the locks and properly redirect execution to the selected function.

Basic programs should look something like this:

```rust
#![no_std]
#![no_main]

#[rpsp::entry]
fn main() -> ! {
    // do stuff here
}
```

If you're not using something like `defmt`, _(which is not included by default)_
you'll need a `panic_handler`. The example below is a pretty basic one that just
loops the CPU:

```rust
#[panic_handler]
fn panic(_p: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
```

For the below examples, the `panic_handler` is omitted, so if you want to use
these, you'll need to add it in order for it to compile.

### GPIO

Control Pin output:

```rust
use rpsp::Board
use rpsp::pin::PinID;

#[rpsp::entry]
fn main() -> ! {
    let p = Board::get();
    let my_pin = p.pin(PinID::Pin5);
    // You could also do..
    // let my_pin = Pin::get(&p, PinID::Pin5);

    // Set High
    my_pin.high();

    // Set Low
    my_pin.low();

    // Need this at the end since it's a '!' function.
    loop {}
}
```

Read Pin output:

```rust
use rpsp::Board
use rpsp::pin::PinID;

#[rpsp::entry]
fn main() -> ! {
    let p = Board::get();
    let my_pin = p.pin(PinID::Pin6).into_input();

    // Set High
    if my_pin.is_high() {
        // Do stuff..
    }

    if my_pin.is_low() {
        // Do other stuff..
    }

    // Need this at the end since it's a '!' function.
    loop {}
}
```

### UART

```rust
use rpsp::Board
use rpsp::pin::PinID;
use rpsp::uart::{Uart, UartConfig, UartDev};

#[rpsp::entry]
fn main() -> ! {
    let p = Board::get();

    // DEFAULT_BAUDRATE is 115,200
    let mut u = Uart::new(
        &p,
        UartConfig::DEFAULT_BAUDRATE,
        UartConfig::new(), // Default is NoParity, 8 Bits, 1 Stop Bit.
        UartDev::new(PinID::Pin0, PinID::Pin1).unwrap(),
        // ^ This can error since not all Pinouts are a valid UART set.
        // You can also use..
        // (PinID::Pin0, PinID::Pin1).into()
    ).unwrap();

    let _ = u.write("HEY THERE\n".as_bytes()).unwrap();
    // Returns the amount of bytes written.

    let mut buf = [0u8; 32];
    let n = u.read(&mut buf).unwrap();
    // Read up to 32 bytes.

    // Echo it back.
    let _ = u.write(&buf[0:n]).unwrap();

    // Cleanup
    u.close();

    // Need this at the end since it's a '!' function.
    loop {}
}
```

### Time  and Sleep

```rust
use rpsp::Board

#[rpsp::entry]
fn main() -> ! {
    let p = Board::get();

    for i in 0..25 {
        p.sleep(5_000); // Wait 5 seconds.

        // Get current RTC time.
        let now = p.rtc().now().unwrap();

        debug!("the time is now {now:?}");
    }

    // Need this at the end since it's a '!' function.
    loop {}
}
```

### Watchdog

```rust
use rpsp::Board

#[rpsp::entry]
fn main() -> ! {
    let p = Board::get();
    let dog = p.watchdog();

    dog.start(5_000); // Die if we don't feed the dog every 5 seconds.

    for _ in 0..10 {
        p.sleep(2_500); // ait 2.5 seconds.

        dog.feed(); // Feed da dog.
    }

    p.sleep(10_000); // Device will restart during here.

    // Need this at the end since it's a '!' function.
    loop {}
}
```

Theres alot of more examples I need to add..

## TODO

- CYW Driver
  - Networking??

License for the CYW driver is [located here](src/cyw/firmware/LICENSE-permissive-binary-license-1.0.txt)
