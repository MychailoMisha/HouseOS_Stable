use crate::rtc::RtcTime;

#[derive(Copy, Clone)]
pub struct SystemInfo {
    pub mem_total_kib: u64,
    pub mem_avail_kib: u64,
    pub fb_w: usize,
    pub fb_h: usize,
    pub fb_bpp: u8,
}

#[derive(Copy, Clone)]
pub struct UiSettings {
    pub status_bar: bool,
    pub clock_24h: bool,
    pub accent: u32,
    pub mouse_scale: i32,
    pub dark: bool,
}

static mut SYSINFO: SystemInfo = SystemInfo {
    mem_total_kib: 0,
    mem_avail_kib: 0,
    fb_w: 0,
    fb_h: 0,
    fb_bpp: 0,
};

static mut UI: UiSettings = UiSettings {
    status_bar: true,
    clock_24h: true,
    accent: 0x003A8FE5,
    mouse_scale: 1,
    dark: false,
};

static mut GMT_OFFSET: i8 = 0;
static mut BOOT_TIME: Option<RtcTime> = None;

pub fn set_system_info(info: SystemInfo) {
    unsafe {
        SYSINFO = info;
    }
}

pub fn system_info() -> SystemInfo {
    unsafe { SYSINFO }
}

pub fn ui_settings() -> UiSettings {
    unsafe { UI }
}

pub fn set_status_bar(on: bool) {
    unsafe {
        UI.status_bar = on;
    }
}

pub fn set_clock_24h(on: bool) {
    unsafe {
        UI.clock_24h = on;
    }
}

pub fn set_accent(color: u32) {
    unsafe {
        UI.accent = color;
    }
}

pub fn set_mouse_scale(scale: i32) {
    let mut s = scale;
    if s < 1 {
        s = 1;
    }
    if s > 4 {
        s = 4;
    }
    unsafe {
        UI.mouse_scale = s;
    }
}

pub fn set_theme(dark: bool) {
    unsafe {
        UI.dark = dark;
    }
}

pub fn toggle_theme() -> bool {
    unsafe {
        UI.dark = !UI.dark;
        UI.dark
    }
}

pub fn set_gmt_offset(offset: i8) {
    unsafe {
        GMT_OFFSET = offset;
    }
}

pub fn get_gmt_offset() -> i8 {
    unsafe { GMT_OFFSET }
}

pub fn set_boot_time(time: RtcTime) {
    unsafe {
        BOOT_TIME = Some(time);
    }
}

pub fn get_boot_time() -> Option<RtcTime> {
    unsafe { BOOT_TIME }
}

// Перевірка на високосний рік
fn is_leap(year: i32) -> bool {
    (year % 4 == 0) && (year % 100 != 0 || year % 400 == 0)
}

// Повертає кількість днів у місяці
fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 => 31, 2 => if is_leap(year) { 29 } else { 28 },
        3 => 31, 4 => 30, 5 => 31, 6 => 30,
        7 => 31, 8 => 31, 9 => 30, 10 => 31, 11 => 30, 12 => 31,
        _ => 31,
    }
}

pub fn apply_timezone(mut t: RtcTime) -> RtcTime {
    let offset = get_gmt_offset();
    let mut new_hour = t.hour as i16 + offset as i16;
    let mut day_delta = 0;

    while new_hour < 0 {
        new_hour += 24;
        day_delta -= 1;
    }
    while new_hour >= 24 {
        new_hour -= 24;
        day_delta += 1;
    }
    t.hour = new_hour as u8;

    if day_delta != 0 {
        let mut day = t.day as i32 + day_delta;
        let mut month = t.month as i32;
        let mut year = t.year as i32;

        while day < 1 {
            month -= 1;
            if month < 1 {
                month = 12;
                year -= 1;
            }
            day += days_in_month(year, month);
        }
        while day > days_in_month(year, month) {
            day -= days_in_month(year, month);
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }

        t.day = day as u8;
        t.month = month as u8;
        t.year = year as u16;
    }

    t
}