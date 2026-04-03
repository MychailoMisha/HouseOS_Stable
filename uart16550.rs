#![allow(dead_code)]

use crate::drivers::port_io::{inb, outb};

const COM1: u16 = 0x3F8;

pub unsafe fn init() {
    outb(COM1 + 1, 0x00); // disable interrupts
    outb(COM1 + 3, 0x80); // enable DLAB
    outb(COM1 + 0, 0x03); // divisor low  (38400 baud)
    outb(COM1 + 1, 0x00); // divisor high
    outb(COM1 + 3, 0x03); // 8 bits, no parity, one stop
    outb(COM1 + 2, 0xC7); // enable FIFO
    outb(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set
}

fn tx_ready() -> bool {
    unsafe { inb(COM1 + 5) & 0x20 != 0 }
}

pub unsafe fn write_byte(b: u8) {
    while !tx_ready() {}
    outb(COM1, b);
}

pub unsafe fn write_str(s: &str) {
    for b in s.bytes() {
        write_byte(b);
    }
}
