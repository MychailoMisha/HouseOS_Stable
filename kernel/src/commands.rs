use crate::clipboard;
use crate::rtc;
use crate::system;

#[derive(Copy, Clone, PartialEq)]
pub enum ConsoleAction {
    None,
    OpenExplorer,
    OpenClipboard,
}

#[derive(Copy, Clone, PartialEq)]
pub enum LineType {
    Normal,
    Success,
    Error,
    Info,
}

pub struct CommandResult {
    pub lines: [[u8; 64]; 8],
    pub lens: [usize; 8],
    pub types: [LineType; 8],
    pub count: usize,
    pub action: ConsoleAction,
}

impl CommandResult {
    pub fn new() -> Self {
        Self {
            lines: [[0u8; 64]; 8],
            lens: [0; 8],
            types: [LineType::Normal; 8],
            count: 0,
            action: ConsoleAction::None,
        }
    }

    pub fn add_line(&mut self, text: &[u8], line_type: LineType) {
        if self.count >= 8 {
            return;
        }
        let mut len = 0;
        for &b in text {
            if len >= 64 {
                break;
            }
            self.lines[self.count][len] = b;
            len += 1;
        }
        self.lens[self.count] = len;
        self.types[self.count] = line_type;
        self.count += 1;
    }

    pub fn add_success(&mut self, text: &[u8]) {
        self.add_line(text, LineType::Success);
    }

    pub fn add_error(&mut self, text: &[u8]) {
        self.add_line(text, LineType::Error);
    }

    pub fn add_info(&mut self, text: &[u8]) {
        self.add_line(text, LineType::Info);
    }

    pub fn add_normal(&mut self, text: &[u8]) {
        self.add_line(text, LineType::Normal);
    }
}

