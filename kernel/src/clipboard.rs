use crate::display::{self, Framebuffer};
use crate::system;
use crate::window;

const LINE_HEIGHT: usize = 20;          // збільшено для кращої читабельності
const PAD: usize = 12;
const SCROLL_W: usize = 14;
const BTN_H: usize = 16;
const CLIPBOARD_MAX: usize = 2048;
const CLEAR_BTN_W: usize = 60;
const CLEAR_BTN_H: usize = 22;

static mut CLIPBOARD: [u8; CLIPBOARD_MAX] = [0; CLIPBOARD_MAX];
static mut CLIPBOARD_LEN: usize = 0;

pub fn set(data: &[u8]) {
    let mut len = data.len();
    if len > CLIPBOARD_MAX {
        len = CLIPBOARD_MAX;
    }
    unsafe {
        CLIPBOARD_LEN = len;
        if len > 0 {
            CLIPBOARD[..len].copy_from_slice(&data[..len]);
        }
    }
}

pub fn data() -> &'static [u8] {
    unsafe { &CLIPBOARD[..CLIPBOARD_LEN] }
}

pub fn clear() {
    unsafe {
        CLIPBOARD_LEN = 0;
    }
}

pub struct ClipboardWindow {
    visible: bool,
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,
    scroll: usize,
}

