#![allow(dead_code)]

use crate::drivers::pci::{self, PciDevice};

const MAX_CONTROLLERS: usize = 16;

#[derive(Copy, Clone, PartialEq)]
pub enum UsbControllerKind {
    Uhci,
    Ohci,
    Ehci,
    Xhci,
    Unknown,
}

#[derive(Copy, Clone)]
pub struct UsbController {
    pub kind: UsbControllerKind,
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    pub bars: [u32; 6],
    pub irq_line: u8,
}

impl UsbController {
    const EMPTY: UsbController = UsbController {
        kind: UsbControllerKind::Unknown,
        bus: 0,
        dev: 0,
        func: 0,
        bars: [0; 6],
        irq_line: 0,
    };
}

static mut CONTROLLERS: [UsbController; MAX_CONTROLLERS] = [UsbController::EMPTY; MAX_CONTROLLERS];
static mut CONTROLLER_COUNT: usize = 0;
static mut SCANNED: bool = false;

pub fn init() -> &'static [UsbController] {
    unsafe {
        if SCANNED {
            return &CONTROLLERS[..CONTROLLER_COUNT];
        }
        CONTROLLER_COUNT = 0;
        let devices = pci::scan();
        for dev in devices {
            if dev.class_code != 0x0C || dev.subclass != 0x03 {
                continue;
            }
            if CONTROLLER_COUNT >= MAX_CONTROLLERS {
                break;
            }
            let kind = kind_from_prog_if(dev.prog_if);
            CONTROLLERS[CONTROLLER_COUNT] = UsbController {
                kind,
                bus: dev.bus,
                dev: dev.dev,
                func: dev.func,
                bars: dev.bars,
                irq_line: dev.irq_line,
            };
            CONTROLLER_COUNT += 1;
        }
        SCANNED = true;
        &CONTROLLERS[..CONTROLLER_COUNT]
    }
}

fn kind_from_prog_if(prog_if: u8) -> UsbControllerKind {
    match prog_if {
        0x00 => UsbControllerKind::Uhci,
        0x10 => UsbControllerKind::Ohci,
        0x20 => UsbControllerKind::Ehci,
        0x30 => UsbControllerKind::Xhci,
        _ => UsbControllerKind::Unknown,
    }
}

pub fn controllers() -> &'static [UsbController] {
    init()
}

pub fn any_usb() -> bool {
    !controllers().is_empty()
}
