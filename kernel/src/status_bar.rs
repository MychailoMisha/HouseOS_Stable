use crate::display::{self, Framebuffer};
use crate::drivers::battery;
use crate::rtc::RtcTime;
use crate::system;

pub const BAR_H: usize = 32;
const MAX_W: usize = 1920;
const MAX_BACK: usize = MAX_W * BAR_H;

static mut STATUS_BACK: [u32; MAX_BACK] = [0; MAX_BACK];
static mut STATUS_W: usize = 0;
static mut STATUS_SAVED: bool = false;

static mut LAST_TIME_HASH: u32 = 0;
static mut CACHED_TIME_STR: [u8; 8] = [0; 8];
static mut CACHED_DATE_STR: [u8; 10] = [0; 10];

pub fn init(fb: &Framebuffer) {
    if fb.width == 0 || fb.height == 0 {
        return;
    }

    battery::init();

    let w = fb.width.min(MAX_W);
    let y = fb.height.saturating_sub(BAR_H);
    let mut idx = 0usize;
    for row in 0..BAR_H {
        for col in 0..w {
            let px = col;
            let py = y + row;
            unsafe {
                STATUS_BACK[idx] = display::get_pixel(fb, px, py);
            }
            idx += 1;
        }
    }
    unsafe {
        STATUS_W = w;
        STATUS_SAVED = true;
    }
}

pub fn draw(fb: &Framebuffer, now: RtcTime) {
    let settings = system::ui_settings();
    if !settings.status_bar {
        return;
    }

    battery::update();

    let bar_h = BAR_H;
    let y = fb.height.saturating_sub(bar_h);
    let (bg_top, bg_bottom, text, detail_text, brand, border) = if settings.dark {
        (
            0x00272727,
            0x001E1E1E,
            0x00F2F5F8,
            0x00B7C0CC,
            blend_rgb(settings.accent, 0x00FFFFFF, 20),
            0x00444444,
        )
    } else {
        (
            0x00FDFEFF,
            0x00ECF2FB,
            0x00121B29,
            0x004D5D72,
            blend_rgb(settings.accent, 0x00FFFFFF, 36),
            0x00CDDAEA,
        )
    };

    fill_vertical_gradient(fb, 0, y, fb.width, bar_h, bg_top, bg_bottom);
    display::fill_rect(fb, 0, y, fb.width, 1, border);
    display::fill_rect(
        fb,
        0,
        y + bar_h.saturating_sub(1),
        fb.width,
        1,
        blend_rgb(border, 0x00FFFFFF, 24),
    );

    let mut writer = crate::TextWriter::new(*fb);

    writer.set_color(brand);
    writer.set_pos(12, y + 11);
    writer.write_bytes(b"HouseOS");

    let mut right_x = fb.width.saturating_sub(12);

    if battery::has_battery() {
        right_x = draw_battery(fb, right_x, y, bar_h, &mut writer, text, detail_text);
    }

    draw_clock_with_date(right_x, y, now, &settings, &mut writer, text, detail_text);
}

fn draw_battery(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    h: usize,
    writer: &mut crate::TextWriter,
    text_color: u32,
    detail_text_color: u32,
) -> usize {
    let level = battery::get_level();

    let icon_w = 24usize;
    let icon_h = 10usize;
    let icon_x = x.saturating_sub(94);
    let icon_y = y + (h.saturating_sub(icon_h)) / 2 + 1;
    let body_border = if level > 20 { detail_text_color } else { 0x00E84A5F };

    display::fill_rect(fb, icon_x, icon_y, icon_w, 1, body_border);
    display::fill_rect(fb, icon_x, icon_y + icon_h.saturating_sub(1), icon_w, 1, body_border);
    display::fill_rect(fb, icon_x, icon_y, 1, icon_h, body_border);
    display::fill_rect(fb, icon_x + icon_w.saturating_sub(1), icon_y, 1, icon_h, body_border);
    display::fill_rect(
        fb,
        icon_x + icon_w,
        icon_y + 3,
        2,
        icon_h.saturating_sub(6),
        body_border,
    );

    let fill_w = ((icon_w.saturating_sub(4)) * level as usize) / 100;
    let fill_color = if level > 50 {
        0x0051B56B
    } else if level > 20 {
        0x00D39B39
    } else {
        0x00D14B55
    };
    if fill_w > 0 {
        display::fill_rect(
            fb,
            icon_x + 2,
            icon_y + 2,
            fill_w.min(icon_w.saturating_sub(4)),
            icon_h.saturating_sub(4),
            fill_color,
        );
    }

    writer.set_color(text_color);
    writer.set_pos(icon_x + icon_w + 8, y + h / 2 - 4);

    let mut buf = [0u8; 4];
    let idx = if level >= 100 {
        buf[0] = b'1';
        buf[1] = b'0';
        buf[2] = b'0';
        3
    } else if level >= 10 {
        buf[0] = b'0' + (level / 10);
        buf[1] = b'0' + (level % 10);
        2
    } else {
        buf[0] = b'0' + level;
        1
    };
    buf[idx] = b'%';
    writer.write_bytes(&buf[..=idx]);

    icon_x.saturating_sub(8)
}