impl ClipboardWindow {
    pub fn new(_fb: Framebuffer) -> Self {
        Self {
            visible: false,
            win_x: 0,
            win_y: 0,
            win_w: 0,
            win_h: 0,
            scroll: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, fb: &Framebuffer) {
        self.visible = true;
        self.scroll = 0;
        if self.win_w == 0 || self.win_h == 0 {
            let (x, y, w, h) = calc_rect(fb);
            self.win_x = x;
            self.win_y = y;
            self.win_w = w;
            self.win_h = h;
        }
    }

    pub fn hide(&mut self, _fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        self.visible = false;
    }

    pub fn handle_click(&mut self, fb: &Framebuffer, x: usize, y: usize) -> bool {
        if !self.visible {
            return false;
        }
        let (wx, wy, ww, wh) = self.rect(fb);

        // Перевірка кнопки очищення (у верхньому правому куті області вмісту)
        let clear_x = wx + ww - PAD - CLEAR_BTN_W;
        let clear_y = wy + window::HEADER_H + PAD;
        if hit(x, y, clear_x, clear_y, CLEAR_BTN_W, CLEAR_BTN_H) {
            clear();
            self.scroll = 0;
            self.redraw(fb);
            return true;
        }

        let body_y = wy + window::HEADER_H + 1;
        let body_h = wh.saturating_sub(window::HEADER_H + 2);
        let scroll_x = wx + ww.saturating_sub(PAD + SCROLL_W);
        let scroll_y = body_y + PAD;
        let scroll_h = body_h.saturating_sub(PAD * 2);

        let up_rect = (scroll_x, scroll_y, SCROLL_W, BTN_H);
        let down_rect = (
            scroll_x,
            scroll_y + scroll_h.saturating_sub(BTN_H),
            SCROLL_W,
            BTN_H,
        );
        if hit(x, y, up_rect.0, up_rect.1, up_rect.2, up_rect.3) {
            self.scroll_up(fb);
            return true;
        }
        if hit(x, y, down_rect.0, down_rect.1, down_rect.2, down_rect.3) {
            self.scroll_down(fb);
            return true;
        }

        // Клік по треку смуги прокрутки (переміщення повзунка)
        let track_y = scroll_y + BTN_H;
        let track_h = scroll_h.saturating_sub(BTN_H * 2);
        if x >= scroll_x && x < scroll_x + SCROLL_W && y >= track_y && y < track_y + track_h {
            let total_lines = count_lines(data());
            let max_lines = self.max_lines(fb);
            let max_scroll = total_lines.saturating_sub(max_lines);
            if max_scroll > 0 {
                let ratio = (y - track_y) as f32 / track_h as f32;
                let new_scroll = (ratio * max_scroll as f32) as usize;
                self.scroll = new_scroll.min(max_scroll);
                self.redraw(fb);
            }
            return true;
        }

        false
    }

    pub fn scroll_up(&mut self, fb: &Framebuffer) {
        if self.scroll > 0 {
            self.scroll -= 1;
            self.redraw(fb);
        }
    }

    pub fn scroll_down(&mut self, fb: &Framebuffer) {
        let data = data();
        let total_lines = count_lines(data);
        let max_lines = self.max_lines(fb);
        let max_scroll = total_lines.saturating_sub(max_lines);
        if self.scroll < max_scroll {
            self.scroll += 1;
            self.redraw(fb);
        }
    }

    fn max_lines(&self, fb: &Framebuffer) -> usize {
        let (_, _, _, h) = self.rect(fb);
        let body_h = h.saturating_sub(window::HEADER_H + 2);
        // Віднімаємо місце для кнопки очищення
        let available_h = body_h.saturating_sub(PAD * 2 + CLEAR_BTN_H + PAD);
        available_h / LINE_HEIGHT
    }

    pub fn redraw(&mut self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        let accent = ui.accent;
        let is_dark = ui.dark;

        // Акриловий фон
        draw_acrylic_surface(fb, x, y, w, h, is_dark);

        // Тонка рамка
        let border_color = if is_dark { 0x00404040 } else { 0x00C0C0C0 };
        display::fill_rect(fb, x, y, w, 1, border_color);
        display::fill_rect(fb, x, y + h.saturating_sub(1), w, 1, border_color);
        display::fill_rect(fb, x, y, 1, h, border_color);
        display::fill_rect(fb, x + w.saturating_sub(1), y, 1, h, border_color);

        // Заголовок у стилі Windows 10
        let header_bg = if is_dark { 0x00333333 } else { 0x00F3F3F3 };
        display::fill_rect(fb, x + 1, y + 1, w.saturating_sub(2), window::HEADER_H, header_bg);
        // Акцентна лінія під заголовком
        display::fill_rect(fb, x + 1, y + window::HEADER_H - 1, w.saturating_sub(2), 2, accent);

        let mut writer = crate::TextWriter::new(*fb);
        let text_color = if is_dark { 0x00FFFFFF } else { 0x00000000 };
        writer.set_color(text_color);
        writer.set_pos(x + PAD, y + 8);
        writer.write_bytes(b"Clipboard");

        // Кнопка закриття (X)
        let close = window::close_rect(x, y, w);
        display::fill_rect(fb, close.0, close.1, close.2, close.3, 0x00E81123);
        writer.set_color(0x00FFFFFF);
        writer.set_pos(close.0 + 4, close.1 + 3);
        writer.write_bytes(b"X");

        // Область вмісту (тіло)
        let body_y = y + window::HEADER_H + 1;
        let body_h = h.saturating_sub(window::HEADER_H + 2);
        let content_bg = if is_dark { 0x801C1C1C } else { 0x80FFFFFF };
        display::fill_rect(fb, x + 1, body_y, w.saturating_sub(2), body_h, content_bg);

        // Кнопка очищення
        let clear_x = x + w - PAD - CLEAR_BTN_W;
        let clear_y = body_y + PAD;
        let clear_bg = if is_dark { 0x00404040 } else { 0x00D0D0D0 };
        display::fill_rect(fb, clear_x, clear_y, CLEAR_BTN_W, CLEAR_BTN_H, clear_bg);
        display::fill_rect(fb, clear_x, clear_y, CLEAR_BTN_W, 1, border_color);
        display::fill_rect(fb, clear_x, clear_y + CLEAR_BTN_H - 1, CLEAR_BTN_W, 1, border_color);
        writer.set_color(text_color);
        writer.set_pos(clear_x + 8, clear_y + 5);
        writer.write_bytes(b"Clear");

        // Смуга прокрутки
        let scroll_x = x + w.saturating_sub(PAD + SCROLL_W);
        let scroll_y = body_y + PAD;
        let scroll_h = body_h.saturating_sub(PAD * 2);
        let scroll_bg = if is_dark { 0x00323232 } else { 0x00E0E0E0 };
        display::fill_rect(fb, scroll_x, scroll_y, SCROLL_W, scroll_h, scroll_bg);

        // Кнопки вгору/вниз
        let btn_color = if is_dark { 0x00505050 } else { 0x00C0C0C0 };
        display::fill_rect(fb, scroll_x, scroll_y, SCROLL_W, BTN_H, btn_color);
        display::fill_rect(fb, scroll_x, scroll_y + scroll_h.saturating_sub(BTN_H), SCROLL_W, BTN_H, btn_color);
        writer.set_color(text_color);
        writer.set_pos(scroll_x + 4, scroll_y + 3);
        writer.write_bytes(b"^");
        writer.set_pos(scroll_x + 4, scroll_y + scroll_h.saturating_sub(BTN_H - 3));
        writer.write_bytes(b"v");

        // Повзунок смуги прокрутки
        let data = data();
        let total_lines = count_lines(data);
        let max_lines = self.max_lines(fb);
        let max_scroll = total_lines.saturating_sub(max_lines);
        if max_scroll > 0 {
            let thumb_h = ((max_lines as f32 / total_lines as f32) * (scroll_h - 2 * BTN_H) as f32) as usize;
            let thumb_h = thumb_h.max(16);
            let track_h = scroll_h - 2 * BTN_H;
            let thumb_y = scroll_y + BTN_H + ((self.scroll as f32 / max_scroll as f32) * (track_h - thumb_h) as f32) as usize;
            let thumb_color = if is_dark { 0x00808080 } else { 0x00A0A0A0 };
            display::fill_rect(fb, scroll_x, thumb_y, SCROLL_W, thumb_h, thumb_color);
        }

        // Відображення тексту
        let text_x = x + PAD;
        let text_y = body_y + PAD + CLEAR_BTN_H + PAD;
        if data.is_empty() {
            writer.set_color(text_color);
            writer.set_pos(text_x, text_y);
            writer.write_bytes(b"(empty)");
        } else {
            draw_lines(&mut writer, data, self.scroll, max_lines, text_x, text_y);
        }
    }

    pub fn rect(&self, fb: &Framebuffer) -> (usize, usize, usize, usize) {
        if self.win_w == 0 || self.win_h == 0 {
            return calc_rect(fb);
        }
        (self.win_x, self.win_y, self.win_w, self.win_h)
    }

    pub fn set_pos(&mut self, x: usize, y: usize) {
        self.win_x = x;
        self.win_y = y;
    }
}

fn count_lines(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }
    let mut lines = 1usize;
    for &b in data {
        if b == b'\n' {
            lines += 1;
        }
    }
    lines
}

