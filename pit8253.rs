#![allow(dead_code)]

use crate::drivers::port_io::outb;

const PIT_CH0: u16 = 0x40;
const PIT_CMD: u16 = 0x43;
const PIT_BASE: u32 = 1193182;

pub unsafe fn init(hz: u32) {
    let divisor = if hz == 0 { 0 } else { PIT_BASE / hz };
    outb(PIT_CMD, 0x36); // channel 0, lo/hi, mode 3
    outb(PIT_CH0, (divisor & 0xFF) as u8);
    outb(PIT_CH0, ((divisor >> 8) & 0xFF) as u8);
}
