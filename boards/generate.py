#!/usr/bin/python3

from glob import glob
from io import StringIO
from os import makedirs
from traceback import format_exc
from sys import argv, exit, stderr
from os.path import join, isdir, exists, dirname

_VALID_I2C = [
    "I2C0_SDA",
    "I2C0_SCL",
    "I2C1_SDA",
    "I2C1_SCL",
]
_VALID_SPI = [
    "SPI0_RX",
    "SPI0_CS",
    "SPI0_SCK",
    "SPI0_TX",
    "SPI1_RX",
    "SPI1_CS",
    "SPI1_SCK",
    "SPI1_TX",
]
_VALID_UART = [
    "UART0_TX",
    "UART0_RX",
    "UART0_CTS",
    "UART0_RTS",
    "UART1_TX",
    "UART1_RX",
    "UART1_CTS",
    "UART1_RTS",
]

CODE = """// AUTOMATICALLY GENERATED: DO NOT EDIT!
//
// Use the boards/generate.py script to generate this file.
//

#![no_implicit_prelude]
#![cfg(feature = "{tag}")]

extern crate core;

use core::option::Option::{{self, None, Some}};

use crate::pin::pwm::PwmID;
use crate::pin::{{I2cID, SpiID, UartID}};

/// Pins for "{name}"{pins}
{pwm}{i2c}{spi}{uart}
"""
CODE_LIB = """// AUTOMATICALLY GENERATED: DO NOT EDIT!
//
// Use the boards/generate.py script to update this file.
//

#![no_implicit_prelude]
#![cfg_attr(rustfmt, rustfmt_skip)]
"""
CODE_PWM = """
#[inline]
pub(crate) fn pins_pwm(pin: &PinID) -> PwmID {{
    match pin {{
{pins}}}
}}"""
CODE_I2C = """
#[inline]
pub(crate) fn pins_i2c(sda: &PinID, scl: &PinID) -> Option<I2cID> {{
    let d = match sda {{
{i2c_sda}_ => return None,
    }};
    match (&d, scl) {{
{i2c_scl}(..) => return None,
    }}
    Some(d)
}}"""
CODE_SPI = """
#[inline]
pub(crate) fn pins_spi(tx: &PinID, sck: &PinID, rx: Option<&PinID>, cs: Option<&PinID>) -> Option<SpiID> {{
    let d = match tx {{
{spi_tx}_ => return None,
    }};
    match (&d, sck) {{
{spi_scl}(..) => return None,
    }}
    if rx.is_none() && cs.is_none() {{
        return Some(d);
    }}
    match (&d, rx) {{
        (_, None) => (),
{spi_rx}(..) => return None,
    }}
    match (&d, cs) {{
        (_, None) => (),
{spi_cs}(..) => return None,
    }}
    Some(d)
}}"""
CODE_UART = """
#[inline]
pub(crate) fn pins_uart(tx: &PinID, rx: &PinID, cts: Option<&PinID>, rts: Option<&PinID>) -> Option<UartID> {{
    let d = match tx {{
{uart_tx}_ => return None,
    }};
    match (&d, rx) {{
{uart_rx}(..) => return None,
    }}
    if cts.is_none() && rts.is_none() {{
        return Some(d);
    }}
    match (&d, cts) {{
        (_, None) => (),
{uart_cts}(..) => return None,
    }}
    match (&d, rts) {{
        (_, None) => (),
{uart_rts}(..) => return None,
    }}
    Some(d)
}}"""
CODE_PINS = """
#[repr(u8)]
pub enum PinID {{
{pins}}}"""


class Pin(object):
    __slots__ = ("id", "doc", "roles")

    def __init__(self, id, doc, roles):
        self.id = id
        if len(doc) > 0:
            self.doc = doc.copy()
        else:
            self.doc = None
        self.roles = list()
        if not isinstance(roles, list):
            return
        i, s, u = 0, 0, 0
        for r in roles:
            if len(r) == 0:
                continue
            v = r.upper()
            self.roles.append(v)
            if v in _VALID_I2C:
                i += 1
                continue
            if v in _VALID_SPI:
                s += 1
                continue
            if v in _VALID_UART:
                u += 1
                continue
            raise ValueError(f'pin "{id}" has an invalid role "{v}"')
        if i > 1:
            raise ValueError(f'pin "{id}" can only have a single I2C role')
        if s > 1:
            raise ValueError(f'pin "{id}" can only have a single SPI role')
        if u > 1:
            raise ValueError(f'pin "{id}" can only have a single UART role')
        del i, s, u

    def __str__(self):
        return f"PinID::Pin{self.id}"


