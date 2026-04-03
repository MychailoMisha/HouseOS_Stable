use crate::display::{self, Framebuffer};

pub fn draw_panel(fb: &Framebuffer, height: usize) {
    if height == 0 {
        return;
    }
    let y = fb.height.saturating_sub(height);
    display::fill_rect(fb, 0, y, fb.width, height, 0x00FFFFFF);
    if y > 0 {
        display::fill_rect(fb, 0, y - 1, fb.width, 1, 0x00DDDDDD);
    }
}

pub fn draw_start_button(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize, pressed: bool) {
    if w == 0 || h == 0 {
        return;
    }
    let bg = if pressed { 0x00E6E6E6 } else { 0x00F5F5F5 };
    display::fill_rect(fb, x, y, w, h, bg);
    display::fill_rect(fb, x, y, w, 1, 0x00BBBBBB);
    display::fill_rect(fb, x, y + h.saturating_sub(1), w, 1, 0x00BBBBBB);
    display::fill_rect(fb, x, y, 1, h, 0x00BBBBBB);
    display::fill_rect(fb, x + w.saturating_sub(1), y, 1, h, 0x00BBBBBB);

    let logo_size = (h.min(w) / 2).max(8);
    let tile = logo_size / 2;
    let gap = 2usize;
    let logo_w = tile * 2 + gap;
    let logo_h = tile * 2 + gap;
    let lx = x + (w.saturating_sub(logo_w)) / 2;
    let ly = y + (h.saturating_sub(logo_h)) / 2;
    let color = 0x00007ACC;
    display::fill_rect(fb, lx, ly, tile, tile, color);
    display::fill_rect(fb, lx + tile + gap, ly, tile, tile, color);
    display::fill_rect(fb, lx, ly + tile + gap, tile, tile, color);
    display::fill_rect(fb, lx + tile + gap, ly + tile + gap, tile, tile, color);
}

pub fn draw_start_menu(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize) {
    if w == 0 || h == 0 {
        return;
    }
    display::fill_rect(fb, x + 4, y + 4, w, h, 0x00202020);
    display::fill_rect(fb, x, y, w, h, 0x00FFFFFF);
    display::fill_rect(fb, x, y, w, 1, 0x00CCCCCC);
    display::fill_rect(fb, x, y + h.saturating_sub(1), w, 1, 0x00CCCCCC);
    display::fill_rect(fb, x, y, 1, h, 0x00CCCCCC);
    display::fill_rect(fb, x + w.saturating_sub(1), y, 1, h, 0x00CCCCCC);

    let accent_w = w / 4;
    display::fill_rect(fb, x, y, accent_w, h, 0x00F3F6FA);

    let item_h = 36usize;
    let mut iy = y + 12;
    for _ in 0..5 {
        display::fill_rect(fb, x + accent_w + 12, iy, w - accent_w - 24, item_h, 0x00F8F8F8);
        iy = iy.saturating_add(item_h + 10);
        if iy + item_h >= y + h {
            break;
        }
    }
}
