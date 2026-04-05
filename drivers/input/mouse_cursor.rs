#![allow(dead_code)]

use crate::cursor::{self, CursorRaw};
use crate::display::Framebuffer;
use crate::drivers::input::mouse_ps2;

pub fn run(fb: &Framebuffer, cursor_raw: Option<CursorRaw>) -> ! {
    let mut mouse_x = fb.width.saturating_sub(1) / 2;
    let mut mouse_y = fb.height.saturating_sub(1) / 2;

    cursor::save_background(fb, mouse_x, mouse_y);
    cursor::draw(fb, mouse_x, mouse_y, cursor_raw);

    unsafe { mouse_ps2::init(); }

    let mut packet = [0u8; 3];
    let mut idx = 0usize;
    loop {
        if let Some(b) = unsafe { mouse_ps2::read_byte() } {
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
