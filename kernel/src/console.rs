use crate::clipboard;
use crate::commands::{self, ConsoleAction, LineType};
use crate::display::{self, Framebuffer};
use crate::system;
use crate::window;

const MAX_LINES: usize = 32;
const MAX_COLS: usize = 64;
const LINE_HEIGHT: usize = 16; // Збільшено для кращої читаємості
const PAD: usize = 14;

pub struct Console {
    visible: bool,
    fb_w: usize,
    fb_h: usize,
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,
    lines: [[u8; MAX_COLS]; MAX_LINES],
    lens: [usize; MAX_LINES],
    types: [LineType; MAX_LINES],
    count: usize,
    input: [u8; MAX_COLS],
    input_len: usize,
    action: ConsoleAction,
    rand_state: u32,
}

impl Console {
    pub fn new(fb: Framebuffer) -> Self {
        let seed = (fb.width as u32) ^ ((fb.height as u32) << 16);
        Self {
            visible: false, fb_w: fb.width, fb_h: fb.height,
            win_x: 0, win_y: 0, win_w: 0, win_h: 0,
            lines: [[0u8; MAX_COLS]; MAX_LINES],
            lens: [0usize; MAX_LINES],
            types: [LineType::Normal; MAX_LINES],
            count: 0, input: [0u8; MAX_COLS], input_len: 0,
            action: ConsoleAction::None,
            rand_state: seed ^ 0xA5A5_5A5A,
        }
    }

    pub fn is_visible(&self) -> bool { self.visible }

    pub fn show(&mut self, fb: &Framebuffer) {
        self.visible = true;
        if self.win_w == 0 {
            let (x, y, w, h) = calc_rect(fb);
            (self.win_x, self.win_y, self.win_w, self.win_h) = (x, y, w, h);
        }
    }

    pub fn hide(&mut self, _fb: &Framebuffer) { self.visible = false; }

    pub fn handle_click(&mut self, _fb: &Framebuffer, _x: usize, _y: usize) -> bool { self.visible }

    pub fn take_action(&mut self) -> ConsoleAction {
        core::mem::replace(&mut self.action, ConsoleAction::None)
    }

    pub fn copy_input(&self) {
        if self.visible && self.input_len > 0 {
            clipboard::set(&self.input[..self.input_len]);
        }
    }

    pub fn paste_clipboard(&mut self, fb: &Framebuffer) -> bool {
        if !self.visible { return false; }
        let data = clipboard::data();
        for &b in data.iter().filter(|&&b| b != b'\n' && b != b'\r') {
            if self.input_len >= MAX_COLS { break; }
            self.input[self.input_len] = b;
            self.input_len += 1;
        }
        self.redraw(fb);
        true
    }

    pub fn handle_char(&mut self, fb: &Framebuffer, ch: u8) -> bool {
        if !self.visible { return false; }
        match ch {
            b'\n' => self.execute(fb),
            0x08 => if self.input_len > 0 { self.input_len -= 1; self.redraw(fb); },
            b'\t' => {},
            _ if self.input_len < MAX_COLS => {
                self.input[self.input_len] = ch;
                self.input_len += 1;
                self.redraw(fb);
            }
            _ => {}
        }
        true
    }

    fn execute(&mut self, fb: &Framebuffer) {
        let len = self.input_len;
        let cmd = self.input;
        
        self.push_prompt_line(&cmd, len);
        
        let result = commands::execute_command(&cmd, len, &mut self.rand_state, self.fb_w, self.fb_h);
        
        let (head, _) = split_first_word(&cmd, len);
        if eq_ignore_case(head, b"clear") || eq_ignore_case(head, b"cls") {
            self.clear();
        } else {
            for i in 0..result.count {
                self.push_line_raw(&result.lines[i], result.lens[i], result.types[i]);
            }
        }
        
        if result.action != ConsoleAction::None { self.action = result.action; }
        self.input_len = 0;
        self.redraw(fb);
    }

    fn push_prompt_line(&mut self, cmd: &[u8; MAX_COLS], len: usize) {
        let mut line = [0u8; MAX_COLS];
        line[0] = b'>';
        line[1] = b' ';
        let content_len = (len).min(MAX_COLS - 2);
        line[2..2 + content_len].copy_from_slice(&cmd[..content_len]);
        self.push_line_raw(&line, content_len + 2, LineType::Info);
    }

    fn clear(&mut self) {
        self.count = 0;
        self.lens.fill(0);
    }

