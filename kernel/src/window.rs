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
    let shadow_color = if is_dark { 0x00000000 } else { 0x00202020 }; // Спрощено для швидкості
    let outer_border = if is_dark { 0x00333333 } else { 0x00D1D5DB };

    // 1. Малюємо тінь (тільки прямокутник, без заокруглень для швидкості)
    display::fill_rect(fb, x + 4, y + 4, w, h, shadow_color);

    // 2. Основне тіло вікна (ШВИДКА ЗАЛИВКА)
    // Замість перевірки кожного пікселя, заливаємо центр прямокутником
    display::fill_rect(fb, x, y + CORNER_RADIUS, w, h - 2 * CORNER_RADIUS, bg_main);
    display::fill_rect(fb, x + CORNER_RADIUS, y, w - 2 * CORNER_RADIUS, CORNER_RADIUS, bg_main);
    display::fill_rect(fb, x + CORNER_RADIUS, y + h - CORNER_RADIUS, w - 2 * CORNER_RADIUS, CORNER_RADIUS, bg_main);

    // 3. Малюємо закруглені кути окремо (всього 4 маленькі зони замість всього вікна)
    draw_corner(fb, x, y, CORNER_RADIUS, 0, bg_main); // Top-left
    draw_corner(fb, x + w - CORNER_RADIUS, y, CORNER_RADIUS, 1, bg_main); // Top-right
    draw_corner(fb, x, y + h - CORNER_RADIUS, CORNER_RADIUS, 2, bg_main); // Bottom-left
    draw_corner(fb, x + w - CORNER_RADIUS, y + h - CORNER_RADIUS, CORNER_RADIUS, 3, bg_main); // Bottom-right

    // 4. Шапка вікна
    display::fill_rect(fb, x + CORNER_RADIUS, y, w - 2 * CORNER_RADIUS, HEADER_H, header_bg);
    display::fill_rect(fb, x, y + CORNER_RADIUS, w, HEADER_H - CORNER_RADIUS, header_bg);
    draw_corner(fb, x, y, CORNER_RADIUS, 0, header_bg);
    draw_corner(fb, x + w - CORNER_RADIUS, y, CORNER_RADIUS, 1, header_bg);

    // Лінія розділювач
    display::fill_rect(fb, x, y + HEADER_H - 1, w, 1, outer_border);
    
    // Акцентний індикатор
    display::fill_rect(fb, x + 8, y + 14, 3, 14, accent);

    // 5. Кнопки керування
    let close = close_rect(x, y, w);
    let maximize = maximize_rect(x, y, w);
    let minimize = minimize_rect(x, y, w);
    let btn_bg = if is_dark { 0x00333333 } else { 0x00E5E7EB };

    display::fill_rect(fb, minimize.0, minimize.1, minimize.2, minimize.3, btn_bg);
    display::fill_rect(fb, maximize.0, maximize.1, maximize.2, maximize.3, btn_bg);
    display::fill_rect(fb, close.0, close.1, close.2, close.3, btn_bg);

    // 6. Текст
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

/// Малює лише один конкретний кут (0: TL, 1: TR, 2: BL, 3: BR)
fn draw_corner(fb: &Framebuffer, x: usize, y: usize, r: usize, corner: u8, color: u32) {
    let r_i = r as isize;
    let r_sq = r_i * r_i;
    for dy in 0..r {
        for dx in 0..r {
            let (px, py) = (dx as isize, dy as isize);
            let inside = match corner {
                0 => (px - r_i).pow(2) + (py - r_i).pow(2) <= r_sq, // Top-Left
                1 => (px).pow(2) + (py - r_i).pow(2) <= r_sq,       // Top-Right
                2 => (px - r_i).pow(2) + (py).pow(2) <= r_sq,       // Bottom-Left
                3 => (px).pow(2) + (py).pow(2) <= r_sq,             // Bottom-Right
                _ => true,
            };
            if inside {
                display::put_pixel(fb, x + dx, y + dy, color);
            }
        }
    }
}

// Решта допоміжних функцій (без змін, вони швидкі)
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