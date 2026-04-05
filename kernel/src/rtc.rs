use crate::drivers::port_io::{inb, outb};

const CMOS_ADDR: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

#[derive(Copy, Clone, PartialEq)]
pub struct RtcTime {
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

pub fn read_time() -> Option<RtcTime> {
    for _ in 0..8 {
        if !wait_not_updating() {
            return None;
        }
        let a = read_raw();
        if !wait_not_updating() {
            return None;
        }
        let b = read_raw();
        if a == b {
            return Some(convert(a));
        }
    }
    None
}

#[derive(Copy, Clone, PartialEq)]
struct RawTime {
    sec: u8,
    min: u8,
    hour: u8,
    day: u8,
    month: u8,
    year: u8,
    status_b: u8,
}

fn read_raw() -> RawTime {
    RawTime {
        sec: read_reg(0x00),
        min: read_reg(0x02),
        hour: read_reg(0x04),
        day: read_reg(0x07),
        month: read_reg(0x08),
        year: read_reg(0x09),
        status_b: read_reg(0x0B),
    }
}

fn convert(raw: RawTime) -> RtcTime {
    let mut sec = raw.sec;
    let mut min = raw.min;
    let mut hour = raw.hour;
    let mut day = raw.day;
    let mut month = raw.month;
    let mut year = raw.year;

    let bcd = (raw.status_b & 0x04) == 0;
    let is_24 = (raw.status_b & 0x02) != 0;

    if bcd {
        sec = bcd_to_bin(sec);
        min = bcd_to_bin(min);
        hour = bcd_to_bin(hour & 0x7F) | (hour & 0x80);
        day = bcd_to_bin(day);
        month = bcd_to_bin(month);
        year = bcd_to_bin(year);
    }

    if !is_24 {
        let pm = (hour & 0x80) != 0;
        hour = hour & 0x7F;
        if pm && hour < 12 {
            hour += 12;
        } else if !pm && hour == 12 {
            hour = 0;
        }
    }

    let full_year = 2000u16 + year as u16;

    RtcTime {
        hour,
        min,
        sec,
        day,
        month,
        year: full_year,
    }
}

fn read_reg(reg: u8) -> u8 {
    unsafe {
        outb(CMOS_ADDR, reg | 0x80);
        inb(CMOS_DATA)
    }
}

fn is_update_in_progress() -> bool {
    read_reg(0x0A) & 0x80 != 0
}

fn wait_not_updating() -> bool {
    for _ in 0..10000 {
        if !is_update_in_progress() {
            return true;
        }
    }
    false
}

fn bcd_to_bin(val: u8) -> u8 {
    (val & 0x0F) + ((val >> 4) * 10)
}
