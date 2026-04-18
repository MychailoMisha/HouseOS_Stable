// start_menu.rs
use crate::display::{self, Framebuffer};
use crate::system;

const LINE_HEIGHT: usize = 22;
const PAD: usize = 12;
const HEADER_H: usize = 72;
const CLOSE_SIZE: usize = 18;
const BAR_H: usize = 26;
const ICON_SIZE: usize = 18;
const AVATAR_SIZE: usize = 40;
const CORNER_RADIUS: usize = 12;

#[derive(Copy, Clone)]
pub enum StartAction {
    OpenConsole,
    OpenExplorer,
    OpenClipboard,
    OpenBin,
    OpenCalculator,
    ToggleTheme,
    Reboot,
    Shutdown,
}

pub struct StartMenu {
    visible: bool,
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,
}

impl StartMenu {
    pub fn new(_fb: Framebuffer) -> Self {
        Self {
            visible: false,
            win_x: 0,
            win_y: 0,
            win_w: 0,
            win_h: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, fb: &Framebuffer) {
        self.visible = true;
        let (x, y, w, h) = calc_rect(fb);
        self.win_x = x;
        self.win_y = y;
        self.win_w = w;
        self.win_h = h;
    }

    pub fn hide(&mut self, _fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        self.visible = false;
    }

    pub fn handle_click(
        &mut self,
        fb: &Framebuffer,
        x: usize,
        y: usize,
    ) -> Option<StartAction> {
        if !self.visible {
            return None;
        }
        let (wx, wy, ww, _) = self.rect(fb);

        // Закриття через хрестик (збільшена зона кліку)
        let (cx, cy, cw, ch) = close_rect(wx, wy, ww);
        // Розширюємо зону кліку на 4 пікселі з кожного боку
        if hit(x, y, cx.saturating_sub(4), cy.saturating_sub(4), cw + 8, ch + 8) {
            self.hide(fb);
            return None;
        }

        // Перевірка кліку по меню
        let _list_x = wx + PAD;
        let list_y = wy + HEADER_H + 8;

        if x >= wx && x < wx + ww && y >= list_y {
            let row = (y - list_y) / LINE_HEIGHT;
            return match row {
                0 => Some(StartAction::OpenConsole),
                1 => Some(StartAction::OpenExplorer),
                2 => Some(StartAction::OpenClipboard),
                3 => Some(StartAction::OpenBin),
                4 => Some(StartAction::OpenCalculator),
                5 => None, // роздільник
                6 => Some(StartAction::ToggleTheme),
                7 => Some(StartAction::Reboot),
                8 => Some(StartAction::Shutdown),
                _ => None,
            };
        }
        None
    }

    pub fn refresh(&self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        self.redraw(fb);
    }

    fn redraw(&self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        let accent = ui.accent;
        let is_dark = ui.dark;

        // Тінь
        draw_rounded_rect(fb, x + 6, y + 6, w, h, CORNER_RADIUS, 0x00101010);
        draw_rounded_rect(fb, x + 3, y + 3, w, h, CORNER_RADIUS, 0x001A1A1A);

        // Основний фон із заокругленнями
        draw_rounded_rect(
            fb,
            x,
            y,
            w,
            h,
            CORNER_RADIUS,
            if is_dark { 0x00222222 } else { 0x00F9FCFF },
        );

        // Обвідка
        draw_rounded_rect_outline(
            fb,
            x,
            y,
            w,
            h,
            CORNER_RADIUS,
            1,
            if is_dark { 0x00494949 } else { 0x00CDDAEA },
        );

        // Заголовок із градієнтом (лише верхня частина)
        let header_w = w.saturating_sub(2);
        fill_vertical_gradient_rounded_top(
            fb,
            x + 1,
            y + 1,
            header_w,
            HEADER_H,
            CORNER_RADIUS.saturating_sub(1),
            if is_dark { 0x00353535 } else { 0x00FFFFFF },
            if is_dark { 0x002C2C2C } else { 0x00EEF4FC },
        );

        // Акцентна смуга
        display::fill_rect(
            fb,
            x + 1,
            y + HEADER_H,
            w.saturating_sub(2),
            2,
            blend_rgb(accent, 0x00FFFFFF, if is_dark { 18 } else { 28 }),
        );

        // Аватар
        let avatar_x = x + 16;
        let avatar_y = y + 16;
        draw_avatar(fb, avatar_x, avatar_y, AVATAR_SIZE, accent);

        let mut writer = crate::TextWriter::new(*fb);
        let text_color = if is_dark { 0x00F2F5F8 } else { 0x00111D2B };
        let secondary_text = if is_dark { 0x00B7C0CC } else { 0x004D5D72 };

        writer.set_color(text_color);
        writer.set_pos(avatar_x + AVATAR_SIZE + 12, y + 24);
        writer.write_bytes(b"User");
        writer.set_pos(avatar_x + AVATAR_SIZE + 12, y + 44);
        writer.set_color(secondary_text);
        writer.write_bytes(b"user@aurora-os");

        // Кнопка закриття
        let (cx, cy, cw, ch) = close_rect(x, y, w);
        fill_vertical_gradient(fb, cx, cy, cw, ch, 0x00E84A5F, 0x00C92B40);
        draw_rounded_rect_outline(fb, cx, cy, cw, ch, 4, 1, blend_rgb(0x00E84A5F, 0x00FFFFFF, 20));
        writer.set_color(0x00FFFFFF);
        writer.set_pos(cx + 6, cy + 5);
        writer.write_bytes(b"X");

        // Список пунктів
        let list_x = x + PAD;
        let list_y = y + HEADER_H + 8;
        writer.set_color(text_color);

        let items = [
            ("Console", ">"),
            ("Explorer", "#"),
            ("Clipboard", "@"),
            ("Recycle Bin", "%"),
            ("Calculator", "C"),
        ];

        let actions_after = [
            ("Theme", if is_dark { "O" } else { "o" }),
            ("Restart", "R"),
            ("Shutdown", "S"),
        ];

        let mut current_row = 0;
        for (label, icon) in items.iter() {
            let row_y = list_y + current_row * LINE_HEIGHT;
            let card_bg = if is_dark { 0x002A2A2A } else { 0x00F8FBFF };
            display::fill_rect(
                fb,
                list_x,
                row_y.saturating_sub(1),
                w.saturating_sub(PAD * 2),
                LINE_HEIGHT.saturating_sub(1),
                card_bg,
            );

            writer.set_color(secondary_text);
            writer.set_pos(list_x + 4, row_y + 3);
            writer.write_bytes(icon.as_bytes());

            writer.set_color(text_color);
            writer.set_pos(list_x + ICON_SIZE + 12, row_y + 3);
            writer.write_bytes(label.as_bytes());

            current_row += 1;
        }

        // Роздільник
        let sep_y = list_y + current_row * LINE_HEIGHT + 10;
        let sep_color = if is_dark { 0x00474747 } else { 0x00D2DDEA };
        display::fill_rect(fb, list_x, sep_y, w - PAD * 2, 1, sep_color);
        current_row += 1;

        let theme_label = if is_dark {
            "Switch to Light Mode"
        } else {
            "Switch to Dark Mode"
        };

        for (i, (label, icon)) in actions_after.iter().enumerate() {
            let row_y = list_y + (current_row + i) * LINE_HEIGHT;

            let action_bg = if i == 0 {
                blend_rgb(
                    accent,
                    if is_dark { 0x001E1E1E } else { 0x00FFFFFF },
                    if is_dark { 44 } else { 78 },
                )
            } else if is_dark {
                0x002A2A2A
            } else {
                0x00F8FBFF
            };
            display::fill_rect(
                fb,
                list_x,
                row_y.saturating_sub(1),
                w.saturating_sub(PAD * 2),
                LINE_HEIGHT.saturating_sub(1),
                action_bg,
            );

            let row_text = if i == 0 { 0x00FFFFFF } else { text_color };
            writer.set_color(row_text);
            writer.set_pos(list_x + 4, row_y + 3);
            writer.write_bytes(icon.as_bytes());

            writer.set_color(row_text);
            writer.set_pos(list_x + ICON_SIZE + 12, row_y + 3);

            if i == 0 {
                writer.write_bytes(theme_label.as_bytes());
            } else {
                writer.write_bytes(label.as_bytes());
            }
        }
    }

    pub fn rect(&self, fb: &Framebuffer) -> (usize, usize, usize, usize) {
        if self.win_w == 0 || self.win_h == 0 {
            return calc_rect(fb);
        }
        (self.win_x, self.win_y, self.win_w, self.win_h)
    }
}

// ---------- Допоміжні функції ----------
fn draw_acrylic_surface(_fb: &Framebuffer, _x: usize, _y: usize, _w: usize, _h: usize, _is_dark: bool) {
    // Замінено на draw_rounded_rect з градієнтом
}

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
        let c = lerp_rgb(top, bottom, row as u32, den);
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
                if is_inside_rounded_top(col, row, w, radius) {
                    display::put_pixel(fb, x + col, y + row, c);
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

fn is_inside_rounded(dx: usize, dy: usize, w: usize, h: usize, radius: usize) -> bool {
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
    if dx < r && dy >= h - r {
        let cy = (h - r) as isize;
        let dist = (dx as isize - r as isize).pow(2) + (dy as isize - cy).pow(2);
        return dist <= (r * r) as isize;
    }
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

fn draw_avatar(fb: &Framebuffer, x: usize, y: usize, size: usize, color: u32) {
    let r = size / 2;
    for dy in 0..size {
        for dx in 0..size {
            let px = x + dx;
            let py = y + dy;
            let dist = ((dx as isize - r as isize).pow(2) + (dy as isize - r as isize).pow(2)) as f32;
            if dist <= (r * r) as f32 {
                display::put_pixel(fb, px, py, color);
            }
        }
    }
    // обводка
    for dy in 0..size {
        for dx in 0..size {
            let px = x + dx;
            let py = y + dy;
            let dist = ((dx as isize - r as isize).pow(2) + (dy as isize - r as isize).pow(2)) as f32;
            let outer = (r * r) as f32;
            let inner = ((r - 1) * (r - 1)) as f32;
            if dist <= outer && dist > inner {
                display::put_pixel(fb, px, py, 0x00FFFFFF);
            }
        }
    }
}

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let w = 360;
    let h = 480;
    let x = 16usize;
    let y = fb.height.saturating_sub(h + BAR_H + 10);
    (x, y, w, h)
}

fn close_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    let cx = x + w.saturating_sub(CLOSE_SIZE + 12);
    let cy = y + (HEADER_H.saturating_sub(CLOSE_SIZE)) / 2;
    (cx, cy, CLOSE_SIZE, CLOSE_SIZE)
}

fn hit(px: usize, py: usize, x: usize, y: usize, w: usize, h: usize) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}