def _in(n):
    return "    " * n


def _name(v):
    for x in range(0, len(v)):
        if len(v[x]) == 0:
            continue
        if v[x].startswith("//"):
            continue
        if v[x].startswith("#") or ":" in v[x]:
            break
        return (v[x], x + 1)
    raise ValueError("name entry was not found")


def parse(file):
    with open(file) as f:
        d = [i.strip() for i in f.read().split("\n")]
    if len(d) <= 4:
        raise ValueError(f'file "{file}" content is invalid')
    n, i = _name(d)
    if not _check_ascii(n):
        raise ValueError(f'name value "{n}" is not valid')
    t, i = _tag(d, i)
    if not _check_ascii(t, True):
        raise ValueError(f'tag value "{t}" is not valid')
    p = _pins(d, i)
    if len(p) == 0:
        raise ValueError("no pin entries found")
    del d, i
    p.sort(key=lambda x: x.id)
    return (n, t.lower(), p)


def _tag(v, start):
    if start >= len(v):
        raise ValueError("#<tag> entry was not found")
    for x in range(start, len(v)):
        if len(v[x]) == 0:
            continue
        if v[x].startswith("//"):
            continue
        if v[x].startswith("#") and len(v[x]) >= 4:
            return (v[x][1:], x + 1)
        if ":" in v[x]:
            break
    raise ValueError("#<tag> entry was not found")


def _make_pwm(pins):
    a = StringIO()
    try:
        for i in pins:
            v = int(i.id / 2) & 0x7
            a.write(f"{_in(2)}PinID::Pin{i.id} => PwmID::Pwm{v}")
            del v
            if i.id % 2 == 0:
                a.write("A,\n")
            else:
                a.write("B,\n")
        a.write(_in(1))
        return CODE_PWM.format(pins=a.getvalue())
    finally:
        a.close()
        del a


def _make_i2c(pins):
    a, b = StringIO(), StringIO()
    try:
        for i in pins:
            if "I2C0_SDA" in i.roles:
                a.write(f"{_in(2)}PinID::Pin{i.id} => I2cID::I2C0,\n")
            if "I2C1_SDA" in i.roles:
                a.write(f"{_in(2)}PinID::Pin{i.id} => I2cID::I2C1,\n")
            if "I2C0_SCL" in i.roles:
                b.write(f"{_in(2)}(I2cID::I2C0, PinID::Pin{i.id}) => (),\n")
            if "I2C1_SCL" in i.roles:
                b.write(f"{_in(2)}(I2cID::I2C1, PinID::Pin{i.id}) => (),\n")
        a.write(_in(2))
        b.write(_in(2))
        return CODE_I2C.format(i2c_sda=a.getvalue(), i2c_scl=b.getvalue())
    finally:
        a.close()
        b.close()
        del a, b


def _make_spi(pins):
    a, b, c, d = StringIO(), StringIO(), StringIO(), StringIO()
    try:
        for i in pins:
            if "SPI0_TX" in i.roles:
                a.write(f"{_in(2)}PinID::Pin{i.id} => SpiID::Spi0,\n")
            if "SPI1_TX" in i.roles:
                a.write(f"{_in(2)}PinID::Pin{i.id} => SpiID::Spi1,\n")
            if "SPI0_SCK" in i.roles:
                b.write(f"{_in(2)}(SpiID::Spi0, PinID::Pin{i.id}) => (),\n")
            if "SPI1_SCK" in i.roles:
                b.write(f"{_in(2)}(SpiID::Spi1, PinID::Pin{i.id}) => (),\n")
            if "SPI0_RX" in i.roles:
                c.write(f"{_in(2)}(SpiID::Spi0, Some(PinID::Pin{i.id})) => (),\n")
            if "SPI1_RX" in i.roles:
                c.write(f"{_in(2)}(SpiID::Spi1, Some(PinID::Pin{i.id})) => (),\n")
            if "SPI0_CS" in i.roles:
                d.write(f"{_in(2)}(SpiID::Spi0, Some(PinID::Pin{i.id})) => (),\n")
            if "SPI1_CS" in i.roles:
                d.write(f"{_in(2)}(SpiID::Spi1, Some(PinID::Pin{i.id})) => (),\n")
        a.write(_in(2))
        b.write(_in(2))
        c.write(_in(2))
        d.write(_in(2))
        return CODE_SPI.format(
            spi_tx=a.getvalue(),
            spi_scl=b.getvalue(),
            spi_rx=c.getvalue(),
            spi_cs=d.getvalue(),
        )
    finally:
        a.close()
        b.close()
        c.close()
        d.close()
        del a, b, c, d


