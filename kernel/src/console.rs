use crate::clipboard;
use crate::commands::{self, ConsoleAction, LineType};
use crate::display::{self, Framebuffer};
use crate::system;
use crate::window;

const MAX_LINES: usize = 32;
const MAX_COLS: usize = 64;
const LINE_HEIGHT: usize = 14;
const PAD: usize = 12;

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
            visible: false,
            fb_w: fb.width,
            fb_h: fb.height,
            win_x: 0,
            win_y: 0,
            win_w: 0,
            win_h: 0,
            lines: [[0u8; MAX_COLS]; MAX_LINES],
            lens: [0usize; MAX_LINES],
            types: [LineType::Normal; MAX_LINES],
            count: 0,
            input: [0u8; MAX_COLS],
            input_len: 0,
            action: ConsoleAction::None,
            rand_state: seed ^ 0xA5A5_5A5A,
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

    pub fn hide(&mut self, _fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        self.visible = false;
    }

    pub fn handle_click(&mut self, _fb: &Framebuffer, _x: usize, _y: usize) -> bool {
        if !self.visible {
            return false;
        }
        false
    }

    pub fn take_action(&mut self) -> ConsoleAction {
        let act = self.action;
        self.action = ConsoleAction::None;
        act
    }

    pub fn copy_input(&self) {
        if !self.visible {
            return;
        }
        if self.input_len == 0 {
            return;
        }
        clipboard::set(&self.input[..self.input_len]);
    }

    pub fn paste_clipboard(&mut self, fb: &Framebuffer) -> bool {
        if !self.visible {
            return false;
        }
        let data = clipboard::data();
        if data.is_empty() {
            return false;
        }
        for &b in data {
            if b == b'\n' || b == b'\r' {
                continue;
            }
            if self.input_len >= MAX_COLS {
                break;
            }
            self.input[self.input_len] = b;
            self.input_len += 1;
        }
        self.redraw(fb);
        true
    }

    pub fn handle_char(&mut self, fb: &Framebuffer, ch: u8) -> bool {
        if !self.visible {
            return false;
        }
        match ch {
            b'\n' => {
                let mut cmd = [0u8; MAX_COLS];
                let mut len = 0usize;
                for i in 0..self.input_len {
                    if len >= MAX_COLS {
                        break;
                    }
                    cmd[len] = self.input[i];
                    len += 1;
                }
                
                // Показуємо введену команду
                self.push_prompt_line(&cmd, len);
                
                // Виконуємо команду
                let result = commands::execute_command(&cmd, len, &mut self.rand_state, self.fb_w, self.fb_h);
                
                // Обробляємо команду clear окремо
                let (head, _) = split_first_word(&cmd, len);
                if eq_ignore_case(head, b"clear") || eq_ignore_case(head, b"cls") {
                    self.clear();
                } else {
                    // Додаємо результати виконання
                    for i in 0..result.count {
                        self.push_line_raw(&result.lines[i], result.lens[i], result.types[i]);
                    }
                }
                
                // Зберігаємо action
                if result.action != ConsoleAction::None {
                    self.action = result.action;
                }
                
                self.input_len = 0;
                self.redraw(fb);
                true
            }
            0x08 => {
                if self.input_len > 0 {
                    self.input_len -= 1;
                    self.redraw(fb);
                }
                true
            }
            b'\t' => true,
            _ => {
                if self.input_len < MAX_COLS {
                    self.input[self.input_len] = ch;
                    self.input_len += 1;
                    self.redraw(fb);
                }
                true
            }
        }
    }

    fn push_prompt_line(&mut self, cmd: &[u8; MAX_COLS], len: usize) {
        let mut line = [0u8; MAX_COLS];
        let mut out_len = 0usize;
        
        // Символ промпту
        if out_len < MAX_COLS {
            line[out_len] = b'>';
            out_len += 1;
        }
        if out_len < MAX_COLS {
            line[out_len] = b' ';
            out_len += 1;
        }
        
        // Сама команда
        for i in 0..len {
            if out_len >= MAX_COLS {
                break;
            }
            line[out_len] = cmd[i];
            out_len += 1;
        }
        
        self.push_line_raw(&line, out_len, LineType::Info);
    }

    fn clear(&mut self) {
        self.count = 0;
        for i in 0..MAX_LINES {
            self.lens[i] = 0;
        }
    }

    fn push_line_raw(&mut self, line: &[u8; MAX_COLS], len: usize, line_type: LineType) {
        if self.count < MAX_LINES {
            self.lines[self.count] = *line;
            self.lens[self.count] = len;
            self.types[self.count] = line_type;
            self.count += 1;
            return;
        }
        // Scroll up
        for i in 1..MAX_LINES {
            self.lines[i - 1] = self.lines[i];
            self.lens[i - 1] = self.lens[i];
            self.types[i - 1] = self.types[i];
        }
        self.lines[MAX_LINES - 1] = *line;
        self.lens[MAX_LINES - 1] = len;
        self.types[MAX_LINES - 1] = line_type;
    }

    pub fn redraw(&self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        
        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        
        // Кольорова схема
        let (bg, input_bg, border_color, prompt_color) = if ui.dark {
            (0x001E1E1E, 0x002D2D2D, 0x00404040, 0x0066B3FF)
        } else {
            (0x00F5F5F5, 0x00FFFFFF, 0x00CCCCCC, 0x002196F3)
        };
        
        // Малюємо вікно з рамкою
        let chrome = window::draw_window(fb, x, y, w, h, b"Terminal");
        
        // Фон вмісту
        display::fill_rect(
            fb,
            chrome.content_x,
            chrome.content_y,
            chrome.content_w,
            chrome.content_h,
            bg,
        );
        
        // Рамка навколо вмісту
        draw_border(fb, chrome.content_x, chrome.content_y, chrome.content_w, chrome.content_h, border_color);

        let text_x = chrome.content_x + PAD;
        let text_y = chrome.content_y + PAD;
        let max_visible = (chrome.content_h.saturating_sub(PAD * 3 + LINE_HEIGHT + 8)) / LINE_HEIGHT;
        let visible_lines = max_visible;
        
        let start = if self.count > visible_lines {
            self.count - visible_lines
        } else {
            0
        };

        let mut writer = crate::TextWriter::new(*fb);

        // Виводимо історію команд
        let mut row = 0usize;
        for i in start..self.count {
            let color = get_line_color(self.types[i], ui.dark);
            writer.set_color(color);
            writer.set_pos(text_x, text_y + row * LINE_HEIGHT);
            let len = self.lens[i];
            if len > 0 {
                writer.write_bytes(&self.lines[i][..len]);
            }
            row += 1;
        }

        // Поле вводу з фоном
        let input_y = chrome.content_y + chrome.content_h.saturating_sub(LINE_HEIGHT + PAD + 8);
        let input_h = LINE_HEIGHT + 12;
        
        display::fill_rect(
            fb,
            chrome.content_x + 4,
            input_y.saturating_sub(6),
            chrome.content_w.saturating_sub(8),
            input_h,
            input_bg,
        );
        
        // Рамка навколо поля вводу
        draw_border(
            fb,
            chrome.content_x + 4,
            input_y.saturating_sub(6),
            chrome.content_w.saturating_sub(8),
            input_h,
            prompt_color,
        );
        
        // Промпт і введений текст
        writer.set_color(prompt_color);
        writer.set_pos(text_x, input_y);
        writer.write_bytes(b"> ");
        
        let text_color = if ui.dark { 0x00E6E6E6 } else { 0x00212121 };
        writer.set_color(text_color);
        
        if self.input_len > 0 {
            writer.write_bytes(&self.input[..self.input_len]);
        }
        
        // Курсор миготіння (просто вертикальна лінія)
        let cursor_x = text_x + 16 + (self.input_len * 8);
        if cursor_x < chrome.content_x + chrome.content_w - PAD {
            display::fill_rect(fb, cursor_x, input_y + 1, 2, LINE_HEIGHT - 2, prompt_color);
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

fn get_line_color(line_type: LineType, is_dark: bool) -> u32 {
    match line_type {
        LineType::Success => {
            if is_dark {
                0x0066FF66  // Яскраво-зелений для темної теми
            } else {
                0x00008800  // Темно-зелений для світлої теми
            }
        }
        LineType::Error => {
            if is_dark {
                0x00FF6666  // Яскраво-червоний для темної теми
            } else {
                0x00CC0000  // Темно-червоний для світлої теми
            }
        }
        LineType::Info => {
            if is_dark {
                0x0066B3FF  // Яскраво-синій для темної теми
            } else {
                0x002196F3  // Синій для світлої теми
            }
        }
        LineType::Normal => {
            if is_dark {
                0x00E6E6E6  // Світлий текст для темної теми
            } else {
                0x00212121  // Темний текст для світлої теми
            }
        }
    }
}

fn draw_border(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize, color: u32) {
    // Верхня лінія
    display::fill_rect(fb, x, y, w, 1, color);
    // Нижня лінія
    display::fill_rect(fb, x, y + h.saturating_sub(1), w, 1, color);
    // Ліва лінія
    display::fill_rect(fb, x, y, 1, h, color);
    // Права лінія
    display::fill_rect(fb, x + w.saturating_sub(1), y, 1, h, color);
}

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let w = (fb.width * 3) / 5;  // 60% ширини екрану
    let h = (fb.height * 2) / 3; // 66% висоти екрану
    if w == 0 || h == 0 {
        return (0, 0, 0, 0);
    }
    let x = (fb.width - w) / 2;
    let y = (fb.height - h) / 2;
    (x, y, w, h)
}

fn split_first_word(buf: &[u8], len: usize) -> (&[u8], &[u8]) {
    let mut i = 0;
    while i < len && buf[i] == b' ' {
        i += 1;
    }
    let start = i;
    while i < len && buf[i] != b' ' {
        i += 1;
    }
    let end = i;
    while i < len && buf[i] == b' ' {
        i += 1;
    }
    (&buf[start..end], &buf[i..len])
}

fn eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for i in 0..a.len() {
        let mut ca = a[i];
        let mut cb = b[i];
        if ca >= b'A' && ca <= b'Z' {
            ca = ca + 32;
        }
        if cb >= b'A' && cb <= b'Z' {
            cb = cb + 32;
        }
        if ca != cb {
            return false;
        }
    }
    true
}