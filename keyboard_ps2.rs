#![allow(dead_code)]

use crate::drivers::input::ps2_controller;
use crate::drivers::port_io::inb;

const PS2_STATUS: u16 = 0x64;

pub unsafe fn init() {
    // Enable IRQ1 and make sure keyboard port is enabled.
    ps2_controller::write_cmd(0x20);
    let mut status = ps2_controller::read_data();
    status |= 0x01; // IRQ1 enable
    status &= !0x10; // clear keyboard disable
    ps2_controller::write_cmd(0x60);
    ps2_controller::write_data(status);
    ps2_controller::write_cmd(0xAE);
}

pub unsafe fn read_byte() -> Option<u8> {
    let status = inb(PS2_STATUS);
    if (status & 0x01) == 0 {
        return None;
    }
    if (status & 0x20) != 0 {
        return None;
    }
    Some(ps2_controller::read_data())
}

pub unsafe fn read_scancode() -> u8 {
    ps2_controller::read_data()
}

pub fn scancode_to_ascii(sc: u8, shift: bool) -> Option<u8> {
    let c = match sc {
        0x02 => if shift { b'!' } else { b'1' },
        0x03 => if shift { b'@' } else { b'2' },
        0x04 => if shift { b'#' } else { b'3' },
        0x05 => if shift { b'$' } else { b'4' },
        0x06 => if shift { b'%' } else { b'5' },
        0x07 => if shift { b'^' } else { b'6' },
        0x08 => if shift { b'&' } else { b'7' },
        0x09 => if shift { b'*' } else { b'8' },
        0x0A => if shift { b'(' } else { b'9' },
        0x0B => if shift { b')' } else { b'0' },
        0x0C => if shift { b'_' } else { b'-' },
        0x0D => if shift { b'+' } else { b'=' },
        0x10 => if shift { b'Q' } else { b'q' },
        0x11 => if shift { b'W' } else { b'w' },
        0x12 => if shift { b'E' } else { b'e' },
        0x13 => if shift { b'R' } else { b'r' },
        0x14 => if shift { b'T' } else { b't' },
        0x15 => if shift { b'Y' } else { b'y' },
        0x16 => if shift { b'U' } else { b'u' },
        0x17 => if shift { b'I' } else { b'i' },
        0x18 => if shift { b'O' } else { b'o' },
        0x19 => if shift { b'P' } else { b'p' },
        0x1A => if shift { b'{' } else { b'[' },
        0x1B => if shift { b'}' } else { b']' },
        0x1E => if shift { b'A' } else { b'a' },
        0x1F => if shift { b'S' } else { b's' },
        0x20 => if shift { b'D' } else { b'd' },
        0x21 => if shift { b'F' } else { b'f' },
        0x22 => if shift { b'G' } else { b'g' },
        0x23 => if shift { b'H' } else { b'h' },
        0x24 => if shift { b'J' } else { b'j' },
        0x25 => if shift { b'K' } else { b'k' },
        0x26 => if shift { b'L' } else { b'l' },
        0x27 => if shift { b':' } else { b';' },
        0x28 => if shift { b'"' } else { b'\'' },
        0x29 => if shift { b'~' } else { b'`' },
        0x2B => if shift { b'|' } else { b'\\' },
        0x2C => if shift { b'Z' } else { b'z' },
        0x2D => if shift { b'X' } else { b'x' },
        0x2E => if shift { b'C' } else { b'c' },
        0x2F => if shift { b'V' } else { b'v' },
        0x30 => if shift { b'B' } else { b'b' },
        0x31 => if shift { b'N' } else { b'n' },
        0x32 => if shift { b'M' } else { b'm' },
        0x33 => if shift { b'<' } else { b',' },
        0x34 => if shift { b'>' } else { b'.' },
        0x35 => if shift { b'?' } else { b'/' },
        0x39 => b' ',
        0x1C => b'\n',
        0x0F => b'\t',
        0x0E => 0x08, // backspace
        _ => return None,
    };
    Some(c)
}