fn draw_lines(
    writer: &mut crate::TextWriter,
    data: &[u8],
    start_line: usize,
    max_lines: usize,
    x: usize,
    y: usize,
) {
    let mut line_idx = 0usize;
    let mut row = 0usize;
    let mut start = 0usize;
    let len = data.len();
    let mut i = 0usize;
    while i <= len {
        let at_end = i == len;
        let is_break = if at_end { true } else { data[i] == b'\n' };
        if is_break {
            if line_idx >= start_line && row < max_lines {
                let mut end = i;
                if end > start && data[end - 1] == b'\r' {
                    end -= 1;
                }
                writer.set_pos(x, y + row * LINE_HEIGHT);
                if end > start {
                    writer.write_bytes(&data[start..end]);
                }
                row += 1;
            }
            line_idx += 1;
            if row >= max_lines {
                return;
            }
            start = i + 1;
        }
        i += 1;
    }
}

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let w = (fb.width * 3 / 5).min(500).max(320);
    let h = (fb.height * 3 / 5).min(400).max(240);
    let x = (fb.width - w) / 2;
    let y = (fb.height - h) / 2;
    (x, y, w, h)
}

fn hit(px: usize, py: usize, x: usize, y: usize, w: usize, h: usize) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}

fn draw_acrylic_surface(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize, is_dark: bool) {
    let base = if is_dark { 0x801C1C1C } else { 0x80F0F0F0 };
    display::fill_rect(fb, x, y, w, h, base);
}