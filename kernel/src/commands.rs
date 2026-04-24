use crate::clipboard;
use crate::rtc;
use crate::system;
use crate::drivers::port_io::{inb, outb};
use core::arch::asm;

#[derive(Copy, Clone, PartialEq)]
pub enum ConsoleAction {
    None,
    OpenExplorer,
    OpenClipboard,
    OpenNotepad,
    OpenBrowser,
    Reboot,
    Shutdown,
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
        result.add_normal(b"help  clear  cls  echo  ver  about  res  whoami");
        result.add_normal(b"ping  time  date  mem  sysinfo  rand  explorer  notepad  browser");
        result.add_normal(b"clip  copy  paste  set  gmt  uptime  beep");
        result.add_normal(b"sysfetch  reboot  shutdown");
    } else if eq_ignore_case(head, b"clear") || eq_ignore_case(head, b"cls") {
        result.add_success(b"Screen cleared");
    } else if eq_ignore_case(head, b"echo") {
        if rest.is_empty() {
            result.add_normal(b"");
        } else {
            result.add_normal(rest);
        }
    } else if eq_ignore_case(head, b"ver") || eq_ignore_case(head, b"version") {
        result.add_info(b"HouseOS v3.4.0");
        result.add_success(b"Build: Stable (GMT aware)");
    } else if eq_ignore_case(head, b"about") {
        result.add_info(b"=== HouseOS v3.4 ===");
        result.add_normal(b"Lightweight OS Demo with timezone support");
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
            let adjusted = system::apply_timezone(t);
            let settings = system::ui_settings();
            let mut hour = adjusted.hour;
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
            pos += write_str(&mut buf[pos..], b"Time (GMT");
            let offset = system::get_gmt_offset();
            if offset >= 0 {
                if pos < 64 { buf[pos] = b'+'; pos += 1; }
                pos += write_i8(&mut buf[pos..], offset);
            } else {
                if pos < 64 { buf[pos] = b'-'; pos += 1; }
                pos += write_i8(&mut buf[pos..], -offset);
            }
            if pos < 64 { buf[pos] = b')'; pos += 1; }
            if pos < 64 { buf[pos] = b':'; pos += 1; }
            pos += write_str(&mut buf[pos..], b" ");
            pos += write_two(&mut buf[pos..], hour);
            if pos < 64 { buf[pos] = b':'; pos += 1; }
            pos += write_two(&mut buf[pos..], adjusted.min);
            if pos < 64 { buf[pos] = b':'; pos += 1; }
            pos += write_two(&mut buf[pos..], adjusted.sec);
            
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
            let adjusted = system::apply_timezone(t);
            let mut buf = [0u8; 64];
            let mut pos = 0;
            pos += write_str(&mut buf[pos..], b"Date: ");
            pos += write_u32(&mut buf[pos..], adjusted.year as u32);
            if pos < 64 { buf[pos] = b'-'; pos += 1; }
            pos += write_two(&mut buf[pos..], adjusted.month);
            if pos < 64 { buf[pos] = b'-'; pos += 1; }
            pos += write_two(&mut buf[pos..], adjusted.day);
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
    } else if eq_ignore_case(head, b"notepad") || eq_ignore_case(head, b"note") {
        result.add_success(b"Opening notepad...");
        result.action = ConsoleAction::OpenNotepad;
    } else if eq_ignore_case(head, b"browser") || eq_ignore_case(head, b"web") {
        result.add_success(b"Opening browser...");
        result.action = ConsoleAction::OpenBrowser;
    } else if eq_ignore_case(head, b"gmt") || eq_ignore_case(head, b"tz") {
        handle_gmt(rest, &mut result);
    } else if eq_ignore_case(head, b"uptime") {
        handle_uptime(&mut result);
    } else if eq_ignore_case(head, b"beep") {
        handle_beep(&mut result);
    } else if eq_ignore_case(head, b"sysfetch") {
        handle_sysfetch(&mut result, fb_w, fb_h);
    } else if eq_ignore_case(head, b"reboot") {
        result.add_success(b"Rebooting...");
        result.action = ConsoleAction::Reboot;
    } else if eq_ignore_case(head, b"shutdown") {
        result.add_success(b"Shutting down...");
        result.action = ConsoleAction::Shutdown;
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

// ---- GMT (виправлений) ----
fn handle_gmt(rest: &[u8], result: &mut CommandResult) {
    if rest.is_empty() {
        let off = system::get_gmt_offset();
        let mut buf = [0u8; 32];
        let mut pos = 0;
        pos += write_str(&mut buf[pos..], b"Current GMT offset: ");
        if off >= 0 {
            if pos < 32 { buf[pos] = b'+'; pos += 1; }
            pos += write_i8(&mut buf[pos..], off);
        } else {
            if pos < 32 { buf[pos] = b'-'; pos += 1; }
            pos += write_i8(&mut buf[pos..], -off);
        }
        result.add_success(&buf[..pos]);
        return;
    }

    // Пропускаємо пробіли на початку
    let mut i = 0;
    while i < rest.len() && rest[i] == b' ' {
        i += 1;
    }
    if i >= rest.len() {
        result.add_error(b"Missing offset");
        return;
    }
    let start = i;
    while i < rest.len() && rest[i] != b' ' {
        i += 1;
    }
    let token = &rest[start..i];
    if token.is_empty() {
        result.add_error(b"Missing offset");
        return;
    }

    let mut sign: i8 = 1;
    let mut idx = 0;
    if token[0] == b'+' {
        sign = 1;
        idx = 1;
    } else if token[0] == b'-' {
        sign = -1;
        idx = 1;
    }
    if idx >= token.len() {
        result.add_error(b"Missing number after sign");
        return;
    }

    let mut val: i8 = 0;
    for &b in &token[idx..] {
        if b < b'0' || b > b'9' {
            result.add_error(b"Invalid number");
            return;
        }
        val = val * 10 + (b - b'0') as i8;
        if val > 14 {
            result.add_error(b"Offset out of range (-12..+14)");
            return;
        }
    }
    let new_offset = sign * val;
    if new_offset < -12 || new_offset > 14 {
        result.add_error(b"Offset out of range (-12..+14)");
        return;
    }
    system::set_gmt_offset(new_offset);
    let mut buf = [0u8; 32];
    let mut pos = 0;
    pos += write_str(&mut buf[pos..], b"GMT offset set to ");
    if new_offset >= 0 {
        if pos < 32 { buf[pos] = b'+'; pos += 1; }
        pos += write_i8(&mut buf[pos..], new_offset);
    } else {
        if pos < 32 { buf[pos] = b'-'; pos += 1; }
        pos += write_i8(&mut buf[pos..], -new_offset);
    }
    result.add_success(&buf[..pos]);
}

// ---- Uptime ----
fn handle_uptime(result: &mut CommandResult) {
    if let Some(boot) = system::get_boot_time() {
        if let Some(now) = rtc::read_time() {
            let boot_secs = boot.hour as u32 * 3600 + boot.min as u32 * 60 + boot.sec as u32;
            let now_secs = now.hour as u32 * 3600 + now.min as u32 * 60 + now.sec as u32;
            let mut uptime_secs = if now_secs >= boot_secs {
                now_secs - boot_secs
            } else {
                (24*3600 - boot_secs) + now_secs
            };
            let day_diff = (now.year as i32 - boot.year as i32) * 365 +
                           (now.month as i32 - boot.month as i32) * 30 +
                           (now.day as i32 - boot.day as i32);
            if day_diff > 0 {
                uptime_secs += (day_diff as u32) * 86400;
            }
            let days = uptime_secs / 86400;
            let hours = (uptime_secs % 86400) / 3600;
            let mins = (uptime_secs % 3600) / 60;
            let secs = uptime_secs % 60;
            let mut buf = [0u8; 64];
            let mut pos = 0;
            if days > 0 {
                pos += write_u32(&mut buf[pos..], days);
                pos += write_str(&mut buf[pos..], b" days, ");
            }
            pos += write_u32(&mut buf[pos..], hours);
            pos += write_str(&mut buf[pos..], b":");
            pos += write_two(&mut buf[pos..], mins as u8);
            pos += write_str(&mut buf[pos..], b":");
            pos += write_two(&mut buf[pos..], secs as u8);
            result.add_success(&buf[..pos]);
            return;
        }
    }
    result.add_error(b"Uptime not available (RTC missing)");
}

// ---- Beep ----
fn handle_beep(result: &mut CommandResult) {
    unsafe {
        let freq = 800;
        let div = 1193180 / freq;
        outb(0x43, 0xB6);
        outb(0x42, (div & 0xFF) as u8);
        outb(0x42, ((div >> 8) & 0xFF) as u8);
        let status = inb(0x61);
        outb(0x61, status | 0x03);
        for _ in 0..200000 { asm!("pause"); }
        outb(0x61, status & !0x03);
    }
    result.add_success(b"Beep!");
}

// ---- Sysfetch ----
fn handle_sysfetch(result: &mut CommandResult, fb_w: usize, fb_h: usize) {
    let info = system::system_info();
    result.add_info(b"    _____                      ");
    result.add_info(b"   /  _  \\___  ___  ___ ______");
    result.add_info(b"  /  /_\\  \\  \\/  / |/ // __/");
    result.add_info(b" /    /    \\    /|   /\\__ \\");
    result.add_info(b" \\____|_  /__/\\_\\ |_| /___/");
    result.add_info(b"        \\/                    ");
    result.add_normal(b"");
    let mut buf = [0u8; 64];
    let mut pos = 0;
    pos += write_str(&mut buf[pos..], b"OS: HouseOS v3.4");
    result.add_normal(&buf[..pos]);
    pos = 0;
    pos += write_str(&mut buf[pos..], b"Resolution: ");
    pos += write_u32(&mut buf[pos..], fb_w as u32);
    if pos < 64 { buf[pos] = b'x'; pos += 1; }
    pos += write_u32(&mut buf[pos..], fb_h as u32);
    result.add_normal(&buf[..pos]);
    pos = 0;
    pos += write_str(&mut buf[pos..], b"RAM: ");
    pos += write_u64(&mut buf[pos..], info.mem_total_kib / 1024);
    pos += write_str(&mut buf[pos..], b" MiB");
    result.add_normal(&buf[..pos]);
    pos = 0;
    pos += write_str(&mut buf[pos..], b"Timezone: GMT");
    let off = system::get_gmt_offset();
    if off >= 0 {
        if pos < 64 { buf[pos] = b'+'; pos += 1; }
        pos += write_i8(&mut buf[pos..], off);
    } else {
        if pos < 64 { buf[pos] = b'-'; pos += 1; }
        pos += write_i8(&mut buf[pos..], -off);
    }
    result.add_normal(&buf[..pos]);
    result.add_success(b"System ready");
}

// ---- Допоміжні функції ----
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

fn write_i8(buf: &mut [u8], val: i8) -> usize {
    if val < 0 {
        return 0;
    }
    write_u32(buf, val as u32)
}

fn write_two(buf: &mut [u8], val: u8) -> usize {
    if buf.len() < 2 {
        return 0;
    }
    buf[0] = b'0' + (val / 10);
    buf[1] = b'0' + (val % 10);
    2
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
