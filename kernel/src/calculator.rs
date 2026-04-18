// calculator.rs
use crate::display::{self, Framebuffer};
use crate::system;
use crate::window;

const WIN_WIDTH: usize = 320;
const WIN_HEIGHT: usize = 460;
const PAD: usize = 12;
const DISPLAY_H: usize = 72;
const BUTTON_GAP: usize = 8;
const BUTTON_ROWS: usize = 5;
const BUTTON_COLS: usize = 4;

const MAX_DISPLAY_LEN: usize = 32;
const SCALE: i64 = 1_000_000; // Фіксована кома: 6 знаків

#[derive(Copy, Clone, PartialEq)]
enum Op {
    None,
    Add,
    Sub,
    Mul,
    Div,
}

pub struct Calculator {
    visible: bool,
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,

    display_text: [u8; MAX_DISPLAY_LEN],
    display_len: usize,

    left: i64,      // Акумулятор (вже масштабований)
    right: i64,     // Поточне введене число (ціле або дробове без масштабу)
    current_op: Op,
    has_op: bool,
    last_was_eq: bool,
    has_decimal: bool,
    decimal_places: u32,
}

impl Calculator {
    pub fn new(fb: &Framebuffer) -> Self {
        let (x, y) = centered_rect(fb, WIN_WIDTH, WIN_HEIGHT);
        let mut s = Self {
            visible: false,
            win_x: x,
            win_y: y,
            win_w: WIN_WIDTH,
            win_h: WIN_HEIGHT,
            display_text: [0; MAX_DISPLAY_LEN],
            display_len: 1,
            left: 0,
            right: 0,
            current_op: Op::None,
            has_op: false,
            last_was_eq: false,
            has_decimal: false,
            decimal_places: 0,
        };
        s.display_text[0] = b'0';
        s
    }

    pub fn is_visible(&self) -> bool { self.visible }
    pub fn show(&mut self) { self.visible = true; self.clear(); }
    pub fn hide(&mut self) { self.visible = false; }

    pub fn handle_click(&mut self, fb: &Framebuffer, x: usize, y: usize) {
        if !self.visible { return; }
        let (wx, wy, ww, _) = self.rect(fb);
        let bx_start = wx + PAD;
        let by_start = wy + window::HEADER_H + PAD + DISPLAY_H + PAD;
        let bw = (ww - 2 * PAD - 3 * BUTTON_GAP) / BUTTON_COLS;
        let bh = (WIN_HEIGHT - window::HEADER_H - 3 * PAD - DISPLAY_H - 4 * BUTTON_GAP) / BUTTON_ROWS;

        if x < bx_start || y < by_start { return; }
        let col = (x - bx_start) / (bw + BUTTON_GAP);
        let row = (y - by_start) / (bh + BUTTON_GAP);
        if col >= BUTTON_COLS || row >= BUTTON_ROWS { return; }

        let key = match (row, col) {
            (0, 0) => b'C', (0, 1) => b'<', (0, 2) => b'%', (0, 3) => b'/',
            (1, 0) => b'7', (1, 1) => b'8', (1, 2) => b'9', (1, 3) => b'*',
            (2, 0) => b'4', (2, 1) => b'5', (2, 2) => b'6', (2, 3) => b'-',
            (3, 0) => b'1', (3, 1) => b'2', (3, 2) => b'3', (3, 3) => b'+',
            (4, 0) => b'.', (4, 1) => b'0', (4, 2) => b'=', (4, 3) => b'+',
            _ => return,
        };
        self.process_key(key);
    }

    pub fn handle_char(&mut self, _fb: &Framebuffer, ch: u8) {
        if !self.visible { return; }
        let key = match ch {
            b'\n' | b'=' => b'=',
            0x08 => b'<', // Backspace
            b'c' | b'C' => b'C',
            _ => ch,
        };
        self.process_key(key);
    }

    fn process_key(&mut self, key: u8) {
        match key {
            b'0'..=b'9' => self.input_digit(key),
            b'.' => self.input_decimal(),
            b'C' => self.clear(),
            b'<' => self.backspace(),
            b'+' | b'-' | b'*' | b'/' => self.set_operator(key),
            b'=' => self.calculate(),
            b'%' => self.percent(),
            _ => {}
        }
    }

