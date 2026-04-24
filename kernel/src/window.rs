use crate::display::{self, Framebuffer};
use crate::system;

pub const HEADER_H: usize = 42;
pub const CORNER_RADIUS: usize = 14;

const BTN_SIZE: usize = 22;
const BTN_GAP: usize = 6;
const BTN_RIGHT_PAD: usize = 10;

#[derive(Copy, Clone)]
pub struct ChromeLayout {
    pub content_x: usize,
    pub content_y: usize,
    pub content_w: usize,
    pub content_h: usize,
    pub close: (usize, usize, usize, usize),
    pub maximize: (usize, usize, usize, usize),
    pub minimize: (usize, usize, usize, usize),
    pub header: (usize, usize, usize, usize),
    pub drag_header: (usize, usize, usize, usize),
}

pub fn draw_window(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    title: &[u8],
) -> ChromeLayout {
    let ui = system::ui_settings();
    let accent = ui.accent;
    let is_dark = ui.dark;

    let bg_main = if is_dark { 0x001A1A1A } else { 0x00FDFDFD };
    let header_bg = if is_dark { 0x00222222 } else { 0x00F3F4F6 };
    let text_primary = if is_dark { 0x00FFFFFF } else { 0x001A1A1A };
    let shadow_soft = if is_dark { 0x00050505 } else { 0x00202020 };
    let outer_border = if is_dark { 0x00333333 } else { 0x00D1D5DB };
    let inner_border = if is_dark { 0x00444444 } else { 0x00FFFFFF };

    draw_rounded_rect(fb, x + 5, y + 5, w, h, CORNER_RADIUS, shadow_soft);
    draw_rounded_rect(fb, x + 2, y + 2, w, h, CORNER_RADIUS, shadow_soft);
    draw_rounded_rect(fb, x, y, w, h, CORNER_RADIUS, bg_main);
    draw_rounded_rect_top(fb, x, y, w, HEADER_H, CORNER_RADIUS, header_bg);
    display::fill_rect(fb, x + 1, y + HEADER_H - 1, w.saturating_sub(2), 1, outer_border);
    display::fill_rect(fb, x + 8, y + 14, 3, 14, accent);
    draw_rounded_rect_outline(fb, x, y, w, h, CORNER_RADIUS, 1, outer_border);
    draw_rounded_rect_outline(
        fb,
        x + 1,
        y + 1,
        w.saturating_sub(2),
        h.saturating_sub(2),
        CORNER_RADIUS.saturating_sub(1),
        1,
        inner_border,
    );

    let close = close_rect(x, y, w);
    let maximize = maximize_rect(x, y, w);
    let minimize = minimize_rect(x, y, w);
    let btn_bg = if is_dark { 0x00333333 } else { 0x00E5E7EB };

    draw_rounded_rect(fb, minimize.0, minimize.1, minimize.2, minimize.3, 8, btn_bg);
    draw_rounded_rect(fb, maximize.0, maximize.1, maximize.2, maximize.3, 8, btn_bg);
    draw_rounded_rect(fb, close.0, close.1, close.2, close.3, 8, btn_bg);

    let mut writer = crate::TextWriter::new(*fb);
    writer.set_color(text_primary);
    writer.set_pos(x + 22, y + 13);
    writer.write_bytes(title);

    let icon_color = if is_dark { 0x00B5BBC5 } else { 0x004B5563 };
    writer.set_color(icon_color);
    writer.set_pos(minimize.0 + 7, minimize.1 + 6);
    writer.write_bytes(b"-");
    writer.set_pos(maximize.0 + 7, maximize.1 + 6);
    writer.write_bytes(b"o");
    writer.set_pos(close.0 + 8, close.1 + 6);
    writer.write_bytes(b"x");

    ChromeLayout {
        content_x: x + 2,
        content_y: y + HEADER_H + 2,
        content_w: w.saturating_sub(4),
        content_h: h.saturating_sub(HEADER_H + 4),
        close,
        maximize,
        minimize,
        header: (x, y, w, HEADER_H),
        drag_header: drag_header_rect(x, y, w),
    }
}