pub fn execute_command(cmd: &[u8], len: usize, rand_state: &mut u32, fb_w: usize, fb_h: usize) -> CommandResult {
    let mut result = CommandResult::new();
    
    let (head, rest) = split_first_word(cmd, len);
    if head.is_empty() {
        return result;
    }

    if eq_ignore_case(head, b"help") {
        result.add_info(b"=== Available Commands ===");
        result.add_normal(b"help  clear  cls  echo");
        result.add_normal(b"ver  about  res  whoami");
        result.add_normal(b"ping  time  date  mem");
        result.add_normal(b"sysinfo  rand  explorer");
        result.add_normal(b"clip  copy  paste  set");
    } else if eq_ignore_case(head, b"clear") || eq_ignore_case(head, b"cls") {
        // Clear will be handled in console
        result.add_success(b"Screen cleared");
    } else if eq_ignore_case(head, b"echo") {
        if rest.is_empty() {
            result.add_normal(b"");
        } else {
            result.add_normal(rest);
        }
    } else if eq_ignore_case(head, b"ver") || eq_ignore_case(head, b"version") {
        result.add_info(b"HouseOS v0.1.0");
        result.add_success(b"Build: Stable");
    } else if eq_ignore_case(head, b"about") {
        result.add_info(b"=== HouseOS ===");
        result.add_normal(b"Lightweight OS Demo");
        result.add_success(b"Status: Running");
    } else if eq_ignore_case(head, b"res") || eq_ignore_case(head, b"mode") {
        let mut buf = [0u8; 64];
        let mut pos = 0;
        pos += write_str(&mut buf[pos..], b"Resolution: ");
        pos += write_u32(&mut buf[pos..], fb_w as u32);
        if pos < 64 { buf[pos] = b'x'; pos += 1; }
        pos += write_u32(&mut buf[pos..], fb_h as u32);
        result.add_line(&buf[..pos], LineType::Info);
        result.add_success(b"Display mode active");
    } else if eq_ignore_case(head, b"whoami") {
        result.add_success(b"root");
    } else if eq_ignore_case(head, b"ping") {
        result.add_success(b"pong");
    } else if eq_ignore_case(head, b"time") {
        if let Some(t) = rtc::read_time() {
            let settings = system::ui_settings();
            let mut hour = t.hour;
            let mut is_pm = false;
            
            if !settings.clock_24h {
                if hour >= 12 {
                    is_pm = true;
                }
                if hour == 0 {
                    hour = 12;
                } else if hour > 12 {
                    hour -= 12;
                }
            }
            
            let mut buf = [0u8; 64];
            let mut pos = 0;
            pos += write_str(&mut buf[pos..], b"Time: ");
            pos += write_two(&mut buf[pos..], hour);
            if pos < 64 { buf[pos] = b':'; pos += 1; }
            pos += write_two(&mut buf[pos..], t.min);
            if pos < 64 { buf[pos] = b':'; pos += 1; }
            pos += write_two(&mut buf[pos..], t.sec);
            
            if !settings.clock_24h {
                if pos < 64 { buf[pos] = b' '; pos += 1; }
                if is_pm {
                    pos += write_str(&mut buf[pos..], b"PM");
                } else {
                    pos += write_str(&mut buf[pos..], b"AM");
                }
            }
            
            result.add_line(&buf[..pos], LineType::Success);
        } else {
            result.add_error(b"Time unavailable");
        }
    } else if eq_ignore_case(head, b"date") {
        if let Some(t) = rtc::read_time() {
            let mut buf = [0u8; 64];
            let mut pos = 0;
            pos += write_str(&mut buf[pos..], b"Date: ");
            pos += write_u32(&mut buf[pos..], t.year as u32);
            if pos < 64 { buf[pos] = b'-'; pos += 1; }
            pos += write_two(&mut buf[pos..], t.month);
            if pos < 64 { buf[pos] = b'-'; pos += 1; }
            pos += write_two(&mut buf[pos..], t.day);
            result.add_line(&buf[..pos], LineType::Success);
        } else {
            result.add_error(b"Date unavailable");
        }
    } else if eq_ignore_case(head, b"mem") {
        let info = system::system_info();
        let total_mib = info.mem_total_kib / 1024;
        let avail_mib = info.mem_avail_kib / 1024;
        let used_mib = total_mib.saturating_sub(avail_mib);
        
        result.add_info(b"=== Memory Info ===");
        add_mem_line(&mut result, b"Total: ", total_mib, b" MiB");
        add_mem_line(&mut result, b"Available: ", avail_mib, b" MiB");
        add_mem_line(&mut result, b"Used: ", used_mib, b" MiB");
        result.add_success(b"Memory OK");
    } else if eq_ignore_case(head, b"sysinfo") {
        let info = system::system_info();
        result.add_info(b"=== System Info ===");
        
        let mut buf = [0u8; 64];
        let mut pos = 0;
        pos += write_str(&mut buf[pos..], b"Display: ");
        pos += write_u32(&mut buf[pos..], info.fb_w as u32);
        if pos < 64 { buf[pos] = b'x'; pos += 1; }
        pos += write_u32(&mut buf[pos..], info.fb_h as u32);
        if pos < 64 { buf[pos] = b' '; pos += 1; }
        pos += write_u32(&mut buf[pos..], info.fb_bpp as u32);
        pos += write_str(&mut buf[pos..], b"bpp");
        result.add_line(&buf[..pos], LineType::Normal);
        
        let total_mib = info.mem_total_kib / 1024;
        add_mem_line(&mut result, b"RAM: ", total_mib, b" MiB");
        result.add_success(b"System healthy");
    } else if eq_ignore_case(head, b"rand") {
        *rand_state = rand_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let mut buf = [0u8; 64];
        let mut pos = 0;
        pos += write_str(&mut buf[pos..], b"Random: ");
        pos += write_u32(&mut buf[pos..], *rand_state);
        result.add_line(&buf[..pos], LineType::Success);
    } else if eq_ignore_case(head, b"set") {
        handle_set(rest, &mut result);
    } else if eq_ignore_case(head, b"clip") || eq_ignore_case(head, b"clipboard") {
        result.add_success(b"Opening clipboard...");
        result.action = ConsoleAction::OpenClipboard;
    } else if eq_ignore_case(head, b"copy") {
        if rest.is_empty() {
            result.add_error(b"Usage: copy <text>");
        } else {
            clipboard::set(rest);
            result.add_success(b"Copied to clipboard");
        }
    } else if eq_ignore_case(head, b"paste") {
        let data = clipboard::data();
        if data.is_empty() {
            result.add_error(b"Clipboard is empty");
        } else {
            result.add_normal(data);
        }
    } else if eq_ignore_case(head, b"explorer") || eq_ignore_case(head, b"dir") || eq_ignore_case(head, b"ls") {
        result.add_success(b"Opening file explorer...");
        result.action = ConsoleAction::OpenExplorer;
    } else {
        let mut buf = [0u8; 64];
        let mut pos = 0;
        pos += write_str(&mut buf[pos..], b"Unknown: '");
        for &b in head {
            if pos >= 60 { break; }
            buf[pos] = b;
            pos += 1;
        }
        if pos < 63 { buf[pos] = b'\''; pos += 1; }
        result.add_error(&buf[..pos]);
    }

    result
}