    fn push_line_raw(&mut self, line: &[u8; MAX_COLS], len: usize, line_type: LineType) {
        if self.count < MAX_LINES {
            self.lines[self.count] = *line;
            self.lens[self.count] = len;
            self.types[self.count] = line_type;
            self.count += 1;
        } else {
            for i in 1..MAX_LINES {
                self.lines[i - 1] = self.lines[i];
                self.lens[i - 1] = self.lens[i];
                self.types[i - 1] = self.types[i];
            }
            self.lines[MAX_LINES - 1] = *line;
            self.lens[MAX_LINES - 1] = len;
            self.types[MAX_LINES - 1] = line_type;
        }
    }

    pub fn redraw(&self, fb: &Framebuffer) {
        if !self.visible { return; }
        
        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        
        // Покращена кольорова палітра з легкою прозорістю (якщо підтримується)
        let (bg, input_bg, border, prompt) = if ui.dark {
            (0xEE1A1A1A, 0x00252525, 0x00333333, ui.accent)
        } else {
            (0xEEFDFDFD, 0x00F0F0F0, 0x00D0D0D0, ui.accent)
        };
        
        let chrome = window::draw_window(fb, x, y, w, h, b"Terminal");
        display::fill_rect(fb, chrome.content_x, chrome.content_y, chrome.content_w, chrome.content_h, bg);

        let text_x = chrome.content_x + PAD;
        let input_h = LINE_HEIGHT + 10;
        let input_area_y = chrome.content_y + chrome.content_h - input_h - 6;
        
        // Вивід історії
        let max_visible = (input_area_y - chrome.content_y - PAD) / LINE_HEIGHT;
        let start = self.count.saturating_sub(max_visible);
        let mut writer = crate::TextWriter::new(*fb);

        for i in 0..(self.count - start) {
            let idx = start + i;
            writer.set_color(get_line_color(self.types[idx], ui.dark));
            writer.set_pos(text_x, chrome.content_y + PAD + i * LINE_HEIGHT);
            writer.write_bytes(&self.lines[idx][..self.lens[idx]]);
        }

        // Поле вводу (Styled)
        let input_box_x = chrome.content_x + 6;
        let input_box_w = chrome.content_w - 12;
        display::fill_rect(fb, input_box_x, input_area_y, input_box_w, input_h, input_bg);
        draw_border(fb, input_box_x, input_area_y, input_box_w, input_h, border);

        // Текст вводу
        let input_text_y = input_area_y + (input_h - 10) / 2;
        writer.set_color(prompt);
        writer.set_pos(text_x, input_text_y);
        writer.write_bytes(b"> ");
        
        writer.set_color(if ui.dark { 0x00FFFFFF } else { 0x000000 });
        writer.write_bytes(&self.input[..self.input_len]);

        // Активний курсор
        let cursor_x = text_x + 18 + (self.input_len * 8);
        if cursor_x < input_box_x + input_box_w - 4 {
            display::fill_rect(fb, cursor_x, input_text_y, 2, 12, prompt);
        }
    }

    pub fn rect(&self, fb: &Framebuffer) -> (usize, usize, usize, usize) {
        if self.win_w == 0 { calc_rect(fb) } else { (self.win_x, self.win_y, self.win_w, self.win_h) }
    }

    pub fn set_pos(&mut self, x: usize, y: usize) { (self.win_x, self.win_y) = (x, y); }
}

// --- Helpers (Без змін логіки, лише чистіший вигляд) ---

fn get_line_color(t: LineType, dark: bool) -> u32 {
    match (t, dark) {
        (LineType::Success, true) => 0x0088FF88,
        (LineType::Success, false) => 0x0000AA00,
        (LineType::Error, true) => 0x00FF7777,
        (LineType::Error, false) => 0x00AA0000,
        (LineType::Info, true) => 0x0077CCFF,
        (LineType::Info, false) => 0x000066CC,
        (_, true) => 0x00DDDDDD,
        (_, false) => 0x00222222,
    }
}

fn draw_border(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize, color: u32) {
    display::fill_rect(fb, x, y, w, 1, color);
    display::fill_rect(fb, x, y + h - 1, w, 1, color);
    display::fill_rect(fb, x, y, 1, h, color);
    display::fill_rect(fb, x + w - 1, y, 1, h, color);
}

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let w = (fb.width * 6) / 10;
    let h = (fb.height * 6) / 10;
    ((fb.width - w) / 2, (fb.height - h) / 2, w, h)
}

fn split_first_word(buf: &[u8], len: usize) -> (&[u8], &[u8]) {
    let s = buf[..len].iter().position(|&b| b != b' ').unwrap_or(len);
    let e = buf[s..len].iter().position(|&b| b == b' ').map(|p| p + s).unwrap_or(len);
    let next = buf[e..len].iter().position(|&b| b != b' ').map(|p| p + e).unwrap_or(len);
    (&buf[s..e], &buf[next..len])
}

fn eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&ac, &bc)| ac.to_ascii_lowercase() == bc.to_ascii_lowercase())
}