pub fn close_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    let cx = x + w.saturating_sub(BTN_RIGHT_PAD + BTN_SIZE);
    let cy = y + (HEADER_H.saturating_sub(BTN_SIZE)) / 2;
    (cx, cy, BTN_SIZE, BTN_SIZE)
}

pub fn maximize_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    let close = close_rect(x, y, w);
    let cx = close.0.saturating_sub(BTN_SIZE + BTN_GAP);
    (cx, close.1, BTN_SIZE, BTN_SIZE)
}

pub fn minimize_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    let max = maximize_rect(x, y, w);
    let cx = max.0.saturating_sub(BTN_SIZE + BTN_GAP);
    (cx, max.1, BTN_SIZE, BTN_SIZE)
}

pub fn header_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    (x, y, w, HEADER_H)
}

pub fn drag_header_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    let min_btn = minimize_rect(x, y, w);
    let start_x = x + 12;
    let right = min_btn.0.saturating_sub(6);
    let drag_w = right.saturating_sub(start_x);
    (start_x, y, drag_w, HEADER_H)
}

pub fn hit(px: usize, py: usize, rect: (usize, usize, usize, usize)) -> bool {
    px >= rect.0 && py >= rect.1 && px < rect.0 + rect.2 && py < rect.1 + rect.3
}

fn draw_rounded_rect(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    r: usize,
    color: u32,
) {
    if w == 0 || h == 0 {
        return;
    }
    for dy in 0..h {
        for dx in 0..w {
            if is_inside_rounded(dx, dy, w, h, r) {
                display::put_pixel(fb, x + dx, y + dy, color);
            }
        }
    }
}

fn draw_rounded_rect_top(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    r: usize,
    color: u32,
) {
    for dy in 0..h {
        for dx in 0..w {
            let inside = if dy < r {
                if dx < r {
                    (dx as isize - r as isize).pow(2) + (dy as isize - r as isize).pow(2)
                        <= (r * r) as isize
                } else if dx >= w.saturating_sub(r) {
                    (dx as isize - (w - r) as isize).pow(2) + (dy as isize - r as isize).pow(2)
                        <= (r * r) as isize
                } else {
                    true
                }
            } else {
                true
            };
            if inside {
                display::put_pixel(fb, x + dx, y + dy, color);
            }
        }
    }
}

fn draw_rounded_rect_outline(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    r: usize,
    thickness: usize,
    color: u32,
) {
    for dy in 0..h {
        for dx in 0..w {
            let inside = is_inside_rounded(dx, dy, w, h, r);
            let inner = is_inside_rounded(
                dx + thickness,
                dy + thickness,
                w.saturating_sub(2 * thickness),
                h.saturating_sub(2 * thickness),
                r.saturating_sub(thickness),
            );
            if inside && !inner {
                display::put_pixel(fb, x + dx, y + dy, color);
            }
        }
    }
}

fn is_inside_rounded(dx: usize, dy: usize, w: usize, h: usize, r: usize) -> bool {
    if dx < r && dy < r {
        return (dx as isize - r as isize).pow(2) + (dy as isize - r as isize).pow(2)
            <= (r * r) as isize;
    }
    if dx >= w.saturating_sub(r) && dy < r {
        return (dx as isize - (w - r) as isize).pow(2) + (dy as isize - r as isize).pow(2)
            <= (r * r) as isize;
    }
    if dx < r && dy >= h.saturating_sub(r) {
        return (dx as isize - r as isize).pow(2) + (dy as isize - (h - r) as isize).pow(2)
            <= (r * r) as isize;
    }
    if dx >= w.saturating_sub(r) && dy >= h.saturating_sub(r) {
        return (dx as isize - (w - r) as isize).pow(2) + (dy as isize - (h - r) as isize).pow(2)
            <= (r * r) as isize;
    }
    true
}
