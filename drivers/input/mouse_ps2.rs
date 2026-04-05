#![allow(dead_code)]

use crate::drivers::input::ps2_controller;
use crate::drivers::port_io::inb;

const PS2_STATUS: u16 = 0x64;

#[derive(Clone, Copy, Debug)]
pub struct MousePacket {
    pub buttons: u8,
    pub dx: i8,
    pub dy: i8,
}

pub unsafe fn init() {
    // Disable ports and flush stale data.
    ps2_controller::disable_ports();
    flush_output();

    // Enable IRQ12 and ensure the aux port isn't disabled.
    ps2_controller::write_cmd(0x20);
    let mut status = ps2_controller::read_data();
    status |= 0x03; // IRQ1 + IRQ12 enable
    status &= !0x30; // clear keyboard + mouse disable
    ps2_controller::write_cmd(0x60);
    ps2_controller::write_data(status);

    // Enable auxiliary device.
    ps2_controller::enable_ports();

    // Reset mouse and enable streaming.
    write_mouse(0xFF);
    let _ = read_mouse_ack();
    let _ = read_mouse_response(); // self-test (0xAA)
    let _ = read_mouse_response(); // device id

    write_mouse(0xF6); // defaults
    let _ = read_mouse_ack();
    write_mouse(0xF4); // data reporting
    let _ = read_mouse_ack();
}

pub unsafe fn read_packet() -> MousePacket {
    let b0 = ps2_controller::read_data();
    let b1 = ps2_controller::read_data();
    let b2 = ps2_controller::read_data();
    MousePacket {
        buttons: b0 & 0x07,
        dx: b1 as i8,
        dy: b2 as i8,
    }
}

pub unsafe fn read_byte() -> Option<u8> {
    let status = inb(PS2_STATUS);
    if (status & 0x01) == 0 {
        return None;
    }
    if (status & 0x20) == 0 {
        return None;
    }
    Some(ps2_controller::read_data())
}

unsafe fn write_mouse(val: u8) {
    ps2_controller::write_cmd(0xD4);
    ps2_controller::write_data(val);
}

fn flush_output() {
    for _ in 0..10000 {
        let status = unsafe { inb(PS2_STATUS) };
        if (status & 0x01) == 0 {
            break;
        }
        let _ = unsafe { ps2_controller::read_data() };
    }
}

fn read_mouse_response() -> Option<u8> {
    for _ in 0..10000 {
        if let Some(b) = unsafe { read_byte() } {
            return Some(b);
        }
    }
    None
}

fn read_mouse_ack() -> bool {
    for _ in 0..10000 {
        if let Some(b) = unsafe { read_byte() } {
            return b == 0xFA;
        }
    }
    false
}
