// drivers/battery/sim_backend.rs

use crate::drivers::port_io::inb;
use super::{BatteryBackend, BatteryState};

pub struct SimBatteryBackend {
    level: core::sync::atomic::AtomicU8,
}

impl SimBatteryBackend {
    pub const fn new() -> Self {
        Self {
            level: core::sync::atomic::AtomicU8::new(87),
        }
    }
}

impl BatteryBackend for SimBatteryBackend {
    fn init(&self) {
        self.level.store(87, core::sync::atomic::Ordering::Relaxed);
    }

    fn update(&self) {
        // Проста псевдо‑динаміка, щоб UI жив
        let rtc_tick = unsafe { inb(0x40) }; // будь-який "шум"
        if rtc_tick % 16 == 0 {
            let mut lvl = self.level.load(core::sync::atomic::Ordering::Relaxed);
            if lvl > 5 {
                lvl -= 1;
            } else {
                lvl = 95;
            }
            self.level.store(lvl, core::sync::atomic::Ordering::Relaxed);
        }
    }

    fn state(&self) -> BatteryState {
        let lvl = self.level.load(core::sync::atomic::Ordering::Relaxed);
        BatteryState {
            present: true,
            level_percent: lvl,
            charging: lvl > 80,
        }
    }
}
