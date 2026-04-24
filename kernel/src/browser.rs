use crate::display::{self, Framebuffer};
use crate::drivers::net::{self, NetKind, NetState};
use crate::system;
use crate::window;

const MAX_URL: usize = 120;
const MAX_LINES: usize = 40;
const MAX_COLS: usize = 90;
const LINE_HEIGHT: usize = 16;
const PAD: usize = 12;
const SCROLL_W: usize = 14;
const BTN_H: usize = 16;

pub struct Browser {
    visible: bool,
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,
    url: [u8; MAX_URL],
    url_len: usize,
    lines: [[u8; MAX_COLS]; MAX_LINES],
    lens: [usize; MAX_LINES],
    count: usize,
    scroll: usize,
}

impl Browser {
    pub fn new(_fb: Framebuffer) -> Self {
        let mut url = [0u8; MAX_URL];
        let default = b"https://example.com";
        url[..default.len()].copy_from_slice(default);
        Self {
            visible: false,
            win_x: 0,
            win_y: 0,
            win_w: 0,
            win_h: 0,
            url,
            url_len: default.len(),
            lines: [[0u8; MAX_COLS]; MAX_LINES],
            lens: [0; MAX_LINES],
            count: 0,
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
        if self.count == 0 {
            self.navigate_current();
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn handle_click(&mut self, fb: &Framebuffer, x: usize, y: usize) -> bool {
        if !self.visible {
            return false;
        }
        let (wx, wy, ww, wh) = self.rect(fb);
        let body_y = wy + window::HEADER_H + 2;
        let body_h = wh.saturating_sub(window::HEADER_H + 4);

        let url_y = body_y + PAD;
        let go_w = 46usize;
        let go_h = 22usize;
        let go_x = wx + ww.saturating_sub(PAD + go_w);
        if hit(x, y, go_x, url_y, go_w, go_h) {
            self.navigate_current();
            self.redraw(fb);
            return true;
        }

        let scroll_x = wx + ww.saturating_sub(PAD + SCROLL_W);
        let scroll_y = url_y + go_h + PAD;
        let scroll_h = body_h.saturating_sub((url_y - body_y) + go_h + PAD + PAD);

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
        false
    }

    pub fn handle_char(&mut self, ch: u8) {
        if !self.visible {
            return;
        }
        match ch {
            b'\n' => self.navigate_current(),
            0x08 => {
                if self.url_len > 0 {
                    self.url_len -= 1;
                }
            }
            _ if (32..=126).contains(&ch) => {
                if self.url_len < MAX_URL {
                    self.url[self.url_len] = ch;
                    self.url_len += 1;
                }
            }
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

        let chrome = window::draw_window(fb, x, y, w, h, b"Browser");
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

        let url_y = chrome.content_y + PAD;
        let go_w = 46usize;
        let go_h = 22usize;
        let go_x = chrome.content_x + chrome.content_w.saturating_sub(PAD + go_w);

        let url_x = chrome.content_x + PAD;
        let url_w = go_x.saturating_sub(url_x + 8);
        display::fill_rect(
            fb,
            url_x,
            url_y,
            url_w,
            go_h,
            if is_dark { 0x002C333D } else { 0x00EAF1FA },
        );
        display::fill_rect(fb, url_x, url_y, url_w, 1, if is_dark { 0x00424D5D } else { 0x00B8CCE6 });
        display::fill_rect(
            fb,
            url_x,
            url_y + go_h.saturating_sub(1),
            url_w,
            1,
            if is_dark { 0x00424D5D } else { 0x00B8CCE6 },
        );

        fill_vertical_gradient(
            fb,
            go_x,
            url_y,
            go_w,
            go_h,
            if is_dark { 0x00494848 } else { 0x00EAF0F8 },
            if is_dark { 0x003F3F3F } else { 0x00D8E2EE },
        );
        writer.set_color(text_color);
        writer.set_pos(go_x + 11, url_y + 6);
        writer.write_bytes(b"Go");

        writer.set_color(text_color);
        writer.set_pos(url_x + 6, url_y + 6);
        let max_url = url_w.saturating_sub(12) / 8;
        let len = self.url_len.min(max_url);
        writer.write_bytes(&self.url[..len]);

        let text_x = chrome.content_x + PAD;
        let text_y = url_y + go_h + PAD;
        let text_h = chrome.content_h.saturating_sub((text_y - chrome.content_y) + PAD);
        let max_lines = (text_h / LINE_HEIGHT).max(1);
        let max_scroll = self.count.saturating_sub(max_lines);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }

        let start = self.scroll;
        let end = (start + max_lines).min(self.count);
        writer.set_color(text_color);
        for (row, i) in (start..end).enumerate() {
            writer.set_pos(text_x, text_y + row * LINE_HEIGHT);
            writer.write_bytes(&self.lines[i][..self.lens[i]]);
        }

        let scroll_x = chrome.content_x + chrome.content_w.saturating_sub(PAD + SCROLL_W);
        let scroll_y = text_y;
        let scroll_h = text_h;
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
            let thumb_h = ((max_lines as f32 / self.count as f32) * track_h as f32) as usize;
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

    fn navigate_current(&mut self) {
        self.count = 0;
        self.scroll = 0;

        self.push_line(b"HouseOS Browser");
        self.push_line(b"----------------");
        let mut url_line = [0u8; MAX_COLS];
        let mut p = 0usize;
        p += write_str(&mut url_line[p..], b"URL: ");
        p += write_str(&mut url_line[p..], &self.url[..self.url_len]);
        self.push_bytes(&url_line[..p]);

        let devices = net::devices();
        if devices.is_empty() {
            self.push_line(b"No network adapter detected.");
            self.push_line(b"Check QEMU NIC or PCI passthrough.");
            return;
        }

        self.push_line(b"Network device detected:");
        let dev = devices[0];
        let mut line = [0u8; MAX_COLS];
        let mut p = 0usize;
        p += write_str(&mut line[p..], kind_name(dev.kind));
        p += write_str(&mut line[p..], b"  VID:0x");
        p += write_hex16(&mut line[p..], dev.vendor_id);
        p += write_str(&mut line[p..], b"  DID:0x");
        p += write_hex16(&mut line[p..], dev.device_id);
        self.push_bytes(&line[..p]);

        let mut status = [0u8; MAX_COLS];
        let mut p = 0usize;
        p += write_str(&mut status[p..], b"Driver: ");
        p += write_str(&mut status[p..], state_name(dev.state));
        p += write_str(
            &mut status[p..],
            if dev.driver_online {
                b" (online)"
            } else {
                b" (offline)"
            },
        );
        self.push_bytes(&status[..p]);

        let mut io = [0u8; MAX_COLS];
        let mut p = 0usize;
        p += write_str(&mut io[p..], b"IO:0x");
        p += write_hex16(&mut io[p..], dev.io_base);
        p += write_str(&mut io[p..], b"  IRQ:0x");
        p += write_hex8(&mut io[p..], dev.irq_line);
        self.push_bytes(&io[..p]);

        if has_mac(&dev.mac) {
            let mut mac_line = [0u8; MAX_COLS];
            let mut p = 0usize;
            p += write_str(&mut mac_line[p..], b"MAC: ");
            p += write_mac(&mut mac_line[p..], &dev.mac);
            self.push_bytes(&mac_line[..p]);
        }

        if starts_with(&self.url[..self.url_len], b"https://") {
            self.push_line(b"HTTPS requested.");
            if dev.state == NetState::Ready {
                self.push_line(b"NIC driver is ready.");
            } else {
                self.push_line(b"NIC exists but driver init failed.");
            }
            self.push_line(b"TLS + TCP/IP stack is not implemented yet.");
        } else if starts_with(&self.url[..self.url_len], b"http://") {
            self.push_line(b"HTTP requested.");
            if dev.state == NetState::Ready {
                self.push_line(b"NIC driver is ready.");
            } else {
                self.push_line(b"NIC exists but driver init failed.");
            }
            self.push_line(b"TCP/IP + DNS + HTTP parser are still in progress.");
        } else {
            self.push_line(b"Type URL starting with http:// or https://");
        }
    }

    fn push_line(&mut self, line: &[u8]) {
        self.push_bytes(line);
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        if self.count < MAX_LINES {
            let len = bytes.len().min(MAX_COLS);
            self.lines[self.count][..len].copy_from_slice(&bytes[..len]);
            self.lens[self.count] = len;
            self.count += 1;
        } else {
            for i in 1..MAX_LINES {
                self.lines[i - 1] = self.lines[i];
                self.lens[i - 1] = self.lens[i];
            }
            let len = bytes.len().min(MAX_COLS);
            self.lines[MAX_LINES - 1].fill(0);
            self.lines[MAX_LINES - 1][..len].copy_from_slice(&bytes[..len]);
            self.lens[MAX_LINES - 1] = len;
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    fn scroll_down(&mut self, fb: &Framebuffer) {
        let (_, _, _, h) = self.rect(fb);
        let text_h = h.saturating_sub(window::HEADER_H + 52);
        let max_lines = (text_h / LINE_HEIGHT).max(1);
        let max_scroll = self.count.saturating_sub(max_lines);
        if self.scroll < max_scroll {
            self.scroll += 1;
        }
    }
}

fn kind_name(kind: NetKind) -> &'static [u8] {
    match kind {
        NetKind::IntelE1000 => b"Intel E1000",
        NetKind::Realtek8139 => b"Realtek RTL8139",
        NetKind::VirtioNet => b"VirtIO Net",
        NetKind::Unknown => b"Unknown NIC",
    }
}

fn state_name(state: NetState) -> &'static [u8] {
    match state {
        NetState::Detected => b"Detected",
        NetState::Ready => b"Ready",
        NetState::Error => b"Error",
    }
}

fn starts_with(buf: &[u8], pref: &[u8]) -> bool {
    if buf.len() < pref.len() {
        return false;
    }
    for i in 0..pref.len() {
        if buf[i] != pref[i] {
            return false;
        }
    }
    true
}

fn write_str(buf: &mut [u8], s: &[u8]) -> usize {
    let mut n = 0usize;
    while n < s.len() && n < buf.len() {
        buf[n] = s[n];
        n += 1;
    }
    n
}

fn write_hex16(buf: &mut [u8], value: u16) -> usize {
    if buf.len() < 4 {
        return 0;
    }
    let nibbles = [
        ((value >> 12) & 0xF) as u8,
        ((value >> 8) & 0xF) as u8,
        ((value >> 4) & 0xF) as u8,
        (value & 0xF) as u8,
    ];
    for i in 0..4 {
        buf[i] = if nibbles[i] < 10 {
            b'0' + nibbles[i]
        } else {
            b'A' + (nibbles[i] - 10)
        };
    }
    4
}

fn write_hex8(buf: &mut [u8], value: u8) -> usize {
    if buf.len() < 2 {
        return 0;
    }
    let hi = (value >> 4) & 0xF;
    let lo = value & 0xF;
    buf[0] = if hi < 10 { b'0' + hi } else { b'A' + (hi - 10) };
    buf[1] = if lo < 10 { b'0' + lo } else { b'A' + (lo - 10) };
    2
}

fn has_mac(mac: &[u8; 6]) -> bool {
    let mut any = false;
    for b in mac {
        if *b != 0 {
            any = true;
            break;
        }
    }
    any
}

fn write_mac(buf: &mut [u8], mac: &[u8; 6]) -> usize {
    let mut p = 0usize;
    for (idx, byte) in mac.iter().enumerate() {
        if p >= buf.len() {
            break;
        }
        p += write_hex8(&mut buf[p..], *byte);
        if idx < 5 && p < buf.len() {
            buf[p] = b':';
            p += 1;
        }
    }
    p
}

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let w = (fb.width * 4 / 5).min(920).max(520);
    let h = (fb.height * 4 / 5).min(620).max(360);
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
    let ar = ((a >> 16) & 0xFF) as u32;
    let ag = ((a >> 8) & 0xFF) as u32;
    let ab = (a & 0xFF) as u32;
    let br = ((b >> 16) & 0xFF) as u32;
    let bg = ((b >> 8) & 0xFF) as u32;
    let bb = (b & 0xFF) as u32;
    let r = (ar * (den - num) + br * num) / den;
    let g = (ag * (den - num) + bg * num) / den;
    let b = (ab * (den - num) + bb * num) / den;
    (r << 16) | (g << 8) | b
}
