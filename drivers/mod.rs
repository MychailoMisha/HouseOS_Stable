#![allow(dead_code)]

pub mod port_io;
pub mod pci;
pub mod usb;
pub mod net;
pub mod battery;

pub use battery::init as battery_init;

pub mod video {
    pub mod vesa_lfb;
}
pub mod input {
    pub mod ps2_controller;
    pub mod keyboard_ps2;
    pub mod mouse_ps2;
    pub mod mouse_cursor;
}
pub mod interrupts {
    pub mod pic8259;
}
pub mod timer {
    pub mod pit8253;
}
pub mod serial {
    pub mod uart16550;
}
