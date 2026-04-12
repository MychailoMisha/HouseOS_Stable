// drivers/battery/mod.rs - Простий драйвер батареї

use crate::drivers::port_io::{inb, outb};

static mut BATTERY_LEVEL: u8 = 100;
static mut BATTERY_PRESENT: bool = false;
static mut INITIALIZED: bool = false;

/// Ініціалізація драйвера батареї
pub fn init() {
    unsafe {
        if INITIALIZED {
            return;
        }
        
        // Перевіряємо наявність батареї
        BATTERY_PRESENT = detect_battery();
        if BATTERY_PRESENT {
            BATTERY_LEVEL = read_battery_level();
        }
        INITIALIZED = true;
    }
}

/// Оновити статус батареї
pub fn update() {
    unsafe {
        if !INITIALIZED {
            init();
        }
        if BATTERY_PRESENT {
            BATTERY_LEVEL = read_battery_level();
        }
    }
}

/// Чи є батарея в системі
pub fn has_battery() -> bool {
    unsafe {
        if !INITIALIZED {
            init();
        }
        BATTERY_PRESENT
    }
}

/// Отримати рівень заряду (0-100)
pub fn get_level() -> u8 {
    unsafe {
        if !INITIALIZED {
            init();
        }
        BATTERY_LEVEL
    }
}

/// Визначення наявності батареї
fn detect_battery() -> bool {
    unsafe {
        // Метод 1: QEMU емуляція (для тестування)
        outb(0x501, 0x01);
        let level = inb(0x501);
        if level > 0 && level <= 100 {
            return true;
        }
        
        // Метод 2: ACPI Embedded Controller
        outb(0x66, 0x80);
        for _ in 0..1000 {
            if (inb(0x66) & 0x02) == 0 {
                break;
            }
        }
        let status = inb(0x62);
        if status != 0xFF && (status & 0x10) != 0 {
            return true;
        }
        
        // Метод 3: CMOS батарея
        outb(0x70, 0x0E);
        let cmos = inb(0x71);
        if (cmos & 0x04) == 0 {
            return true;
        }
        
        false
    }
}

/// Зчитування рівня заряду
fn read_battery_level() -> u8 {
    unsafe {
        // QEMU
        outb(0x501, 0x01);
        let level = inb(0x501);
        if level > 0 && level <= 100 {
            return level;
        }
        
        // ACPI Embedded Controller
        outb(0x66, 0x80);
        for _ in 0..1000 {
            if (inb(0x66) & 0x02) == 0 {
                break;
            }
        }
        let status = inb(0x62);
        if status != 0xFF && (status & 0x10) != 0 {
            outb(0x66, 0x83);
            for _ in 0..1000 {
                if (inb(0x66) & 0x02) == 0 {
                    break;
                }
            }
            let level = inb(0x62);
            if level <= 100 {
                return level;
            }
        }
        
        // CMOS - повертаємо 100% якщо батарея OK
        outb(0x70, 0x0E);
        let cmos = inb(0x71);
        if (cmos & 0x04) == 0 {
            return 100;
        } else {
            return 10;
        }
    }
}