    fn input_digit(&mut self, digit: u8) {
        if self.last_was_eq { self.clear(); }
        let d = (digit - b'0') as i64;
        
        if self.has_decimal {
            if self.decimal_places < 6 {
                self.right = self.right * 10 + d;
                self.decimal_places += 1;
            } else { return; }
        } else {
            if self.right == 0 && self.display_len == 1 && self.display_text[0] == b'0' {
                self.right = d;
            } else {
                self.right = self.right * 10 + d;
            }
        }
        self.update_display_from_input();
    }

    fn update_display_from_input(&mut self) {
        // Просте відображення того, що вводить користувач
        let mut temp = self.right;
        let mut buf = [0u8; 20];
        let mut i = 0;
        if temp == 0 { buf[0] = b'0'; i = 1; }
        while temp > 0 { buf[i] = (temp % 10) as u8 + b'0'; temp /= 10; i += 1; }
        
        self.display_len = 0;
        if self.has_decimal && self.decimal_places == 0 {
            // Випадок коли натиснули тільки точку "0."
            for j in (0..i).rev() { self.display_text[self.display_len] = buf[j]; self.display_len += 1; }
            self.display_text[self.display_len] = b'.';
            self.display_len += 1;
        } else if self.has_decimal {
            // Відображаємо число з крапкою в правильній позиції
            let int_digits = i.saturating_sub(self.decimal_places as usize);
            if int_digits == 0 {
                self.display_text[0] = b'0'; self.display_text[1] = b'.'; self.display_len = 2;
                for _ in 0..(self.decimal_places as usize - i) { self.display_text[self.display_len] = b'0'; self.display_len += 1; }
            }
            for j in (0..i).rev() {
                self.display_text[self.display_len] = buf[j];
                self.display_len += 1;
                if j == self.decimal_places as usize && j != 0 {
                    self.display_text[self.display_len] = b'.';
                    self.display_len += 1;
                }
            }
        } else {
            for j in (0..i).rev() { self.display_text[self.display_len] = buf[j]; self.display_len += 1; }
        }
    }

    fn input_decimal(&mut self) {
        if self.last_was_eq { self.clear(); }
        if !self.has_decimal {
            self.has_decimal = true;
            self.decimal_places = 0;
            self.update_display_from_input();
        }
    }

    fn set_operator(&mut self, op_char: u8) {
        if !self.has_op {
            self.left = self.get_scaled_right();
        } else {
            self.apply_pending_operation();
        }
        self.current_op = match op_char {
            b'+' => Op::Add, b'-' => Op::Sub, b'*' => Op::Mul, b'/' => Op::Div,
            _ => Op::None,
        };
        self.has_op = true;
        self.right = 0;
        self.has_decimal = false;
        self.decimal_places = 0;
        self.last_was_eq = false;
    }

    fn get_scaled_right(&self) -> i64 {
        let mut val = self.right;
        if !self.has_decimal {
            val * SCALE
        } else {
            for _ in 0..(6 - self.decimal_places) { val *= 10; }
            val
        }
    }

    fn calculate(&mut self) {
        if self.has_op {
            self.apply_pending_operation();
            self.has_op = false;
            self.current_op = Op::None;
            self.last_was_eq = true;
            // Результат вже в self.left, right готуємо до нового вводу
            self.right = 0; 
            self.has_decimal = false;
        }
    }

    fn apply_pending_operation(&mut self) {
        let r_val = self.get_scaled_right();
        match self.current_op {
            Op::Add => self.left = self.left.saturating_add(r_val),
            Op::Sub => self.left = self.left.saturating_sub(r_val),
            Op::Mul => {
                let res = (self.left as i128 * r_val as i128) / SCALE as i128;
                self.left = res as i64;
            }
            Op::Div => {
                if r_val != 0 {
                    let res = (self.left as i128 * SCALE as i128) / r_val as i128;
                    self.left = res as i64;
                }
            }
            Op::None => self.left = r_val,
        }
        self.format_display_from_left();
    }

