use crate::display::{self, Framebuffer};
use crate::system;
use crate::window;

const LINE_HEIGHT: usize = 12;
const PAD: usize = 10;
const SCROLL_W: usize = 14;
const BTN_H: usize = 14;
const CLIPBOARD_MAX: usize = 2048;

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
        if hit(
            x,
            y,
            down_rect.0,
            down_rect.1,
            down_rect.2,
            down_rect.3,
        ) {
            self.scroll_down(fb);
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
        body_h.saturating_sub(PAD * 2) / LINE_HEIGHT
    }

    pub fn redraw(&mut self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        let (text, scroll_bg, scroll_btn, content_bg) = if ui.dark {
            (0x00E6E6E6, 0x00323232, 0x00404040, 0x00212121)
        } else {
            (0x00111111, 0x00F0F0F0, 0x00E0E0E0, 0x00FFFFFF)
        };

        let chrome = window::draw_window(fb, x, y, w, h, b"Clipboard");

        let body_y = chrome.content_y;
        let body_h = chrome.content_h;
        display::fill_rect(
            fb,
            chrome.content_x,
            chrome.content_y,
            chrome.content_w,
            chrome.content_h,
            content_bg,
        );

        let scroll_x = x + w.saturating_sub(PAD + SCROLL_W);
        let scroll_y = body_y + PAD;
        let scroll_h = body_h.saturating_sub(PAD * 2);
        display::fill_rect(fb, scroll_x, scroll_y, SCROLL_W, scroll_h, scroll_bg);
        display::fill_rect(fb, scroll_x, scroll_y, SCROLL_W, BTN_H, scroll_btn);
        display::fill_rect(
            fb,
            scroll_x,
            scroll_y + scroll_h.saturating_sub(BTN_H),
            SCROLL_W,
            BTN_H,
            scroll_btn,
        );

        let mut writer = crate::TextWriter::new(*fb);

        writer.set_color(text);
        writer.set_pos(scroll_x + 4, scroll_y + 2);
        writer.write_bytes(b"^");
        writer.set_pos(scroll_x + 4, scroll_y + scroll_h.saturating_sub(BTN_H - 2));
        writer.write_bytes(b"v");

        let data = data();
        let max_lines = self.max_lines(fb);
        let total_lines = count_lines(data);
        let max_scroll = total_lines.saturating_sub(max_lines);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }

        if data.is_empty() {
            writer.set_color(text);
            writer.set_pos(x + PAD, body_y + PAD);
            writer.write_bytes(b"(empty)");
            return;
        }

        let text_x = chrome.content_x + PAD;
        let text_y = body_y + PAD;
        draw_lines(&mut writer, data, self.scroll, max_lines, text_x, text_y);
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
    let mut w = fb.width / 2;
    let mut h = fb.height / 2;
    if w < 280 {
        w = 280;
    }
    if h < 200 {
        h = 200;
    }
    let x = (fb.width - w) / 2;
    let y = (fb.height - h) / 2;
    (x, y, w, h)
}

fn hit(px: usize, py: usize, x: usize, y: usize, w: usize, h: usize) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}
