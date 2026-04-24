use crate::display::{self, Framebuffer};
use crate::fat32::Fat32;
use crate::system;
use crate::window;
use crate::ModuleRange;

const MAX_TEXT: usize = 8192;
const MAX_TITLE: usize = 48;
const PAD: usize = 12;
const LINE_HEIGHT: usize = 16;
const SCROLL_W: usize = 14;
const BTN_H: usize = 16;

pub struct Notepad {
    visible: bool,
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,
    fs_img: Option<ModuleRange>,
    title: [u8; MAX_TITLE],
    title_len: usize,
    text: [u8; MAX_TEXT],
    text_len: usize,
    scroll: usize,
}

impl Notepad {
    pub fn new(_fb: Framebuffer, fs_img: Option<ModuleRange>) -> Self {
        let mut title = [0u8; MAX_TITLE];
        let base = b"Notepad";
        title[..base.len()].copy_from_slice(base);
        Self {
            visible: false,
            win_x: 0,
            win_y: 0,
            win_w: 0,
            win_h: 0,
            fs_img,
            title,
            title_len: base.len(),
            text: [0u8; MAX_TEXT],
            text_len: 0,
            scroll: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, fb: &Framebuffer) {
        self.visible = true;
        if self.win_w == 0 || self.win_h == 0 {
            let (x, y, w, h) = calc_rect(fb);
            self.win_x = x;
            self.win_y = y;
            self.win_w = w;
            self.win_h = h;
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn open_empty(&mut self, fb: &Framebuffer) {
        self.show(fb);
        self.set_title(b"(new)");
        self.text_len = 0;
        self.scroll = 0;
    }

    pub fn open_file(
        &mut self,
        fb: &Framebuffer,
        cluster: u32,
        size: u32,
        file_name: &[u8],
    ) {
        self.show(fb);
        self.set_title(file_name);
        self.scroll = 0;
        self.text_len = 0;

        let fs_img = self.fs_img;
        if let Some(fs) = fs_img.and_then(Fat32::new) {
            let read = fs.read_file(cluster, size as usize, &mut self.text);
            self.text_len = sanitize_loaded_text(&mut self.text, read);
        }
    }

    pub fn handle_click(&mut self, fb: &Framebuffer, x: usize, y: usize) -> bool {
        if !self.visible {
            return false;
        }
        let (wx, wy, ww, wh) = self.rect(fb);
        let body_y = wy + window::HEADER_H + 2;
        let body_h = wh.saturating_sub(window::HEADER_H + 4);

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
            self.scroll_up();
            self.redraw(fb);
            return true;
        }
        if hit(x, y, down_rect.0, down_rect.1, down_rect.2, down_rect.3) {
            self.scroll_down(fb);
            self.redraw(fb);
            return true;
        }

        let track_y = scroll_y + BTN_H;
        let track_h = scroll_h.saturating_sub(BTN_H * 2);
        if x >= scroll_x && x < scroll_x + SCROLL_W && y >= track_y && y < track_y + track_h {
            let (_, max_lines) = self.layout_metrics(fb);
            let total = wrapped_line_count(&self.text[..self.text_len], max_lines.0);
            let max_scroll = total.saturating_sub(max_lines.1);
            if max_scroll > 0 && track_h > 0 {
                let ratio = (y - track_y) as f32 / track_h as f32;
                self.scroll = ((ratio * max_scroll as f32) as usize).min(max_scroll);
                self.redraw(fb);
            }
            return true;
        }

        false
    }

    pub fn handle_char(&mut self, ch: u8) {
        if !self.visible {
            return;
        }
        match ch {
            0x08 => {
                if self.text_len > 0 {
                    self.text_len -= 1;
                }
            }
            b'\n' => self.push_byte(b'\n'),
            b'\t' => {
                self.push_byte(b' ');
                self.push_byte(b' ');
                self.push_byte(b' ');
                self.push_byte(b' ');
            }
            _ if (32..=126).contains(&ch) => self.push_byte(ch),
            _ => {}
        }
    }

    pub fn redraw(&mut self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }

        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        let is_dark = ui.dark;

        let chrome = window::draw_window(fb, x, y, w, h, &self.title[..self.title_len]);
        fill_vertical_gradient(
            fb,
            chrome.content_x,
            chrome.content_y,
            chrome.content_w,
            chrome.content_h,
            if is_dark { 0x001D1D1D } else { 0x00FFFFFF },
            if is_dark { 0x00181818 } else { 0x00F6FAFF },
        );

        let mut writer = crate::TextWriter::new(*fb);
        let text_color = if is_dark { 0x00F2F5F8 } else { 0x00131A28 };
        let detail = if is_dark { 0x00B7C0CC } else { 0x004D5D72 };

        let text_x = chrome.content_x + PAD;
        let text_y = chrome.content_y + PAD;
        let text_w = chrome.content_w.saturating_sub(PAD * 2 + SCROLL_W + 4);
        let text_h = chrome.content_h.saturating_sub(PAD * 2);
        let max_cols = (text_w / 8).max(1);
        let max_lines = (text_h / LINE_HEIGHT).max(1);

        let total_lines = wrapped_line_count(&self.text[..self.text_len], max_cols);
        let max_scroll = total_lines.saturating_sub(max_lines);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }

        writer.set_color(text_color);
        if self.text_len == 0 {
            writer.set_color(detail);
            writer.set_pos(text_x, text_y + 2);
            writer.write_bytes(b"(empty)");
        } else {
            draw_wrapped_lines(
                &mut writer,
                &self.text[..self.text_len],
                self.scroll,
                max_lines,
                max_cols,
                text_x,
                text_y,
            );
        }

        let scroll_x = chrome.content_x + chrome.content_w.saturating_sub(PAD + SCROLL_W);
        let scroll_y = chrome.content_y + PAD;
        let scroll_h = chrome.content_h.saturating_sub(PAD * 2);

        display::fill_rect(
            fb,
            scroll_x,
            scroll_y,
            SCROLL_W,
            scroll_h,
            if is_dark { 0x00323232 } else { 0x00E1EAF5 },
        );
        fill_vertical_gradient(
            fb,
            scroll_x,
            scroll_y,
            SCROLL_W,
            BTN_H,
            if is_dark { 0x00484848 } else { 0x00D8E2EE },
            if is_dark { 0x003E3E3E } else { 0x00CBD8E8 },
        );
        fill_vertical_gradient(
            fb,
            scroll_x,
            scroll_y + scroll_h.saturating_sub(BTN_H),
            SCROLL_W,
            BTN_H,
            if is_dark { 0x00484848 } else { 0x00D8E2EE },
            if is_dark { 0x003E3E3E } else { 0x00CBD8E8 },
        );
        writer.set_color(detail);
        writer.set_pos(scroll_x + 4, scroll_y + 3);
        writer.write_bytes(b"^");
        writer.set_pos(scroll_x + 4, scroll_y + scroll_h.saturating_sub(BTN_H - 3));
        writer.write_bytes(b"v");

        if max_scroll > 0 {
            let track_h = scroll_h.saturating_sub(BTN_H * 2);
            let thumb_h = ((max_lines as f32 / total_lines as f32) * track_h as f32) as usize;
            let thumb_h = thumb_h.max(16).min(track_h.max(16));
            let thumb_y = scroll_y
                + BTN_H
                + ((self.scroll as f32 / max_scroll as f32) * (track_h.saturating_sub(thumb_h)) as f32)
                    as usize;
            display::fill_rect(
                fb,
                scroll_x,
                thumb_y,
                SCROLL_W,
                thumb_h,
                if is_dark { 0x00777F8A } else { 0x0093A9C0 },
            );
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

    pub fn set_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        self.win_x = x;
        self.win_y = y;
        self.win_w = w;
        self.win_h = h;
    }

    fn push_byte(&mut self, b: u8) {
        if self.text_len < MAX_TEXT {
            self.text[self.text_len] = b;
            self.text_len += 1;
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    fn scroll_down(&mut self, fb: &Framebuffer) {
        let (_, max_lines) = self.layout_metrics(fb);
        let total = wrapped_line_count(&self.text[..self.text_len], max_lines.0);
        let max_scroll = total.saturating_sub(max_lines.1);
        if self.scroll < max_scroll {
            self.scroll += 1;
        }
    }

    fn set_title(&mut self, file_name: &[u8]) {
        let prefix = b"Notepad - ";
        self.title_len = 0;
        for &b in prefix {
            if self.title_len >= MAX_TITLE {
                break;
            }
            self.title[self.title_len] = b;
            self.title_len += 1;
        }
        for &b in file_name {
            if self.title_len >= MAX_TITLE {
                break;
            }
            self.title[self.title_len] = b;
            self.title_len += 1;
        }
        if self.title_len == prefix.len() {
            self.title[0..7].copy_from_slice(b"Notepad");
            self.title_len = 7;
        }
    }

    fn layout_metrics(&self, fb: &Framebuffer) -> ((usize, usize), (usize, usize)) {
        let (_, _, w, h) = self.rect(fb);
        let content_w = w.saturating_sub(4);
        let content_h = h.saturating_sub(window::HEADER_H + 4);
        let text_w = content_w.saturating_sub(PAD * 2 + SCROLL_W + 4);
        let text_h = content_h.saturating_sub(PAD * 2);
        let max_cols = (text_w / 8).max(1);
        let max_lines = (text_h / LINE_HEIGHT).max(1);
        ((text_w, text_h), (max_cols, max_lines))
    }
}

fn sanitize_loaded_text(buf: &mut [u8], len: usize) -> usize {
    let mut out = 0usize;
    let max = len.min(buf.len());
    for i in 0..max {
        let b = buf[i];
        if b == b'\r' {
            continue;
        }
        let mapped = if b == b'\n' || b == b'\t' || (32..=126).contains(&b) {
            b
        } else {
            b'.'
        };
        if out < buf.len() {
            buf[out] = mapped;
            out += 1;
        } else {
            break;
        }
    }
    out
}

fn wrapped_line_count(data: &[u8], max_cols: usize) -> usize {
    if max_cols == 0 {
        return 0;
    }
    if data.is_empty() {
        return 1;
    }

    let mut lines = 1usize;
    let mut col = 0usize;
    for &b in data {
        if b == b'\r' {
            continue;
        }
        if b == b'\n' {
            lines += 1;
            col = 0;
            continue;
        }
        if col >= max_cols {
            lines += 1;
            col = 0;
        }
        col += 1;
    }
    lines
}

fn draw_wrapped_lines(
    writer: &mut crate::TextWriter,
    data: &[u8],
    start_line: usize,
    max_lines: usize,
    max_cols: usize,
    x: usize,
    y: usize,
) {
    if max_cols == 0 || max_lines == 0 {
        return;
    }

    let mut line_buf = [0u8; 256];
    let mut line_len = 0usize;
    let mut logical_line = 0usize;
    let mut drawn = 0usize;

    let mut flush = |buf: &[u8], len: usize, logical_line: &mut usize, drawn: &mut usize| {
        if *logical_line >= start_line && *drawn < max_lines {
            writer.set_pos(x, y + *drawn * LINE_HEIGHT);
            if len > 0 {
                writer.write_bytes(&buf[..len]);
            }
            *drawn += 1;
        }
        *logical_line += 1;
    };

    for &b in data {
        if b == b'\r' {
            continue;
        }
        if b == b'\n' {
            flush(&line_buf, line_len, &mut logical_line, &mut drawn);
            line_len = 0;
            if drawn >= max_lines {
                return;
            }
            continue;
        }

        if line_len >= max_cols || line_len >= line_buf.len() {
            flush(&line_buf, line_len, &mut logical_line, &mut drawn);
            line_len = 0;
            if drawn >= max_lines {
                return;
            }
        }

        if line_len < line_buf.len() {
            line_buf[line_len] = b;
            line_len += 1;
        }
    }

    if drawn < max_lines {
        flush(&line_buf, line_len, &mut logical_line, &mut drawn);
    }
}

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let w = (fb.width * 3 / 4).min(840).max(420);
    let h = (fb.height * 3 / 4).min(560).max(320);
    let x = (fb.width.saturating_sub(w)) / 2;
    let y = (fb.height.saturating_sub(h)) / 2;
    (x, y, w, h)
}

fn hit(px: usize, py: usize, x: usize, y: usize, w: usize, h: usize) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
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

fn lerp_rgb(a: u32, b: u32, num: u32, den: u32) -> u32 {
    if den == 0 {
        return a;
    }
    let ar = (a >> 16) & 0xFF;
    let ag = (a >> 8) & 0xFF;
    let ab = a & 0xFF;
    let br = (b >> 16) & 0xFF;
    let bg = (b >> 8) & 0xFF;
    let bb = b & 0xFF;
    let r = (ar * (den - num) + br * num) / den;
    let g = (ag * (den - num) + bg * num) / den;
    let b = (ab * (den - num) + bb * num) / den;
    (r << 16) | (g << 8) | b
}
