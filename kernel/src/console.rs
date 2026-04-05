use crate::clipboard;
use crate::display::{self, Framebuffer};
use crate::rtc;
use crate::system;
use crate::window;

const MAX_LINES: usize = 32;
const MAX_COLS: usize = 64;
const LINE_HEIGHT: usize = 12;
const PAD: usize = 10;

#[derive(Copy, Clone, PartialEq)]
pub enum ConsoleAction {
    None,
    OpenExplorer,
    OpenClipboard,
}

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
                self.push_prompt_line(&cmd, len);
                self.exec_command(&cmd, len);
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
        if out_len < MAX_COLS {
            line[out_len] = b'>';
            out_len += 1;
        }
        if out_len < MAX_COLS {
            line[out_len] = b' ';
            out_len += 1;
        }
        for i in 0..len {
            if out_len >= MAX_COLS {
                break;
            }
            line[out_len] = cmd[i];
            out_len += 1;
        }
        self.push_line_raw(&line, out_len);
    }

    fn exec_command(&mut self, cmd: &[u8; MAX_COLS], len: usize) {
        let (head, rest) = split_first_word(cmd, len);
        if head.len() == 0 {
            return;
        }

        if eq_ignore_case(head, b"help") {
            self.push_line(b"help  clear  cls  echo  ver");
            self.push_line(b"about  res  whoami  ping");
            self.push_line(b"time  date  mem  sysinfo");
            self.push_line(b"rand  explorer  dir  ls");
            self.push_line(b"clip  copy  paste");
            self.push_line(b"set  (system params)");
            self.push_line(b"Try: echo hello");
        } else if eq_ignore_case(head, b"clear") || eq_ignore_case(head, b"cls") {
            self.clear();
        } else if eq_ignore_case(head, b"echo") {
            self.push_line(rest);
        } else if eq_ignore_case(head, b"ver") || eq_ignore_case(head, b"version") {
            self.push_line(b"HouseOS 0.1");
        } else if eq_ignore_case(head, b"about") {
            self.push_line(b"HouseOS demo console");
        } else if eq_ignore_case(head, b"res") || eq_ignore_case(head, b"mode") {
            self.push_res();
        } else if eq_ignore_case(head, b"whoami") {
            self.push_line(b"root");
        } else if eq_ignore_case(head, b"ping") {
            self.push_line(b"pong");
        } else if eq_ignore_case(head, b"time") {
            self.push_time();
        } else if eq_ignore_case(head, b"date") {
            self.push_date();
        } else if eq_ignore_case(head, b"mem") {
            self.push_mem();
        } else if eq_ignore_case(head, b"sysinfo") {
            self.push_sysinfo();
        } else if eq_ignore_case(head, b"rand") {
            self.push_rand();
        } else if eq_ignore_case(head, b"set") {
            self.handle_set(rest);
        } else if eq_ignore_case(head, b"clip") || eq_ignore_case(head, b"clipboard") {
            self.push_line(b"Opening clipboard...");
            self.action = ConsoleAction::OpenClipboard;
        } else if eq_ignore_case(head, b"copy") {
            if rest.is_empty() {
                self.push_line(b"usage: copy <text>");
            } else {
                clipboard::set(rest);
                self.push_line(b"copied");
            }
        } else if eq_ignore_case(head, b"paste") {
            let data = clipboard::data();
            if data.is_empty() {
                self.push_line(b"(clipboard empty)");
            } else {
                self.push_line(data);
            }
        } else if eq_ignore_case(head, b"explorer")
            || eq_ignore_case(head, b"dir")
            || eq_ignore_case(head, b"ls")
        {
            self.push_line(b"Opening explorer...");
            self.action = ConsoleAction::OpenExplorer;
        } else {
            self.push_line(b"Unknown command");
        }
    }

    fn clear(&mut self) {
        self.count = 0;
        for i in 0..MAX_LINES {
            self.lens[i] = 0;
        }
    }

    fn push_res(&mut self) {
        let mut buf = [0u8; MAX_COLS];
        let mut len = 0usize;
        len += write_u32(&mut buf[len..], self.fb_w as u32);
        if len < MAX_COLS {
            buf[len] = b'x';
            len += 1;
        }
        len += write_u32(&mut buf[len..], self.fb_h as u32);
        self.push_line_raw(&buf, len);
    }

    fn push_time(&mut self) {
        if let Some(t) = rtc::read_time() {
            let settings = system::ui_settings();
            let mut hour = t.hour;
            if !settings.clock_24h {
                if hour == 0 {
                    hour = 12;
                } else if hour > 12 {
                    hour -= 12;
                }
            }
            let mut buf = [0u8; MAX_COLS];
            let mut len = 0usize;
            len += write_two(&mut buf[len..], hour);
            if len < MAX_COLS {
                buf[len] = b':';
                len += 1;
            }
            len += write_two(&mut buf[len..], t.min);
            if len < MAX_COLS {
                buf[len] = b':';
                len += 1;
            }
            len += write_two(&mut buf[len..], t.sec);
            self.push_line_raw(&buf, len);
        } else {
            self.push_line(b"(time unavailable)");
        }
    }

    fn push_date(&mut self) {
        if let Some(t) = rtc::read_time() {
            let mut buf = [0u8; MAX_COLS];
            let mut len = 0usize;
            len += write_u32(&mut buf[len..], t.year as u32);
            if len < MAX_COLS {
                buf[len] = b'-';
                len += 1;
            }
            len += write_two(&mut buf[len..], t.month);
            if len < MAX_COLS {
                buf[len] = b'-';
                len += 1;
            }
            len += write_two(&mut buf[len..], t.day);
            self.push_line_raw(&buf, len);
        } else {
            self.push_line(b"(date unavailable)");
        }
    }

    fn push_mem(&mut self) {
        let info = system::system_info();
        let total_mib = info.mem_total_kib / 1024;
        let avail_mib = info.mem_avail_kib / 1024;
        let used_mib = total_mib.saturating_sub(avail_mib);
        self.push_line_num(b"RAM total: ", total_mib, b" MiB");
        self.push_line_num(b"RAM avail: ", avail_mib, b" MiB");
        self.push_line_num(b"RAM used:  ", used_mib, b" MiB");
    }

    fn push_sysinfo(&mut self) {
        let info = system::system_info();
        let mut buf = [0u8; MAX_COLS];
        let mut len = 0usize;
        len += write_u32(&mut buf[len..], info.fb_w as u32);
        if len < MAX_COLS {
            buf[len] = b'x';
            len += 1;
        }
        len += write_u32(&mut buf[len..], info.fb_h as u32);
        if len < MAX_COLS {
            buf[len] = b'x';
            len += 1;
        }
        len += write_u32(&mut buf[len..], info.fb_bpp as u32);
        self.push_line_raw(&buf, len);
    }

    fn push_rand(&mut self) {
        self.rand_state = self.rand_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let mut buf = [0u8; MAX_COLS];
        let mut len = 0usize;
        len += write_u32(&mut buf[len..], self.rand_state);
        self.push_line_raw(&buf, len);
    }

    fn push_line(&mut self, bytes: &[u8]) {
        let mut line = [0u8; MAX_COLS];
        let mut len = 0usize;
        for &b in bytes {
            if len >= MAX_COLS {
                break;
            }
            line[len] = b;
            len += 1;
        }
        self.push_line_raw(&line, len);
    }

    fn push_line_raw(&mut self, line: &[u8; MAX_COLS], len: usize) {
        if self.count < MAX_LINES {
            self.lines[self.count] = *line;
            self.lens[self.count] = len;
            self.count += 1;
            return;
        }
        for i in 1..MAX_LINES {
            self.lines[i - 1] = self.lines[i];
            self.lens[i - 1] = self.lens[i];
        }
        self.lines[MAX_LINES - 1] = *line;
        self.lens[MAX_LINES - 1] = len;
    }

    fn push_line_num(&mut self, prefix: &[u8], val: u64, suffix: &[u8]) {
        let mut buf = [0u8; MAX_COLS];
        let mut len = 0usize;
        for &b in prefix {
            if len >= MAX_COLS {
                break;
            }
            buf[len] = b;
            len += 1;
        }
        len += write_u64(&mut buf[len..], val);
        for &b in suffix {
            if len >= MAX_COLS {
                break;
            }
            buf[len] = b;
            len += 1;
        }
        self.push_line_raw(&buf, len);
    }

    fn handle_set(&mut self, rest: &[u8]) {
        let (key, value) = split_words(rest);
        if key.is_empty() {
            self.push_line(b"set clock 24|12");
            self.push_line(b"set statusbar on|off");
            self.push_line(b"set accent blue|green|orange|gray");
            self.push_line(b"set mouse 1|2|3|4");
            self.push_line(b"set theme dark|light");
            return;
        }

        if eq_ignore_case(key, b"clock") {
            if eq_ignore_case(value, b"24") {
                system::set_clock_24h(true);
                self.push_line(b"clock = 24h");
            } else if eq_ignore_case(value, b"12") {
                system::set_clock_24h(false);
                self.push_line(b"clock = 12h");
            } else {
                self.push_line(b"usage: set clock 24|12");
            }
            return;
        }

        if eq_ignore_case(key, b"statusbar") {
            if eq_ignore_case(value, b"on") {
                system::set_status_bar(true);
                self.push_line(b"statusbar = on");
            } else if eq_ignore_case(value, b"off") {
                system::set_status_bar(false);
                self.push_line(b"statusbar = off");
            } else {
                self.push_line(b"usage: set statusbar on|off");
            }
            return;
        }

        if eq_ignore_case(key, b"accent") {
            if eq_ignore_case(value, b"blue") {
                system::set_accent(0x003B6EA5);
                self.push_line(b"accent = blue");
            } else if eq_ignore_case(value, b"green") {
                system::set_accent(0x00358C5C);
                self.push_line(b"accent = green");
            } else if eq_ignore_case(value, b"orange") {
                system::set_accent(0x00C7772A);
                self.push_line(b"accent = orange");
            } else if eq_ignore_case(value, b"gray") {
                system::set_accent(0x006B6B6B);
                self.push_line(b"accent = gray");
            } else {
                self.push_line(b"usage: set accent blue|green|orange|gray");
            }
            return;
        }

        if eq_ignore_case(key, b"mouse") {
            if value.len() == 1 {
                let v = value[0];
                if v >= b'1' && v <= b'4' {
                    let scale = (v - b'0') as i32;
                    system::set_mouse_scale(scale);
                    self.push_line(b"mouse speed updated");
                    return;
                }
            }
            self.push_line(b"usage: set mouse 1|2|3|4");
            return;
        }

        if eq_ignore_case(key, b"theme") {
            if eq_ignore_case(value, b"dark") {
                system::set_theme(true);
                self.push_line(b"theme = dark");
            } else if eq_ignore_case(value, b"light") {
                system::set_theme(false);
                self.push_line(b"theme = light");
            } else {
                self.push_line(b"usage: set theme dark|light");
            }
            return;
        }

        self.push_line(b"unknown setting");
    }

    pub fn redraw(&self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        let (bg, input_bg, text) = if ui.dark {
            (0x00212121, 0x002B2B2B, 0x00E6E6E6)
        } else {
            (0x00FFFFFF, 0x00FFFFFF, 0x00111111)
        };
        let chrome = window::draw_window(fb, x, y, w, h, b"Run");
        display::fill_rect(
            fb,
            chrome.content_x,
            chrome.content_y,
            chrome.content_w,
            chrome.content_h,
            bg,
        );

        let text_x = chrome.content_x + PAD;
        let text_y = chrome.content_y + PAD;
        let max_visible = (chrome.content_h.saturating_sub(PAD * 2)) / LINE_HEIGHT;
        let visible_lines = max_visible.saturating_sub(1);
        let start = if self.count > visible_lines {
            self.count - visible_lines
        } else {
            0
        };

        let mut writer = crate::TextWriter::new(*fb);
        writer.set_color(text);

        let mut row = 0usize;
        for i in start..self.count {
            writer.set_pos(text_x, text_y + row * LINE_HEIGHT);
            let len = self.lens[i];
            if len > 0 {
                writer.write_bytes(&self.lines[i][..len]);
            }
            row += 1;
        }

        let input_y = text_y + row * LINE_HEIGHT;
        display::fill_rect(
            fb,
            chrome.content_x,
            input_y.saturating_sub(4),
            chrome.content_w,
            LINE_HEIGHT + 6,
            input_bg,
        );
        writer.set_pos(text_x, input_y);
        writer.write_bytes(b"> ");
        if self.input_len > 0 {
            writer.write_bytes(&self.input[..self.input_len]);
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

fn split_first_word(buf: &[u8; MAX_COLS], len: usize) -> (&[u8], &[u8]) {
    let mut i = 0usize;
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

fn split_words(rest: &[u8]) -> (&[u8], &[u8]) {
    let mut i = 0usize;
    while i < rest.len() && rest[i] == b' ' {
        i += 1;
    }
    let start = i;
    while i < rest.len() && rest[i] != b' ' {
        i += 1;
    }
    let end = i;
    while i < rest.len() && rest[i] == b' ' {
        i += 1;
    }
    (&rest[start..end], &rest[i..])
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

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let w = fb.width / 2;
    let h = fb.height / 2;
    if w == 0 || h == 0 {
        return (0, 0, 0, 0);
    }
    let x = (fb.width - w) / 2;
    let y = (fb.height - h) / 2;
    (x, y, w, h)
}


fn write_u32(buf: &mut [u8], mut val: u32) -> usize {
    if buf.is_empty() {
        return 0;
    }
    if val == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 10];
    let mut n = 0usize;
    while val > 0 && n < tmp.len() {
        tmp[n] = (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    let mut out = 0usize;
    while n > 0 && out < buf.len() {
        n -= 1;
        buf[out] = b'0' + tmp[n];
        out += 1;
    }
    out
}

fn write_u64(buf: &mut [u8], mut val: u64) -> usize {
    if buf.is_empty() {
        return 0;
    }
    if val == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut n = 0usize;
    while val > 0 && n < tmp.len() {
        tmp[n] = (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    let mut out = 0usize;
    while n > 0 && out < buf.len() {
        n -= 1;
        buf[out] = b'0' + tmp[n];
        out += 1;
    }
    out
}

fn write_two(buf: &mut [u8], val: u8) -> usize {
    if buf.len() < 2 {
        return 0;
    }
    buf[0] = b'0' + (val / 10) as u8;
    buf[1] = b'0' + (val % 10) as u8;
    2
}
