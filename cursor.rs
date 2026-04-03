use crate::display::{self, Framebuffer};

pub const CURSOR_W: usize = 32;
pub const CURSOR_H: usize = 32;
const CURSOR_OUTLINE: u32 = 0x00000000;
const CURSOR_FILL: u32 = 0x00FFFFFF;
const CURSOR_TRANSPARENT: u32 = 0x00FF00FF;

#[derive(Copy, Clone)]
pub struct CursorRaw {
    pub data: *const u8,
    pub size: usize,
}

static mut CURSOR_BACK: [u32; CURSOR_W * CURSOR_H] = [0; CURSOR_W * CURSOR_H];

pub fn save_background(fb: &Framebuffer, x0: usize, y0: usize) {
    for row in 0..CURSOR_H {
        for col in 0..CURSOR_W {
            let px = x0 + col;
            let py = y0 + row;
            let idx = row * CURSOR_W + col;
            if px < fb.width && py < fb.height {
                unsafe {
                    CURSOR_BACK[idx] = display::get_pixel(fb, px, py);
                }
            }
        }
    }
}

pub fn restore_background(fb: &Framebuffer, x0: usize, y0: usize) {
    for row in 0..CURSOR_H {
        for col in 0..CURSOR_W {
            let px = x0 + col;
            let py = y0 + row;
            let idx = row * CURSOR_W + col;
            if px < fb.width && py < fb.height {
                let rgb = unsafe { CURSOR_BACK[idx] };
                display::put_pixel(fb, px, py, rgb);
            }
        }
    }
}

pub fn draw(fb: &Framebuffer, x0: usize, y0: usize, raw: Option<CursorRaw>) {
    if let Some(raw) = raw {
        if draw_raw(fb, x0, y0, raw.data, raw.size) {
            return;
        }
    }
    draw_builtin(fb, x0, y0);
}

fn draw_builtin(fb: &Framebuffer, x0: usize, y0: usize) {
    for row in 0..CURSOR_H {
        for col in 0..=row {
            if col >= CURSOR_W {
                break;
            }
            let color = if col == 0 || col == row {
                CURSOR_OUTLINE
            } else {
                CURSOR_FILL
            };
            let px = x0 + col;
            let py = y0 + row;
            if px < fb.width && py < fb.height {
                display::put_pixel(fb, px, py, color);
            }
        }
    }
}

fn draw_raw(fb: &Framebuffer, x0: usize, y0: usize, data: *const u8, size: usize) -> bool {
    if size < 8 {
        return false;
    }
    let w = read_u32_le(data) as usize;
    let h = read_u32_le(unsafe { data.add(4) }) as usize;
    if w == 0 || h == 0 || w > 128 || h > 128 {
        return false;
    }
    let pixels = unsafe { data.add(8) };
    let pixel_len = size - 8;
    if pixel_len < w.saturating_mul(h).saturating_mul(4) {
        return false;
    }
    for row in 0..h {
        let py = y0 + row;
        if py >= fb.height {
            break;
        }
        let row_ptr = unsafe { pixels.add(row * w * 4) };
        for col in 0..w {
            let px = x0 + col;
            if px >= fb.width {
                break;
            }
            let p = unsafe { row_ptr.add(col * 4) };
            let b = unsafe { p.read() } as u32;
            let g = unsafe { p.add(1).read() } as u32;
            let r = unsafe { p.add(2).read() } as u32;
            let rgb = (r << 16) | (g << 8) | b;
            if rgb != CURSOR_TRANSPARENT {
                display::put_pixel(fb, px, py, rgb);
            }
        }
    }
    true
}

fn read_u32_le(ptr: *const u8) -> u32 {
    unsafe {
        (ptr.read() as u32)
            | ((ptr.add(1).read() as u32) << 8)
            | ((ptr.add(2).read() as u32) << 16)
            | ((ptr.add(3).read() as u32) << 24)
    }
}