fn handle_set(rest: &[u8], result: &mut CommandResult) {
    let (key, value) = split_words(rest);
    if key.is_empty() {
        result.add_info(b"=== Settings ===");
        result.add_normal(b"set clock 24|12");
        result.add_normal(b"set statusbar on|off");
        result.add_normal(b"set accent blue|green|orange|gray");
        result.add_normal(b"set mouse 1|2|3|4");
        result.add_normal(b"set theme dark|light");
        return;
    }

    if eq_ignore_case(key, b"clock") {
        if eq_ignore_case(value, b"24") {
            system::set_clock_24h(true);
            result.add_success(b"Clock set to 24h format");
        } else if eq_ignore_case(value, b"12") {
            system::set_clock_24h(false);
            result.add_success(b"Clock set to 12h format");
        } else {
            result.add_error(b"Usage: set clock 24|12");
        }
        return;
    }

    if eq_ignore_case(key, b"statusbar") {
        if eq_ignore_case(value, b"on") {
            system::set_status_bar(true);
            result.add_success(b"Status bar enabled");
        } else if eq_ignore_case(value, b"off") {
            system::set_status_bar(false);
            result.add_success(b"Status bar disabled");
        } else {
            result.add_error(b"Usage: set statusbar on|off");
        }
        return;
    }

    if eq_ignore_case(key, b"accent") {
        if eq_ignore_case(value, b"blue") {
            system::set_accent(0x003A8FE5);
            result.add_success(b"Accent color: Blue");
        } else if eq_ignore_case(value, b"green") {
            system::set_accent(0x003AA973);
            result.add_success(b"Accent color: Green");
        } else if eq_ignore_case(value, b"orange") {
            system::set_accent(0x00D98A33);
            result.add_success(b"Accent color: Orange");
        } else if eq_ignore_case(value, b"gray") {
            system::set_accent(0x00718393);
            result.add_success(b"Accent color: Gray");
        } else {
            result.add_error(b"Usage: set accent blue|green|orange|gray");
        }
        return;
    }

    if eq_ignore_case(key, b"mouse") {
        if value.len() == 1 {
            let v = value[0];
            if v >= b'1' && v <= b'4' {
                let scale = (v - b'0') as i32;
                system::set_mouse_scale(scale);
                result.add_success(b"Mouse speed updated");
                return;
            }
        }
        result.add_error(b"Usage: set mouse 1|2|3|4");
        return;
    }

    if eq_ignore_case(key, b"theme") {
        if eq_ignore_case(value, b"dark") {
            system::set_theme(true);
            result.add_success(b"Theme: Dark mode");
        } else if eq_ignore_case(value, b"light") {
            system::set_theme(false);
            result.add_success(b"Theme: Light mode");
        } else {
            result.add_error(b"Usage: set theme dark|light");
        }
        return;
    }

    result.add_error(b"Unknown setting");
}

fn add_mem_line(result: &mut CommandResult, prefix: &[u8], val: u64, suffix: &[u8]) {
    let mut buf = [0u8; 64];
    let mut pos = 0;
    pos += write_str(&mut buf[pos..], prefix);
    pos += write_u64(&mut buf[pos..], val);
    pos += write_str(&mut buf[pos..], suffix);
    result.add_normal(&buf[..pos]);
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

fn split_words(rest: &[u8]) -> (&[u8], &[u8]) {
    let mut i = 0;
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

fn write_str(buf: &mut [u8], s: &[u8]) -> usize {
    let mut pos = 0;
    for &b in s {
        if pos >= buf.len() {
            break;
        }
        buf[pos] = b;
        pos += 1;
    }
    pos
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
    let mut n = 0;
    while val > 0 && n < tmp.len() {
        tmp[n] = (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    let mut out = 0;
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
    let mut n = 0;
    while val > 0 && n < tmp.len() {
        tmp[n] = (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    let mut out = 0;
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
    buf[0] = b'0' + (val / 10);
    buf[1] = b'0' + (val % 10);
    2
}