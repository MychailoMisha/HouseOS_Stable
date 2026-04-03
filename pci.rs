#![allow(dead_code)]

use crate::drivers::port_io::{inl, outl};

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

const MAX_DEVICES: usize = 64;

#[derive(Copy, Clone)]
pub struct PciDevice {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub header_type: u8,
    pub bars: [u32; 6],
    pub irq_line: u8,
}

impl PciDevice {
    const EMPTY: PciDevice = PciDevice {
        bus: 0,
        dev: 0,
        func: 0,
        vendor_id: 0xFFFF,
        device_id: 0xFFFF,
        class_code: 0,
        subclass: 0,
        prog_if: 0,
        header_type: 0,
        bars: [0; 6],
        irq_line: 0,
    };
}

static mut DEVICES: [PciDevice; MAX_DEVICES] = [PciDevice::EMPTY; MAX_DEVICES];
static mut DEVICE_COUNT: usize = 0;
static mut SCANNED: bool = false;

pub fn scan() -> &'static [PciDevice] {
    unsafe {
        if SCANNED {
            return &DEVICES[..DEVICE_COUNT];
        }
        DEVICE_COUNT = 0;
        for bus in 0u16..=255 {
            for dev in 0u16..32 {
                let header = read_config_byte(bus, dev, 0, 0x0E);
                let multi = (header & 0x80) != 0;
                let func_max = if multi { 8 } else { 1 };
                for func in 0u16..func_max {
                    let vendor = read_config_word(bus, dev, func, 0x00);
                    if vendor == 0xFFFF {
                        continue;
                    }
                    if DEVICE_COUNT >= MAX_DEVICES {
                        SCANNED = true;
                        return &DEVICES[..DEVICE_COUNT];
                    }
                    let device_id = read_config_word(bus, dev, func, 0x02);
                    let class_code = read_config_byte(bus, dev, func, 0x0B);
                    let subclass = read_config_byte(bus, dev, func, 0x0A);
                    let prog_if = read_config_byte(bus, dev, func, 0x09);
                    let header_type = read_config_byte(bus, dev, func, 0x0E);
                    let mut bars = [0u32; 6];
                    for i in 0..6 {
                        bars[i] = read_config_dword(bus, dev, func, 0x10 + (i * 4) as u16);
                    }
                    let irq_line = read_config_byte(bus, dev, func, 0x3C);
                    DEVICES[DEVICE_COUNT] = PciDevice {
                        bus: bus as u8,
                        dev: dev as u8,
                        func: func as u8,
                        vendor_id: vendor,
                        device_id,
                        class_code,
                        subclass,
                        prog_if,
                        header_type,
                        bars,
                        irq_line,
                    };
                    DEVICE_COUNT += 1;
                }
            }
        }
        SCANNED = true;
        &DEVICES[..DEVICE_COUNT]
    }
}

pub fn read_config_dword(bus: u16, dev: u16, func: u16, offset: u16) -> u32 {
    let addr = config_address(bus, dev, func, offset);
    unsafe {
        outl(CONFIG_ADDRESS, addr);
        inl(CONFIG_DATA)
    }
}

pub fn read_config_word(bus: u16, dev: u16, func: u16, offset: u16) -> u16 {
    let value = read_config_dword(bus, dev, func, offset & 0xFC);
    let shift = (offset & 2) * 8;
    ((value >> shift) & 0xFFFF) as u16
}

pub fn read_config_byte(bus: u16, dev: u16, func: u16, offset: u16) -> u8 {
    let value = read_config_dword(bus, dev, func, offset & 0xFC);
    let shift = (offset & 3) * 8;
    ((value >> shift) & 0xFF) as u8
}

fn config_address(bus: u16, dev: u16, func: u16, offset: u16) -> u32 {
    0x8000_0000u32
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}
