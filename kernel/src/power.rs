// power.rs - Реальне вимкнення живлення та перезавантаження

use core::arch::asm;
use crate::display::Framebuffer;

/// Реальне вимкнення комп'ютера через ACPI
pub fn shutdown() -> ! {
    unsafe {
        // Метод 1: ACPI shutdown через порт 0x604 (найпоширеніший)
        asm!(
            "out dx, ax",
            in("ax") 0x2000u16,
            in("dx") 0x604u16,
            options(nomem, nostack)
        );
        
        // Невелика пауза
        for _ in 0..100000 {
            asm!("pause");
        }
        
        // Метод 2: ACPI через порт 0xB004 (альтернативний)
        asm!(
            "out dx, ax",
            in("ax") 0x2000u16,
            in("dx") 0xB004u16,
            options(nomem, nostack)
        );
        
        // Метод 3: QEMU/Bochs shutdown
        asm!(
            "out dx, ax",
            in("ax") 0x2000u16,
            in("dx") 0x8900u16,
            options(nomem, nostack)
        );
        
        // Метод 4: VirtualBox shutdown
        asm!(
            "out dx, ax",
            in("ax") 0x3400u16,
            in("dx") 0x4004u16,
            options(nomem, nostack)
        );
        
        // Метод 5: VMWare shutdown
        asm!(
            "mov eax, 0x564D5868",
            "mov ebx, 0x00000000",
            "mov ecx, 0x00000014",
            "mov edx, 0x00005658",
            "out dx, eax",
            options(nomem, nostack)
        );
        
        // Метод 6: APM shutdown через BIOS
        asm!(
            "mov ax, 0x5301",
            "xor bx, bx",
            "int 0x15",
            "mov ax, 0x530E",
            "xor bx, bx",
            "mov cx, 0x0102",
            "int 0x15",
            "mov ax, 0x5307",
            "mov bx, 0x0001",
            "mov cx, 0x0003",
            "int 0x15",
            options(nomem, nostack)
        );
        
        // Метод 7: Скидання через контролер клавіатури з halt
        asm!(
            "mov al, 0xFE",
            "out 0x64, al",
            "hlt",
            options(nomem, nostack)
        );
    }
    
    // Якщо нічого не спрацювало - halt CPU
    loop {
        unsafe { asm!("hlt"); }
    }
}

/// Перезавантаження комп'ютера
pub fn reboot() -> ! {
    unsafe {
        // Метод 1: Контролер клавіатури (стандартний спосіб)
        // Чекаємо коли контролер буде готовий
        let mut timeout = 0xFFFF;
        while timeout > 0 {
            let status: u8;
            asm!("in al, dx", in("dx") 0x64u16, out("al") status);
            if status & 0x02 == 0 {
                break;
            }
            timeout -= 1;
            asm!("pause");
        }
        
        // Відправляємо команду скидання
        asm!(
            "out dx, al",
            in("dx") 0x64u16,
            in("al") 0xFEu8,
            options(nomem, nostack)
        );
        
        // Чекаємо
        for _ in 0..100000 {
            asm!("pause");
        }
        
        // Метод 2: PCI Reset
        asm!(
            "out dx, al",
            in("dx") 0xCF9u16,
            in("al") 0x06u8,
            options(nomem, nostack)
        );
        
        for _ in 0..50000 {
            asm!("pause");
        }
        
        asm!(
            "out dx, al",
            in("dx") 0xCF9u16,
            in("al") 0x0Eu8,
            options(nomem, nostack)
        );
        
        // Метод 3: Triple fault (викликає перезавантаження процесора)
        asm!(
            "lidt [0]",
            "int 3",
            options(nomem, nostack)
        );
        
        // Метод 4: BIOS перезавантаження через int 0x19
        asm!(
            "int 0x19",
            options(nomem, nostack)
        );
    }
    
    loop {
        unsafe { asm!("hlt"); }
    }
}

/// Граціозне вимкнення з повідомленням на екрані
pub fn graceful_shutdown(fb: &Framebuffer) -> ! {
    use crate::display;
    
    let w = fb.width;
    let h = fb.height;
    
    // Малюємо чорний прямокутник по центру
    let box_w = 200;
    let box_h = 60;
    let box_x = w / 2 - box_w / 2;
    let box_y = h / 2 - box_h / 2;
    
    display::fill_rect(fb, box_x, box_y, box_w, box_h, 0x001A1A1A);
    display::fill_rect(fb, box_x, box_y, box_w, 2, 0x000077FF);
    display::fill_rect(fb, box_x, box_y + box_h - 2, box_w, 2, 0x000077FF);
    
    // Текст
    let mut writer = crate::TextWriter::new(*fb);
    writer.set_color(0x00FFFFFF);
    writer.set_pos(box_x + 30, box_y + 20);
    writer.write_bytes(b"Shutting down...");
    
    // Анімація точок
    for i in 0..4 {
        writer.set_pos(box_x + 140, box_y + 20);
        let dots = match i {
            0 => b"   ",
            1 => b".  ",
            2 => b".. ",
            _ => b"...",
        };
        writer.write_bytes(dots);
        
        for _ in 0..1000000 {
            unsafe { asm!("pause"); }
        }
    }
    
    // Вимикаємо
    shutdown()
}

/// Граціозне перезавантаження з повідомленням
pub fn graceful_reboot(fb: &Framebuffer) -> ! {
    use crate::display;
    
    let w = fb.width;
    let h = fb.height;
    
    // Малюємо прямокутник по центру
    let box_w = 200;
    let box_h = 60;
    let box_x = w / 2 - box_w / 2;
    let box_y = h / 2 - box_h / 2;
    
    display::fill_rect(fb, box_x, box_y, box_w, box_h, 0x001A1A1A);
    display::fill_rect(fb, box_x, box_y, box_w, 2, 0x0000AA00);
    display::fill_rect(fb, box_x, box_y + box_h - 2, box_w, 2, 0x0000AA00);
    
    let mut writer = crate::TextWriter::new(*fb);
    writer.set_color(0x00FFFFFF);
    writer.set_pos(box_x + 30, box_y + 20);
    writer.write_bytes(b"Restarting...");
    
    for i in 0..4 {
        writer.set_pos(box_x + 130, box_y + 20);
        let dots = match i {
            0 => b"   ",
            1 => b".  ",
            2 => b".. ",
            _ => b"...",
        };
        writer.write_bytes(dots);
        
        for _ in 0..800000 {
            unsafe { asm!("pause"); }
        }
    }
    
    reboot()
}

/// Просте вимкнення без анімації
pub fn quick_shutdown() -> ! {
    shutdown()
}

/// Просте перезавантаження без анімації
pub fn quick_reboot() -> ! {
    reboot()
}