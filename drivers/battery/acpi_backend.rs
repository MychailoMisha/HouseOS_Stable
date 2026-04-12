// drivers/battery/acpi_backend.rs

use super::{BatteryBackend, BatteryState};

pub struct AcpiBatteryBackend;

impl AcpiBatteryBackend {
    fn find_battery_device() -> Option<AcpiBattery> {
        // TODO:
        // 1. Пройтись по ACPI namespace
        // 2. Знайти пристрій з HID "PNP0C0A" (Battery Device)
        // 3. Зберегти handle
        None
    }

    fn eval_bst(dev: &AcpiBattery) -> Option<AcpiBst> {
        // TODO:
        // Викликати метод _BST
        // _BST повертає пакет з 4 елементів:
        // 0: Battery State (bitfield: discharging/charging/critical)
        // 1: Battery Present Rate
        // 2: Battery Remaining Capacity
        // 3: Battery Present Voltage
        None
    }

    fn eval_bif(dev: &AcpiBattery) -> Option<AcpiBif> {
        // TODO:
        // _BIF: інформація про батарею (design capacity, last full capacity, ...)
        None
    }
}

struct AcpiBattery {
    // handle / pointer на ACPI об’єкт
    // залежить від твого ACPI шару
}

struct AcpiBst {
    state: u32,
    remaining_capacity: u32,
    // інше за потреби
}

struct AcpiBif {
    design_capacity: u32,
    last_full_capacity: u32,
    // інше за потреби
}

static mut ACPI_BATTERY: Option<AcpiBattery> = None;
static mut LAST_STATE: BatteryState = BatteryState {
    present: false,
    level_percent: 0,
    charging: false,
};

impl BatteryBackend for AcpiBatteryBackend {
    fn init(&self) {
        unsafe {
            ACPI_BATTERY = Self::find_battery_device();
            if ACPI_BATTERY.is_some() {
                LAST_STATE.present = true;
            }
        }
    }

    fn update(&self) {
        unsafe {
            let Some(ref dev) = ACPI_BATTERY else {
                LAST_STATE.present = false;
                return;
            };

            if let Some(bst) = Self::eval_bst(dev) {
                // Тут потрібен _BIF, щоб порахувати %
                // level = remaining_capacity / last_full_capacity * 100

                let bif = Self::eval_bif(dev);
                let mut level = 0u8;

                if let Some(bif) = bif {
                    if bif.last_full_capacity > 0 {
                        let percent = (bst.remaining_capacity as u64 * 100)
                            / (bif.last_full_capacity as u64);
                        level = percent.clamp(0, 100) as u8;
                    }
                }

                let charging = (bst.state & 0b10) != 0;
                let discharging = (bst.state & 0b1) != 0;

                LAST_STATE = BatteryState {
                    present: true,
                    level_percent: level,
                    charging,
                };

                // Якщо хочеш — можна додати поле "discharging"
                let _ = discharging;
            } else {
                LAST_STATE.present = false;
            }
        }
    }

    fn state(&self) -> BatteryState {
        unsafe { LAST_STATE }
    }
}
