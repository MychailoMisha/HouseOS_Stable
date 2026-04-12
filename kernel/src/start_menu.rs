use crate::display::{self, Framebuffer};
use crate::system;

const LINE_HEIGHT: usize = 20;
const PAD: usize = 12;
const HEADER_H: usize = 68;
const CLOSE_SIZE: usize = 14;
const BAR_H: usize = 26;
const ICON_SIZE: usize = 16;
const AVATAR_SIZE: usize = 36;

#[derive(Copy, Clone)]
pub enum StartAction {
    OpenConsole,
    OpenExplorer,
    OpenClipboard,
    OpenBin,
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
        
        // Закриття через хрестик
        let (cx, cy, cw, ch) = close_rect(wx, wy, ww);
        if hit(x, y, cx, cy, cw, ch) {
            self.hide(fb);
            return None;
        }

        // Перевірка кліку по меню
        let _list_x = wx + PAD;
        let list_y = wy + HEADER_H + 8;
        
        if x >= wx && x < wx + ww && y >= list_y {
            let row = (y - list_y) / LINE_HEIGHT;
            // Карта дій (пропускаємо роздільник на 4-й позиції)
            return match row {
                0 => Some(StartAction::OpenConsole),
                1 => Some(StartAction::OpenExplorer),
                2 => Some(StartAction::OpenClipboard),
                3 => Some(StartAction::OpenBin),
                // Роздільник
                4 => None,
                5 => Some(StartAction::ToggleTheme),
                6 => Some(StartAction::Reboot),
                7 => Some(StartAction::Shutdown),
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

        // --- АКРИЛОВИЙ ФОН (Windows 10 Fluent Design) ---
        draw_acrylic_surface(fb, x, y, w, h, is_dark);
        
        // Тонка рамка
        let border_color = if is_dark { 0x00404040 } else { 0x00C0C0C0 };
        display::fill_rect(fb, x, y, w, 1, border_color);
        display::fill_rect(fb, x, y + h.saturating_sub(1), w, 1, border_color);
        display::fill_rect(fb, x, y, 1, h, border_color);
        display::fill_rect(fb, x + w.saturating_sub(1), y, 1, h, border_color);

        // --- ЗАГОЛОВОК (Header) ---
        let header_bg = if is_dark { 0x00333333 } else { 0x00F3F3F3 };
        display::fill_rect(fb, x + 1, y + 1, w.saturating_sub(2), HEADER_H, header_bg);
        
        // Аватар (коло)
        let avatar_x = x + 16;
        let avatar_y = y + 16;
        draw_avatar(fb, avatar_x, avatar_y, AVATAR_SIZE, accent);
        
        // Текст "Start" та ім'я користувача
        let mut writer = crate::TextWriter::new(*fb);
        let text_color = if is_dark { 0x00FFFFFF } else { 0x00000000 };
        writer.set_color(text_color);
        writer.set_pos(avatar_x + AVATAR_SIZE + 12, y + 22);
        writer.write_bytes(b"User");
        writer.set_pos(avatar_x + AVATAR_SIZE + 12, y + 40);
        writer.set_color(if is_dark { 0x00AAAAAA } else { 0x00666666 });
        writer.write_bytes(b"user@aurora-os");

        // Кнопка закриття (X) в стилі Windows 10
        let (cx, cy, cw, ch) = close_rect(x, y, w);
        display::fill_rect(fb, cx, cy, cw, ch, 0x00E81123);
        writer.set_color(0x00FFFFFF);
        writer.set_pos(cx + 4, cy + 3);
        writer.write_bytes(b"X");

        // --- СПИСОК ДІЙ ---
        let list_x = x + PAD;
        let list_y = y + HEADER_H + 8;
        writer.set_color(text_color);
        
        let items = [
            ("Console", ">"),
            ("Explorer", "#"),
            ("Clipboard", "@"),
            ("Recycle Bin", "%"),
        ];
        
        let actions_after = [
            ("Theme", if is_dark { "O" } else { "o" }),
            ("Restart", "R"),
            ("Shutdown", "S"),
        ];

        // Малюємо основні пункти
        let mut current_row = 0;
        for (label, icon) in items.iter() {
            let row_y = list_y + current_row * LINE_HEIGHT;
            
            // Іконка
            writer.set_pos(list_x + 4, row_y + 3);
            writer.write_bytes(icon.as_bytes());
            
            // Текст
            writer.set_color(text_color);
            writer.set_pos(list_x + ICON_SIZE + 12, row_y + 3);
            writer.write_bytes(label.as_bytes());
            
            current_row += 1;
        }
        
        // Роздільник
        let sep_y = list_y + current_row * LINE_HEIGHT + 10;
        let sep_color = if is_dark { 0x00444444 } else { 0x00CCCCCC };
        display::fill_rect(fb, list_x, sep_y, w - PAD * 2, 1, sep_color);
        current_row += 1;

        // Малюємо нижні пункти
        let theme_label = if is_dark { "Switch to Light Mode" } else { "Switch to Dark Mode" };
        
        for (i, (label, icon)) in actions_after.iter().enumerate() {
            let row_y = list_y + (current_row + i) * LINE_HEIGHT;
            
            writer.set_pos(list_x + 4, row_y + 3);
            writer.write_bytes(icon.as_bytes());
            
            writer.set_color(text_color);
            writer.set_pos(list_x + ICON_SIZE + 12, row_y + 3);
            
            if i == 0 {
                writer.write_bytes(theme_label.as_bytes());
            } else {
                writer.write_bytes(label.as_bytes());
            }
        }

        // АКЦЕНТНА ЛІНІЯ (знизу заголовка)
        display::fill_rect(fb, x + 1, y + HEADER_H.saturating_sub(1), w.saturating_sub(2), 2, accent);
    }

    pub fn rect(&self, fb: &Framebuffer) -> (usize, usize, usize, usize) {
        if self.win_w == 0 || self.win_h == 0 {
            return calc_rect(fb);
        }
        (self.win_x, self.win_y, self.win_w, self.win_h)
    }
}

fn draw_acrylic_surface(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize, is_dark: bool) {
    let base = if is_dark { 0x801C1C1C } else { 0x80F0F0F0 };
    display::fill_rect(fb, x, y, w, h, base);
}

fn draw_avatar(fb: &Framebuffer, x: usize, y: usize, size: usize, color: u32) {
    let r = size / 2;
    let _cx = x + r;
    let _cy = y + r;
    
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
    
    // Біла обводка
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
    let w = 340;
    let h = 420;
    let x = 16usize;
    let y = fb.height.saturating_sub(h + BAR_H + 10);
    (x, y, w, h)
}

fn close_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    let cx = x + w.saturating_sub(CLOSE_SIZE + 12);
    let cy = y + 12;
    (cx, cy, CLOSE_SIZE, CLOSE_SIZE)
}

fn hit(px: usize, py: usize, x: usize, y: usize, w: usize, h: usize) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}