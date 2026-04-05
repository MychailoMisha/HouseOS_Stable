#![allow(dead_code)]

use core::ptr::write_volatile;

#[derive(Clone, Copy)]
pub struct FrameBuffer {
    pub addr: *mut u8,
    pub pitch: usize,
    pub width: usize,
    pub height: usize,
    pub bpp: u8,
}

impl FrameBuffer {
    pub const fn new(addr: u32, pitch: u16, width: u16, height: u16, bpp: u8) -> Self {
        Self {
            addr: addr as *mut u8,
            pitch: pitch as usize,
            width: width as usize,
            height: height as usize,
            bpp,
        }
    }

    #[inline(always)]
    fn bytes_per_pixel(&self) -> usize {
        match self.bpp {
            24 => 3,
            32 => 4,
            _ => 0,
        }
    }

    pub unsafe fn fill(&self, color: u32) {
        let bpp = self.bytes_per_pixel();
        if bpp == 0 {
            return;
        }
        for y in 0..self.height {
            let row = self.addr.add(y * self.pitch);
            for x in 0..self.width {
                self.put_pixel_raw(row, x, color, bpp);
            }
        }
    }

    pub unsafe fn put_pixel(&self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let bpp = self.bytes_per_pixel();
        if bpp == 0 {
            return;
        }
        let row = self.addr.add(y * self.pitch);
        self.put_pixel_raw(row, x, color, bpp);
    }

    unsafe fn put_pixel_raw(&self, row: *mut u8, x: usize, color: u32, bpp: usize) {
        let p = row.add(x * bpp);
        match bpp {
            3 => {
                write_volatile(p, (color & 0xFF) as u8);
                write_volatile(p.add(1), ((color >> 8) & 0xFF) as u8);
                write_volatile(p.add(2), ((color >> 16) & 0xFF) as u8);
            }
            4 => {
                let p32 = p as *mut u32;
                write_volatile(p32, color);
            }
            _ => {}
        }
    }

    pub unsafe fn draw_glyph8x8(&self, x: usize, y: usize, glyph: &[u8; 8], color: u32) {
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 == 1 {
                    self.put_pixel(x + col, y + row, color);
                }
            }
        }
    }

    pub unsafe fn draw_text(&self, x: usize, y: usize, text: &[u8], font: &[[u8; 8]], color: u32) {
        let mut cx = x;
        for ch in text {
            let idx = *ch as usize;
            if idx < font.len() {
                self.draw_glyph8x8(cx, y, &font[idx], color);
            }
            cx += 8;
        }
    }
}
