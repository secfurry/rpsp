# RPSP Board Layout Files

Generation of the `*.rs` files that contain the Pin definitions and features is
done via `generate.py`. This python script will read the board layout files in
the `data` folder and will create and update the `src/pin/boards` directory and
all files under it.

## Layout Files

Layout files are stored under the `data` folder and **MUST** have the extension
`.layout` to be processed by the generation script.

### Layout File Basics

The format is made to be extremly simple to use, read and understand.

Whitespace does not matter, and spaces can be as large as needed for padding or
for layout purposes.

"    text " and "text" are read as the same value.

Lines starting with `//` or `#` are treated as comments and are omitted, **except**
**when in Pin definitions (explained below)**. `#` has a special use case only
in the **tag** section of the Layout Header. Comments with witespace before them
are treated the same, as the whitespace is trimmed before processing. **Inline**
**comments are not supported.**

Empty lines are skipped over and can be present in any part of the Layout file.

### Layout File Header

The header is made up of two lines. The first line of the header is the device
descriptive name, such as `Raspberry Pi Pico[W]` for the `pico.layout` file.

This is detected as the **first, non-empty, non-comment** line, which can **only**
contain the characters `A-Za-z0-9_- [](){}@|`.

The second header value, is the **tag** value. This is the value used as the Rust
**feature name** to select the device. This is detected as the **first, non-empty,**
**line starting with `#`** and can **only** contain the characters `A-Za-z0-9_-`.

### Layout Pin Definitions

After the header is the Pin definitions section. This section is used to define
which pins exist and any functions they support.

To signal the start of the Pin Definitions section, the next **non-empty line**
**starting with `[number]:`** will be used to signal the first definition line.

The `[number]:` entry, takes the form or any number `[0-9+]` followed by `:`. To
signal no supported functions, use a `-` after the `:`. Otherwise, to add supported
pin functions, the following values can be listed in the same line, **after** the
`:`. These may be comma `,` or space seperated:

- **I2C0_SDA**: This pin can be used as the I2C data pin for I2C bus 0.
- **I2C0_SCL**: This pin can be used as the I2C clock pin for I2C bus 0.
- **I2C1_SDA**: This pin can be used as the I2C data pin for I2C bus 1.
- **I2C1_SCL**: This pin can be used as the I2C clock pin for I2C bus 1.
- **SPI0_RX**: This pin can be used as the SPI receive pin for SPI bus 0.
- **SPI0_CS**: This pin can be used as the SPI clip select pin for SPI bus 0.
- **SPI0_TX**: This pin can be used as the SPI transmit pin for SPI bus 0.
- **SPI0_SCK**: This pin can be used as the SPI clock pin for SPI bus 0.
- **SPI1_RX**: This pin can be used as the SPI receive pin for SPI bus 1.
- **SPI1_CS**: This pin can be used as the SPI clip select pin for SPI bus 1.
- **SPI1_TX**: This pin can be used as the SPI transmit pin for SPI bus 1.
- **SPI1_SCK**: This pin can be used as the SPI clock pin for SPI bus 1.
- **UART0_TX**: This pin can be used as the UART transmit pin for UART bus 0.
- **UART0_RX**: This pin can be used as the UART receive pin for UART bus 0.
- **UART0_CTS**: This pin can be used as the UART clear to send pin for UART bus 0.
- **UART0_RTS**: This pin can be used as the UART clear to receive pin for UART bus 0.
- **UART1_TX**: This pin can be used as the UART transmit pin for UART bus 1.
- **UART1_RX**: This pin can be used as the UART receive pin for UART bus 1.
- **UART1_CTS**: This pin can be used as the UART clear to send pin for UART bus 1.
- **UART1_RTS**: This pin can be used as the UART clear to receive pin for UART bus 1.

Lines that contain conflicting values or multiple of the same role are invalid.
An example of a conflicting value would is `UART0_RX, UART0_TX` as the pin cannot
be both the transmit and receive pin for UART bus 0.

The order of the pins does not matter, as they will be sorted by the pin number
once read by the parser.

### Layout Pin Comment Lines

Each comment between pin lines will be parsed an output as Rust comments in the
resulting output `*.rs` file. These can be used to specify or indicate special
details to users. Each line will be added, seperated by a newline. Empty lines
are ignored, by a comment with a space "// " or "# " can be used as an empty
newline as it will be parsed.

### Example Layout File

This is an example Layout file format for the Tiny 2040 chip. This is a modified
snippet from the `tiny2040.layout` file.

```text
// This is a comment line that is not parsed

# Likewise

Tiny 2040
// ^ Device Name/Description
#tiny2040
// ^ Device Tag

 0: I2C0_SDA, SPI0_RX,  UART0_TX
 1: I2C0_SCL, SPI0_CS,  UART0_RX
 2: I2C1_SDA, SPI0_SCK, UART0_CTS
 3: I2C1_SCL, SPI0_TX,  UART0_RTS
 4: I2C0_SDA, SPI0_RX,  UART1_TX
 5: I2C0_SCL, SPI0_CS,  UART1_RX
 6: I2C1_SDA, SPI0_SCK, UART1_CTS
 7: I2C1_SCL, SPI0_TX,  UART1_RTS
// ADC Pin0 <- This will be added as a comment to the resulting .rs file.
26: I2C1_SDA, SPI1_SCK, UART1_CTS
// ADC Pin1
27: I2C1_SCL, SPI1_TX,  UART1_RTS
// ADC Pin2
28: I2C0_SDA, SPI1_RX,  UART0_TX
// ADC Pin3
29: -
//  ^ No supported features.
```

## Output

Once parsed, the list will be written to `src/pin/boards/lib.rs` and each Layout
file will be written into `src/pin/boards/[tag].rs`.

The `Cargo.toml` file will need to be manually updated, but the `generate.py` script
will output the data to be copy-paste'd over.
