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