def _pins(v, start):
    c, p, g = list(), list(), dict()
    for x in range(start, len(v)):
        if len(v[x]) == 0:
            continue
        if v[x].startswith("//"):
            c.append(_strip_comment(v[x]).strip())
            continue
        i = v[x].find(":")
        if not isinstance(i, int) or i <= 0:
            raise ValueError(f'invalid pin line entry "{v[x]}"')
        try:
            n = int(v[x][0:i], 10)
        except ValueError:
            raise ValueError(f'invalid pin ID "{v[x][0:i]}"')
        if n in g:
            raise ValueError(f'duplicate pin ID "{n}"')
        g[n] = True
        if i + 1 == len(v[x]):
            c.clear()
            continue
        r = v[x][i + 1 :].strip()
        if r == "-":
            e = None
        elif "," in r:
            e = [k.strip() for k in r.split(",")]
        else:
            e = [k.strip() for k in r.split(" ")]
        del r, i
        p.append(Pin(n, c, e))
        c.clear()
        del n, e
    del c, g
    return p


def _make_uart(pins):
    a, b, c, d = StringIO(), StringIO(), StringIO(), StringIO()
    try:
        for i in pins:
            if "UART0_TX" in i.roles:
                a.write(f"{_in(2)}PinID::Pin{i.id} => UartID::Uart0,\n")
            if "UART1_TX" in i.roles:
                a.write(f"{_in(2)}PinID::Pin{i.id} => UartID::Uart1,\n")
            if "UART0_RX" in i.roles:
                b.write(f"{_in(2)}(UartID::Uart0, PinID::Pin{i.id}) => (),\n")
            if "UART1_RX" in i.roles:
                b.write(f"{_in(2)}(UartID::Uart1, PinID::Pin{i.id}) => (),\n")
            if "UART0_CTS" in i.roles:
                c.write(f"{_in(2)}(UartID::Uart0, Some(PinID::Pin{i.id})) => (),\n")
            if "UART1_CTS" in i.roles:
                c.write(f"{_in(2)}(UartID::Uart1, Some(PinID::Pin{i.id})) => (),\n")
            if "UART0_RTS" in i.roles:
                d.write(f"{_in(2)}(UartID::Uart0, Some(PinID::Pin{i.id})) => (),\n")
            if "UART1_RTS" in i.roles:
                d.write(f"{_in(2)}(UartID::Uart1, Some(PinID::Pin{i.id})) => (),\n")
        a.write(_in(2))
        b.write(_in(2))
        c.write(_in(2))
        d.write(_in(2))
        return CODE_UART.format(
            uart_tx=a.getvalue(),
            uart_rx=b.getvalue(),
            uart_cts=c.getvalue(),
            uart_rts=d.getvalue(),
        )
    finally:
        a.close()
        b.close()
        c.close()
        d.close()
        del a, b, c, d


def _make_pins(pins):
    a = StringIO()
    try:
        for i in pins:
            if isinstance(i.doc, list) and len(i.doc) > 0:
                for v in i.doc:
                    a.write(f"{_in(1)}/// {v}\n")
            a.write(f"{_in(1)}Pin{i.id} = 0x{hex(i.id)[2:].upper()}u8,\n")
        return CODE_PINS.format(pins=a.getvalue())
    finally:
        a.close()
        del a


def _strip_comment(v):
    for x in range(0, len(v)):
        i = ord(v[x])
        if i == 0x2F or i == 0x20:
            continue
        del i
        return v[x:]
    return v


