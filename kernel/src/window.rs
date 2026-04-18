// window.rs
use crate::display::{self, Framebuffer};
use crate::system;

pub const HEADER_H: usize = 34;
pub const CLOSE_SIZE: usize = 16;
pub const CORNER_RADIUS: usize = 10;
pub const SHADOW_SIZE: usize = 8;

#[derive(Copy, Clone)]
pub struct ChromeLayout {
    pub content_x: usize,
    pub content_y: usize,
    pub content_w: usize,
    pub content_h: usize,
    pub close: (usize, usize, usize, usize),
    pub header: (usize, usize, usize, usize),
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

    // Кольори залежно від теми
    let (shadow_base, shadow_mid, shadow_light) = if is_dark {
        (0x00000000, 0x00101010, 0x00202020)
    } else {
        (0x00262D36, 0x00424C57, 0x005A6A7A)
    };

    let (bg_top, bg_bottom) = if is_dark {
        (0x00252525, 0x001E1E1E)
    } else {
        (0x00FFFFFF, 0x00F2F6FC)
    };

    let (header_top, header_bottom) = if is_dark {
        (0x00353535, 0x002C2C2C)
    } else {
        (0x00FFFFFF, 0x00F0F5FB)
    };

    let border_color = if is_dark { 0x00454545 } else { 0x00CCD7E6 };
    let text_color = if is_dark { 0x00F0F2F5 } else { 0x001C2A3A };
    let close_top = 0x00E84A5F;
    let close_bottom = 0x00C92B40;
    let close_text = 0x00FFFFFF;

    // Тінь (імітація розмиття)
    draw_rounded_rect(fb, x + SHADOW_SIZE, y + SHADOW_SIZE, w, h, CORNER_RADIUS, shadow_base);
    draw_rounded_rect(
        fb,
        x + SHADOW_SIZE / 2,
        y + SHADOW_SIZE / 2,
        w,
        h,
        CORNER_RADIUS,
        shadow_mid,
    );
    draw_rounded_rect(
        fb,
        x + SHADOW_SIZE / 4,
        y + SHADOW_SIZE / 4,
        w,
        h,
        CORNER_RADIUS,
        shadow_light,
    );

    // Фон вікна з градієнтом та заокругленнями
    fill_rounded_gradient(fb, x, y, w, h, CORNER_RADIUS, bg_top, bg_bottom);

    // Обвідка
    draw_rounded_rect_outline(fb, x, y, w, h, CORNER_RADIUS, 1, border_color);

    // Заголовок вікна (тільки верхня частина із заокругленнями)
    let header_w = w.saturating_sub(2);
    fill_vertical_gradient_rounded_top(
        fb,
        x + 1,
        y + 1,
        header_w,
        HEADER_H,
        CORNER_RADIUS.saturating_sub(1),
        header_top,
        header_bottom,
    );

    // Акцентна смуга під заголовком
    display::fill_rect(
        fb,
        x + 1,
        y + HEADER_H,
        w.saturating_sub(2),
        2,
        blend_rgb(accent, 0x00FFFFFF, if is_dark { 20 } else { 30 }),
    );

    // Кнопка закриття
    let close = close_rect(x, y, w);
    fill_vertical_gradient(
        fb,
        close.0,
        close.1,
        close.2,
        close.3,
        close_top,
        close_bottom,
    );
    draw_rounded_rect_outline(
        fb,
        close.0,
        close.1,
        close.2,
        close.3,
        4,
        1,
        blend_rgb(close_top, 0x00FFFFFF, 20),
    );

    let mut writer = crate::TextWriter::new(*fb);
    writer.set_color(text_color);
    writer.set_pos(x + 14, y + 10);
    writer.write_bytes(title);

    writer.set_color(close_text);
    writer.set_pos(close.0 + 5, close.1 + 4);
    writer.write_bytes(b"X");

    let header = (x, y, w, HEADER_H);
    let content_x = x + 1;
    let content_y = y + HEADER_H + 2;
    let content_w = w.saturating_sub(2);
    let content_h = h.saturating_sub(HEADER_H + 3);

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
    let cx = x + w.saturating_sub(CLOSE_SIZE + 10);
    let cy = y + (HEADER_H.saturating_sub(CLOSE_SIZE)) / 2;
    (cx, cy, CLOSE_SIZE, CLOSE_SIZE)
}

pub fn header_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    (x, y, w, HEADER_H)
}

pub fn hit(px: usize, py: usize, rect: (usize, usize, usize, usize)) -> bool {
    px >= rect.0 && py >= rect.1 && px < rect.0 + rect.2 && py < rect.1 + rect.3
}

// ---------- Допоміжні функції ----------
fn fill_vertical_gradient(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    top: u32,
    bottom: u32,
) {
    if w == 0 || h == 0 {
        return;
    }
    if h == 1 {
        display::fill_rect(fb, x, y, w, 1, top);
        return;
    }
    let den = (h - 1) as u32;
    for row in 0..h {
        let t = row as u32;
        let c = lerp_rgb(top, bottom, t, den);
        display::fill_rect(fb, x, y + row, w, 1, c);
    }
}