    fn format_display_from_left(&mut self) {
        let mut val = self.left;
        self.display_len = 0;
        if val < 0 { self.display_text[0] = b'-'; self.display_len = 1; val = -val; }

        let int_part = val / SCALE;
        let frac_part = val % SCALE;

        // Ціла частина
        let mut temp = int_part;
        let mut buf = [0u8; 20];
        let mut i = 0;
        if temp == 0 { buf[0] = b'0'; i = 1; }
        while temp > 0 { buf[i] = (temp % 10) as u8 + b'0'; temp /= 10; i += 1; }
        for j in (0..i).rev() { self.display_text[self.display_len] = buf[j]; self.display_len += 1; }

        // Дробова частина
        if frac_part > 0 {
            self.display_text[self.display_len] = b'.';
            self.display_len += 1;
            let mut f = frac_part;
            let mut f_buf = [0u8; 6];
            for j in (0..6).rev() { f_buf[j] = (f % 10) as u8 + b'0'; f /= 10; }
            for j in 0..6 { self.display_text[self.display_len] = f_buf[j]; self.display_len += 1; }
            // Видаляємо зайві нулі в кінці
            while self.display_len > 0 && self.display_text[self.display_len - 1] == b'0' { self.display_len -= 1; }
            if self.display_len > 0 && self.display_text[self.display_len - 1] == b'.' { self.display_len -= 1; }
        }
    }

    fn clear(&mut self) {
        self.display_text[0] = b'0'; self.display_len = 1;
        self.left = 0; self.right = 0;
        self.current_op = Op::None; self.has_op = false;
        self.has_decimal = false; self.decimal_places = 0;
        self.last_was_eq = false;
    }

    fn backspace(&mut self) {
        if self.last_was_eq { self.clear(); return; }
        if self.has_decimal {
            if self.decimal_places > 0 { self.right /= 10; self.decimal_places -= 1; }
            else { self.has_decimal = false; }
        } else {
            self.right /= 10;
        }
        self.update_display_from_input();
        if self.display_len == 0 { self.display_text[0] = b'0'; self.display_len = 1; }
    }

    fn percent(&mut self) {
        self.left = (self.left * SCALE) / 100;
        self.format_display_from_left();
    }

    pub fn redraw(&self, fb: &Framebuffer) {
        if !self.visible { return; }
        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        let bg = if ui.dark { 0x00212121 } else { 0x00FFFFFF };
        let accent = ui.accent;

        let chrome = window::draw_window(fb, x, y, w, h, b"Calculator");
        display::fill_rect(fb, chrome.content_x, chrome.content_y, chrome.content_w, chrome.content_h, bg);

        let d_x = chrome.content_x + PAD;
        let d_y = chrome.content_y + PAD;
        let d_w = w - 2 * PAD;
        display::fill_rect(fb, d_x, d_y, d_w, DISPLAY_H, if ui.dark { 0x1A1A1A } else { 0xF5F5F5 });

        let mut writer = crate::TextWriter::new(*fb);
        writer.set_color(if ui.dark { 0xF0F0F0 } else { 0x111111 });
        let txt = &self.display_text[..self.display_len];
        writer.set_pos(d_x + d_w - txt.len() * 8 - 8, d_y + (DISPLAY_H - 10) / 2);
        writer.write_bytes(txt);

        let labels = [
            ["C", "<-", "%", "/"], ["7", "8", "9", "*"],
            ["4", "5", "6", "-"], ["1", "2", "3", "+"],
            [".", "0", "=", "+"]
        ];

        let bw = (w - 2 * PAD - 3 * BUTTON_GAP) / BUTTON_COLS;
        let bh = (h - window::HEADER_H - 3 * PAD - DISPLAY_H - 4 * BUTTON_GAP) / BUTTON_ROWS;

        for r in 0..BUTTON_ROWS {
            for c in 0..BUTTON_COLS {
                let bx = d_x + c * (bw + BUTTON_GAP);
                let by = d_y + DISPLAY_H + PAD + r * (bh + BUTTON_GAP);
                display::fill_rect(fb, bx, by, bw, bh, if ui.dark { 0x333333 } else { 0xE0E0E0 });
                let l = labels[r][c].as_bytes();
                writer.set_pos(bx + (bw - l.len() * 8) / 2, by + (bh - 10) / 2);
                writer.write_bytes(l);
            }
        }
    }

    pub fn rect(&self, _fb: &Framebuffer) -> (usize, usize, usize, usize) {
        (self.win_x, self.win_y, self.win_w, self.win_h)
    }

    pub fn set_pos(&mut self, x: usize, y: usize) {
        self.win_x = x; self.win_y = y;
    }
} // Кінець impl Calculator

fn centered_rect(fb: &Framebuffer, w: usize, h: usize) -> (usize, usize) {
    ((fb.width - w) / 2, (fb.height - h) / 2)
}