def _check_ascii(v, strict=False):
    # Strict  : A-Za-z0-9_-
    #  Cannot start with a number or '_' or '-'
    # Regular : A-Za-z0-9_- [](){}@|
    if len(v) <= 1:
        return False
    # Strict entries cannot start with a number.
    if strict:
        n = ord(v[0])
        if 0x30 <= n <= 0x39 or n == 0x5F or n == 0x2D:
            return False
        del n
    for i in v:
        n = ord(i)
        # a-z
        if 0x61 <= n <= 0x7A:
            continue
        # A-Z
        if 0x41 <= n <= 0x5A:
            continue
        # 0-9
        if 0x30 <= n <= 0x39:
            continue
        # _ -
        if n == 0x5F or n == 0x2D:
            continue
        if strict:
            return False
        # [sp] [ ] |
        if n == 0x20 or n == 0x5B or n == 0x5D or n == 0x7C:
            continue
        # { } ( ) @
        if n == 0x7B or n == 0x7D or n == 0x28 or n == 0x29 or n == 0x40:
            continue
        return False
    return True


def format_device(name, tag, pins):
    return CODE.format(
        tag=tag,
        name=name,
        pwm=_make_pwm(pins),
        i2c=_make_i2c(pins),
        spi=_make_spi(pins),
        pins=_make_pins(pins),
        uart=_make_uart(pins),
    )


def make_code_files(layout_dir, boards_dir):
    if not exists(layout_dir):
        makedirs(layout_dir, mode=0o0755, exist_ok=True)
    if not exists(boards_dir):
        makedirs(boards_dir, mode=0o0755, exist_ok=True)
    if not isdir(layout_dir):
        raise ValueError(f'layout directory "{layout_dir}" is not a directory')
    if not isdir(boards_dir):
        raise ValueError(f'boards directory "{boards_dir}" is not a directory')
    d = glob(join(layout_dir, "*.layout"))
    if len(d) == 0:
        raise ValueError(f'no layouts found in "{layout_dir}"')
    b, c, u = StringIO(), StringIO(), dict()
    try:
        b.write(CODE_LIB)
        for i in d:
            try:
                print(f'Processing "{i}"..')
                n, t, p = parse(i)
                if t == "lib":
                    raise ValueError(f'invalid tag name "{t}" in "{i}"')
                if t in u:
                    raise ValueError(f'duplicate tag name "{t}" in "{i}"')
                u[t] = True
                with open(join(boards_dir, f"{t}.rs"), "w") as f:
                    f.write(format_device(n, t, p))
                del p
                b.write(
                    f'\n#[cfg(feature = "{t}")]\npub mod {t};\n'
                    f'#[cfg(feature = "{t}")]\npub use {t} as pins;\n'
                )
                print(f'Processed "{n}" [{t}] from "{i}".')
                c.write(f"{t} = []\n")
                del n, t
            except (ValueError, OSError) as err:
                raise ValueError(f'error in "{i}": {err}')
        p = join(boards_dir, "lib.rs")
        print(f'Writing final "{p}"..')
        with open(p, "w") as f:
            f.write(b.getvalue())
        del p
        print('Done! Make sure the following entries are in the "Cargo.toml" file:')
        print(f"\n{c.getvalue()}")
    finally:
        b.close()
        c.close()
        del b, c, u


if __name__ == "__main__":
    try:
        d = dirname(__file__)
        if len(argv) == 1:  # No arguments
            s, v = join(d, "data"), join(dirname(d), "src", "pin", "boards")
        elif len(argv) == 2:  # Just the layouts path
            s, v = argv[1], join(dirname(d), "src", "pin", "boards")
        elif len(argv) == 3:  # Layouts and boards path
            s, v = argv[1], argv[2]
        else:
            print(f"{argv[0]} [layouts_dir] [board_code_dir]", file=stderr)
            exit(2)
        print(f'Layouts path "{s}"\nBoard Code path: "{v}"\n')
        make_code_files(s, v)
    except (ValueError, OSError) as err:
        print(f"Error: {err}", file=stderr)
        print(format_exc(limit=6), file=stderr)
        exit(1)
