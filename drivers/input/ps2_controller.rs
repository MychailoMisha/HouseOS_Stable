#![allow(dead_code)]

use crate::drivers::port_io::{inb, outb, io_wait};

const PS2_DATA: u16 = 0x60;
const PS2_STATUS: u16 = 0x64;
const PS2_CMD: u16 = 0x64;

fn status() -> u8 {
    unsafe { inb(PS2_STATUS) }
}

fn can_read() -> bool {
    status() & 0x01 != 0
}

fn can_write() -> bool {
    status() & 0x02 == 0
}

fn wait_read() {
    for _ in 0..100000 {
        if can_read() {
            return;
        }
    }
}

fn wait_write() {
    for _ in 0..100000 {
        if can_write() {
            return;
        }
    }
}

pub unsafe fn read_data() -> u8 {
    wait_read();
    inb(PS2_DATA)
}

pub unsafe fn write_data(val: u8) {
    wait_write();
    outb(PS2_DATA, val);
}

pub unsafe fn write_cmd(val: u8) {
    wait_write();
    outb(PS2_CMD, val);
}

pub unsafe fn disable_ports() {
    write_cmd(0xAD);
    write_cmd(0xA7);
    io_wait();
}

pub unsafe fn enable_ports() {
    write_cmd(0xAE);
    write_cmd(0xA8);
    io_wait();
}

pub unsafe fn self_test() -> bool {
    write_cmd(0xAA);
    read_data() == 0x55
}
