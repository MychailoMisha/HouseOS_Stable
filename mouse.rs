use core::arch::asm;

use crate::cursor::{self, CursorRaw};
use crate::display::Framebuffer;

const PS2_STATUS: u16 = 0x64;
const PS2_CMD: u16 = 0x64;
const PS2_DATA: u16 = 0x60;

pub fn run(fb: &Framebuffer, cursor_raw: Option<CursorRaw>) -> ! {
    let mut mouse_x = fb.width.saturating_sub(1) / 2;
    let mut mouse_y = fb.height.saturating_sub(1) / 2;

    cursor::save_background(fb, mouse_x, mouse_y);
    cursor::draw(fb, mouse_x, mouse_y, cursor_raw);

    mouse_init();

    let mut packet = [0u8; 3];
    let mut idx = 0usize;
    loop {
        if let Some(b) = mouse_read_data() {
            if idx == 0 && (b & 0x08) == 0 {
                continue;
            }
            packet[idx] = b;
            idx += 1;
            if idx == 3 {
                idx = 0;
                let dx = packet[1] as i8 as i32;
                let dy = packet[2] as i8 as i32;
                if dx != 0 || dy != 0 {
                    cursor::restore_background(fb, mouse_x, mouse_y);
                    let max_x = fb.width.saturating_sub(cursor::CURSOR_W + 1) as i32;
                    let max_y = fb.height.saturating_sub(cursor::CURSOR_H + 1) as i32;
                    mouse_x = (mouse_x as i32 + dx).clamp(0, max_x) as usize;
                    mouse_y = (mouse_y as i32 - dy).clamp(0, max_y) as usize;
                    cursor::save_background(fb, mouse_x, mouse_y);
                    cursor::draw(fb, mouse_x, mouse_y, cursor_raw);
                }
            }
        }
    }
}

fn mouse_init() {
    unsafe {
        outb(PS2_CMD, 0xA8);
        ps2_wait_input_clear();
        outb(PS2_CMD, 0x20);
    }
    let mut status = ps2_read_data_blocking();
    status |= 0x02;
    unsafe {
        ps2_wait_input_clear();
        outb(PS2_CMD, 0x60);
        ps2_wait_input_clear();
        outb(PS2_DATA, status);
    }
    mouse_write(0xF6);
    mouse_read_ack();
    mouse_write(0xF4);
    mouse_read_ack();
}

fn mouse_read_data() -> Option<u8> {
    let status = unsafe { inb(PS2_STATUS) };
    if (status & 0x01) == 0 {
        return None;
    }
    if (status & 0x20) == 0 {
        return None;
    }
    Some(unsafe { inb(PS2_DATA) })
}

fn mouse_write(cmd: u8) {
    unsafe {
        ps2_wait_input_clear();
        outb(PS2_CMD, 0xD4);
        ps2_wait_input_clear();
        outb(PS2_DATA, cmd);
    }
}

fn mouse_read_ack() {
    let _ = ps2_read_data_blocking();
}

fn ps2_wait_input_clear() {
    let mut tries = 0usize;
    while tries < 100000 {
        let status = unsafe { inb(PS2_STATUS) };
        if (status & 0x02) == 0 {
            return;
        }
        tries += 1;
    }
}

fn ps2_read_data_blocking() -> u8 {
    let mut tries = 0usize;
    while tries < 100000 {
        let status = unsafe { inb(PS2_STATUS) };
        if (status & 0x01) != 0 {
            return unsafe { inb(PS2_DATA) };
        }
        tries += 1;
    }
    0
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let mut val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}