fn draw_clock_with_date(
    x: usize,
    y: usize,
    now: RtcTime,
    settings: &system::UiSettings,
    writer: &mut crate::TextWriter,
    text_color: u32,
    detail_text_color: u32,
) {
    let clock_x = x.saturating_sub(92);

    let time_hash = (now.hour as u32) << 16 | (now.min as u32) << 8 | (now.sec as u32);
    let use_cached = unsafe { time_hash == LAST_TIME_HASH };

    let time_str: &[u8] = if use_cached {
        unsafe { &CACHED_TIME_STR }
    } else {
        unsafe {
            format_time(&mut CACHED_TIME_STR, now, settings.clock_24h);
            LAST_TIME_HASH = time_hash;
            &CACHED_TIME_STR
        }
    };

    let date_str: &[u8] = if use_cached {
        unsafe { &CACHED_DATE_STR }
    } else {
        unsafe {
            format_date(&mut CACHED_DATE_STR, now);
            &CACHED_DATE_STR
        }
    };

    writer.set_color(text_color);
    let time_len = if settings.clock_24h { 5 } else { 7 };
    let time_w = time_len * 8;
    writer.set_pos(clock_x + (92 - time_w) / 2, y + 4);
    writer.write_bytes(&time_str[..time_len]);

    writer.set_color(detail_text_color);
    let date_len = date_str.iter().take_while(|&&b| b != 0).count();
    let date_w = date_len * 8;
    writer.set_pos(clock_x + (92 - date_w) / 2, y + 16);
    writer.write_bytes(&date_str[..date_len]);
}

fn format_time(buf: &mut [u8; 8], now: RtcTime, clock_24h: bool) {
    let mut hour = now.hour;

    if !clock_24h {
        if hour == 0 {
            hour = 12;
        } else if hour >= 12 {
            if hour > 12 {
                hour -= 12;
            }
            buf[5] = b'P';
            buf[6] = b'M';
        } else {
            buf[5] = b'A';
            buf[6] = b'M';
        }
        buf[7] = 0;
    } else {
        buf[5] = 0;
        buf[6] = 0;
        buf[7] = 0;
    }

    buf[0] = b'0' + (hour / 10) as u8;
    buf[1] = b'0' + (hour % 10) as u8;
    buf[2] = b':';
    buf[3] = b'0' + (now.min / 10) as u8;
    buf[4] = b'0' + (now.min % 10) as u8;
}

fn format_date(buf: &mut [u8; 10], now: RtcTime) {
    buf[0] = b'0' + (now.day / 10) as u8;
    buf[1] = b'0' + (now.day % 10) as u8;
    buf[2] = b'.';
    buf[3] = b'0' + (now.month / 10) as u8;
    buf[4] = b'0' + (now.month % 10) as u8;
    buf[5] = b'.';
    let year = now.year;
    buf[6] = b'0' + ((year / 10) % 10) as u8;
    buf[7] = b'0' + (year % 10) as u8;
    buf[8] = 0;
    buf[9] = 0;
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

fn blend_rgb(base: u32, mix: u32, mix_strength: u8) -> u32 {
    let s = mix_strength as u32;
    let inv = 255u32.saturating_sub(s);
    let br = (base >> 16) & 0xFF;
    let bg = (base >> 8) & 0xFF;
    let bb = base & 0xFF;
    let mr = (mix >> 16) & 0xFF;
    let mg = (mix >> 8) & 0xFF;
    let mb = mix & 0xFF;
    let r = (br * inv + mr * s) / 255;
    let g = (bg * inv + mg * s) / 255;
    let b = (bb * inv + mb * s) / 255;
    (r << 16) | (g << 8) | b
}

pub fn hide(fb: &Framebuffer) {
    unsafe {
        if !STATUS_SAVED || STATUS_W == 0 {
            return;
        }
    }
    let y = fb.height.saturating_sub(BAR_H);
    let w = unsafe { STATUS_W }.min(fb.width);
    let mut idx = 0usize;
    for row in 0..BAR_H {
        for col in 0..w {
            let px = col;
            let py = y + row;
            let rgb = unsafe { STATUS_BACK[idx] };
            display::put_pixel(fb, px, py, rgb);
            idx += 1;
        }
    }
}
