use crate::display::{self, Framebuffer};
use crate::system;

pub const HEADER_H: usize = 28;
pub const CLOSE_SIZE: usize = 14;
pub const SHADOW: usize = 6;

#[derive(Copy, Clone)]
pub struct ChromeLayout {
    pub content_x: usize,
    pub content_y: usize,
    pub content_w: usize,
    pub content_h: usize,
    pub close: (usize, usize, usize, usize),
    pub header: (usize, usize, usize, usize),
}

pub fn draw_window(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize, title: &[u8]) -> ChromeLayout {
    let ui = system::ui_settings();
    let accent = ui.accent;
    let (shadow, border, bg, header_bg, header_text) = if ui.dark {
        (0x00090909, 0x00363636, 0x00212121, 0x002A2A2A, 0x00FFFFFF)
    } else {
        (0x00202020, 0x00D0D0D0, 0x00FFFFFF, 0x00F7F7F7, 0x00000000)
    };

    display::fill_rect(fb, x + SHADOW, y + SHADOW, w, h, shadow);
    display::fill_rect(fb, x, y, w, h, bg);
    display::fill_rect(fb, x, y, w, 1, border);
    display::fill_rect(fb, x, y + h.saturating_sub(1), w, 1, border);
    display::fill_rect(fb, x, y, 1, h, border);
    display::fill_rect(fb, x + w.saturating_sub(1), y, 1, h, border);

    display::fill_rect(fb, x + 1, y + 1, w.saturating_sub(2), HEADER_H, header_bg);
    display::fill_rect(
        fb,
        x + 1,
        y + HEADER_H.saturating_sub(2),
        w.saturating_sub(2),
        2,
        accent,
    );
    let close = close_rect(x, y, w);
    display::fill_rect(fb, close.0, close.1, close.2, close.3, 0x00E81123);

    let mut writer = crate::TextWriter::new(*fb);
    writer.set_color(header_text);
    writer.set_pos(x + 10, y + 7);
    writer.write_bytes(title);
    writer.set_pos(close.0 + 4, close.1 + 3);
    writer.write_bytes(b"X");

    let header = (x, y, w, HEADER_H);
    let content_x = x + 1;
    let content_y = y + HEADER_H + 1;
    let content_w = w.saturating_sub(2);
    let content_h = h.saturating_sub(HEADER_H + 2);

    ChromeLayout {
        content_x,
        content_y,
        content_w,
        content_h,
        close,
        header,
    }
}

pub fn close_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    let cx = x + w.saturating_sub(CLOSE_SIZE + 6);
    let cy = y + (HEADER_H.saturating_sub(CLOSE_SIZE)) / 2 + 1;
    (cx, cy, CLOSE_SIZE, CLOSE_SIZE)
}

pub fn header_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    (x, y, w, HEADER_H)
}

pub fn hit(px: usize, py: usize, rect: (usize, usize, usize, usize)) -> bool {
    px >= rect.0 && py >= rect.1 && px < rect.0 + rect.2 && py < rect.1 + rect.3
}