fn fill_vertical_gradient_rounded_top(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    top: u32,
    bottom: u32,
) {
    if w == 0 || h == 0 {
        return;
    }
    let den = (h - 1) as u32;
    for row in 0..h {
        let t = row as u32;
        let c = lerp_rgb(top, bottom, t, den);
        if row < radius {
            for col in 0..w {
                let px = x + col;
                let py = y + row;
                if is_inside_rounded_top(col, row, w, radius) {
                    display::put_pixel(fb, px, py, c);
                }
            }
        } else {
            display::fill_rect(fb, x, y + row, w, 1, c);
        }
    }
}

fn draw_rounded_rect(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    color: u32,
) {
    if w == 0 || h == 0 {
        return;
    }
    for dy in 0..h {
        for dx in 0..w {
            if is_inside_rounded(dx, dy, w, h, radius) {
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
    radius: usize,
    thickness: usize,
    color: u32,
) {
    for dy in 0..h {
        for dx in 0..w {
            let inside = is_inside_rounded(dx, dy, w, h, radius);
            let inside_inner = if thickness > 0 {
                is_inside_rounded(
                    dx + thickness,
                    dy + thickness,
                    w.saturating_sub(2 * thickness),
                    h.saturating_sub(2 * thickness),
                    radius.saturating_sub(thickness),
                )
            } else {
                false
            };
            if inside && !inside_inner {
                display::put_pixel(fb, x + dx, y + dy, color);
            }
        }
    }
}

fn fill_rounded_gradient(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    top: u32,
    bottom: u32,
) {
    if w == 0 || h == 0 {
        return;
    }
    let den = (h - 1) as u32;
    for dy in 0..h {
        let t = dy as u32;
        let c = lerp_rgb(top, bottom, t, den);
        for dx in 0..w {
            if is_inside_rounded(dx, dy, w, h, radius) {
                display::put_pixel(fb, x + dx, y + dy, c);
            }
        }
    }
}

fn is_inside_rounded(dx: usize, dy: usize, w: usize, h: usize, radius: usize) -> bool {
    let r = radius;
    // верхній лівий
    if dx < r && dy < r {
        let dist = (dx as isize - r as isize).pow(2) + (dy as isize - r as isize).pow(2);
        return dist <= (r * r) as isize;
    }
    // верхній правий
    if dx >= w - r && dy < r {
        let cx = (w - r) as isize;
        let dist = (dx as isize - cx).pow(2) + (dy as isize - r as isize).pow(2);
        return dist <= (r * r) as isize;
    }
    // нижній лівий
    if dx < r && dy >= h - r {
        let cy = (h - r) as isize;
        let dist = (dx as isize - r as isize).pow(2) + (dy as isize - cy).pow(2);
        return dist <= (r * r) as isize;
    }
    // нижній правий
    if dx >= w - r && dy >= h - r {
        let cx = (w - r) as isize;
        let cy = (h - r) as isize;
        let dist = (dx as isize - cx).pow(2) + (dy as isize - cy).pow(2);
        return dist <= (r * r) as isize;
    }
    true
}

fn is_inside_rounded_top(dx: usize, dy: usize, w: usize, radius: usize) -> bool {
    let r = radius;
    if dx < r && dy < r {
        let dist = (dx as isize - r as isize).pow(2) + (dy as isize - r as isize).pow(2);
        return dist <= (r * r) as isize;
    }
    if dx >= w - r && dy < r {
        let cx = (w - r) as isize;
        let dist = (dx as isize - cx).pow(2) + (dy as isize - r as isize).pow(2);
        return dist <= (r * r) as isize;
    }
    true
}

fn lerp_rgb(a: u32, b: u32, num: u32, den: u32) -> u32 {
    if den == 0 {
        return a;
    }
    let ar = ((a >> 16) & 0xFF) as u32;
    let ag = ((a >> 8) & 0xFF) as u32;
    let ab = (a & 0xFF) as u32;
    let br = ((b >> 16) & 0xFF) as u32;
    let bg = ((b >> 8) & 0xFF) as u32;
    let bb = (b & 0xFF) as u32;
    let r = (ar * (den - num) + br * num) / den;
    let g = (ag * (den - num) + bg * num) / den;
    let b = (ab * (den - num) + bb * num) / den;
    (r << 16) | (g << 8) | b
}

fn blend_rgb(base: u32, mix: u32, mix_strength: u8) -> u32 {
    let s = mix_strength as u32;
    let inv = 255u32.saturating_sub(s);
    let br = (base >> 16) & 0xFF;
    let bg = (base >> 8) & 0xFF;
    let bb = base & 0xFF;
    let mr = (mix >> 16) & 0xFF;
    let mg = (mix >> 8) & 0xFF;
    let mb = mix & 0xFF;
    let r = (br * inv + mr * s) / 255;
    let g = (bg * inv + mg * s) / 255;
    let b = (bb * inv + mb * s) / 255;
    (r << 16) | (g << 8) | b
}