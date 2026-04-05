use crate::display::{self, Framebuffer};
use crate::rtc::RtcTime;
use crate::system;

pub const BAR_H: usize = 26;
const MAX_W: usize = 1024;
const MAX_BACK: usize = MAX_W * BAR_H;

static mut STATUS_BACK: [u32; MAX_BACK] = [0; MAX_BACK];
static mut STATUS_W: usize = 0;
static mut STATUS_SAVED: bool = false;

pub fn init(fb: &Framebuffer) {
    if fb.width == 0 || fb.height == 0 {
        return;
    }
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
    let bar_h = BAR_H;
    let y = fb.height.saturating_sub(bar_h);
    let (bg, text, brand, border) = if settings.dark {
        (0x001E1E1E, 0x00FFFFFF, settings.accent, 0x00333333)
    } else {
        (0x00F2F2F2, 0x00000000, settings.accent, 0x00D0D0D0)
    };
    display::fill_rect(fb, 0, y, fb.width, bar_h, bg);
    display::fill_rect(fb, 0, y, fb.width, 1, border);

    let mut writer = crate::TextWriter::new(*fb);
    let text_y = y + (bar_h.saturating_sub(8)) / 2;
    writer.set_color(brand);
    writer.set_pos(8, text_y);
    writer.write_bytes(b"HouseOS");

    let mut buf = [0u8; 19];
    format_datetime(&mut buf, now, settings.clock_24h);
    let text_w = buf.len() * 8;
    let x = fb.width.saturating_sub(text_w + 8);
    writer.set_color(text);
    writer.set_pos(x, text_y);
    writer.write_bytes(&buf);
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

fn format_datetime(buf: &mut [u8; 19], now: RtcTime, clock_24h: bool) {
    let mut hour = now.hour;
    if !clock_24h {
        if hour == 0 {
            hour = 12;
        } else if hour > 12 {
            hour -= 12;
        }
    }
    write_year(buf, now.year);
    buf[4] = b'-';
    buf[5] = b'0' + (now.month / 10) as u8;
    buf[6] = b'0' + (now.month % 10) as u8;
    buf[7] = b'-';
    buf[8] = b'0' + (now.day / 10) as u8;
    buf[9] = b'0' + (now.day % 10) as u8;
    buf[10] = b' ';
    buf[11] = b'0' + (hour / 10) as u8;
    buf[12] = b'0' + (hour % 10) as u8;
    buf[13] = b':';
    buf[14] = b'0' + (now.min / 10) as u8;
    buf[15] = b'0' + (now.min % 10) as u8;
    buf[16] = b':';
    buf[17] = b'0' + (now.sec / 10) as u8;
    buf[18] = b'0' + (now.sec % 10) as u8;
}

fn write_year(buf: &mut [u8; 19], year: u16) {
    let y = year;
    buf[0] = b'0' + ((y / 1000) % 10) as u8;
    buf[1] = b'0' + ((y / 100) % 10) as u8;
    buf[2] = b'0' + ((y / 10) % 10) as u8;
    buf[3] = b'0' + (y % 10) as u8;
}
