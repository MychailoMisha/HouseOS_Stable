// input.rs
use crate::calculator::Calculator;
use crate::clipboard::ClipboardWindow;
use crate::commands::ConsoleAction;
use crate::console::Console;
use crate::cursor::{self, CursorRaw};
use crate::desktop;
use crate::display::{self, Framebuffer};
use crate::drivers::input::{keyboard_ps2, mouse_ps2};
use crate::drivers::port_io::outb;
use crate::explorer::Explorer;
use crate::optimizer;
use crate::rtc;
use crate::start_menu::{StartAction, StartMenu};
use crate::status_bar;
use crate::system;
use crate::window;
use core::arch::asm;

pub fn run(
    fb: &Framebuffer,
    cursor_raw: Option<CursorRaw>,
    fs_img: Option<crate::ModuleRange>,
) -> ! {
    let mut mouse_x = fb.width.saturating_sub(1) / 2;
    let mut mouse_y = fb.height.saturating_sub(1) / 2;

    cursor::save_background(fb, mouse_x, mouse_y);
    cursor::draw(fb, mouse_x, mouse_y, cursor_raw);

    unsafe {
        mouse_ps2::init();
        keyboard_ps2::init();
    }

    let mut packet = [0u8; 3];
    let mut idx = 0usize;
    let mut prev_buttons: u8 = 0;
    let mut shift = false;
    let mut extended = false;
    let mut win = false;
    let mut ctrl = false;
    let mut alt = false;

    let mut console = Console::new(*fb);
    let mut explorer = Explorer::new(*fb, fs_img);
    let mut clipboard = ClipboardWindow::new(*fb);
    let mut calculator = Calculator::new(fb);
    let mut start_menu = StartMenu::new(*fb);
    status_bar::init(fb);
    let mut status_visible = true;
    let mut last_time: Option<rtc::RtcTime> = None;
    let mut order = [WinKind::Console, WinKind::Explorer, WinKind::Clipboard, WinKind::Calculator];
    let mut dragging: Option<(WinKind, usize, usize)> = None;
    let mut need_redraw = true;
    let mut status_only_redraw = false;

    loop {
        let mut had_activity = false;
        let settings = system::ui_settings();
        if settings.status_bar {
            if let Some(now) = rtc::read_time() {
                if last_time.map(|t| t.sec) != Some(now.sec) || !status_visible {
                    last_time = Some(now);
                    status_visible = true;
                    if need_redraw {
                        // Full redraw is already queued.
                    } else {
                        status_only_redraw = true;
                    }
                }
            }
        } else if status_visible {
            status_visible = false;
            need_redraw = true;
            status_only_redraw = false;
        }

        let mut allow_frame = true;
        if let Some(opt) = optimizer::get_optimizer() {
            allow_frame = opt.begin_frame();
            if opt.prevent_hang() {
                need_redraw = true;
                status_only_redraw = false;
            }
        }
        if let Some(b) = unsafe { mouse_ps2::read_byte() } {
            had_activity = true;
            if idx == 0 && (b & 0x08) == 0 {
                continue;
            }
            packet[idx] = b;
            idx += 1;
            if idx == 3 {
                idx = 0;
                let buttons = packet[0] & 0x07;
                let scale = system::ui_settings().mouse_scale;
                let dx = (packet[1] as i8 as i32) * scale;
                let dy = (packet[2] as i8 as i32) * scale;
                if dx != 0 || dy != 0 {
                    let max_x = fb.width.saturating_sub(cursor::CURSOR_W + 1) as i32;
                    let max_y = fb.height.saturating_sub(cursor::CURSOR_H + 1) as i32;
                    let new_x = (mouse_x as i32 + dx).clamp(0, max_x) as usize;
                    let new_y = (mouse_y as i32 - dy).clamp(0, max_y) as usize;
                    if new_x != mouse_x || new_y != mouse_y {
                        if dragging.is_some() || need_redraw {
                            mouse_x = new_x;
                            mouse_y = new_y;
                            need_redraw = true;
                        } else {
                            let old_x = mouse_x;
                            let old_y = mouse_y;
                            cursor::restore_background(fb, old_x, old_y);
                            mouse_x = new_x;
                            mouse_y = new_y;
                            cursor::save_background(fb, mouse_x, mouse_y);
                            cursor::draw(fb, mouse_x, mouse_y, cursor_raw);
                            let min_x = if old_x < mouse_x { old_x } else { mouse_x };
                            let min_y = if old_y < mouse_y { old_y } else { mouse_y };
                            let max_x = if old_x > mouse_x { old_x } else { mouse_x };
                            let max_y = if old_y > mouse_y { old_y } else { mouse_y };
                            let rect_w = max_x.saturating_sub(min_x) + cursor::CURSOR_W;
                            let rect_h = max_y.saturating_sub(min_y) + cursor::CURSOR_H;
                            display::present_rect(fb, min_x, min_y, rect_w, rect_h);
                        }
                    }
                }
                let left = (buttons & 0x01) != 0;
                let prev_left = (prev_buttons & 0x01) != 0;
                if !left && prev_left {
                    dragging = None;
                }

                if left {
                    if let Some((kind, off_x, off_y)) = dragging {
                        let (wx, wy, ww, wh) = win_rect(kind, fb, &console, &explorer, &clipboard, &calculator);
                        let mut nx = mouse_x.saturating_sub(off_x);
                        let mut ny = mouse_y.saturating_sub(off_y);
                        let max_x = fb.width.saturating_sub(ww);
                        let taskbar_h = if system::ui_settings().status_bar {
                            status_bar::BAR_H
                        } else {
                            0
                        };
                        let max_y = fb.height.saturating_sub(wh + taskbar_h);
                        if nx > max_x {
                            nx = max_x;
                        }
                        if ny > max_y {
                            ny = max_y;
                        }
                        if nx != wx || ny != wy {
                            win_set_pos(kind, nx, ny, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                            need_redraw = true;
                        }
                    }
                }

                if left && !prev_left {
                    if start_menu.is_visible() {
                        if let Some(action) = start_menu.handle_click(fb, mouse_x, mouse_y) {
                            match action {
                                StartAction::OpenConsole => {
                                    start_menu.hide(fb);
                                    win_show(WinKind::Console, fb, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                                    bring_to_front(&mut order, WinKind::Console);
                                    need_redraw = true;
                                }
                                StartAction::OpenExplorer => {
                                    start_menu.hide(fb);
                                    win_show(WinKind::Explorer, fb, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                                    bring_to_front(&mut order, WinKind::Explorer);
                                    need_redraw = true;
                                }
                                StartAction::OpenClipboard => {
                                    start_menu.hide(fb);
                                    win_show(WinKind::Clipboard, fb, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                                    bring_to_front(&mut order, WinKind::Clipboard);
                                    need_redraw = true;
                                }
                                StartAction::OpenBin => {
                                    start_menu.hide(fb);
                                    explorer.show_bin(fb);
                                    bring_to_front(&mut order, WinKind::Explorer);
                                    need_redraw = true;
                                }
                                StartAction::OpenCalculator => {
                                    start_menu.hide(fb);
                                    win_show(WinKind::Calculator, fb, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                                    bring_to_front(&mut order, WinKind::Calculator);
                                    need_redraw = true;
                                }
                                StartAction::ToggleTheme => {
                                    system::toggle_theme();
                                    status_visible = false;
                                    need_redraw = true;
                                }
                                StartAction::Reboot => {
                                    graceful_reboot(fb);
                                }
                                StartAction::Shutdown => {
                                    graceful_shutdown(fb);
                                }
                            }
                        }
                    } else {
                        let mut handled = false;
                        for kind in order.iter().rev().copied() {
                            if !win_visible(kind, &console, &explorer, &clipboard, &calculator) {
                                continue;
                            }
                            let (wx, wy, ww, wh) = win_rect(kind, fb, &console, &explorer, &clipboard, &calculator);
                            let close = window::close_rect(wx, wy, ww);
                            if window::hit(mouse_x, mouse_y, close) {
                                win_hide(kind, fb, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                                need_redraw = true;
                                handled = true;
                                break;
                            }
                            let header = window::header_rect(wx, wy, ww);
                            if window::hit(mouse_x, mouse_y, header) {
                                bring_to_front(&mut order, kind);
                                let off_x = mouse_x.saturating_sub(wx);
                                let off_y = mouse_y.saturating_sub(wy);
                                dragging = Some((kind, off_x, off_y));
                                need_redraw = true;
                                handled = true;
                                break;
                            }
                            if window::hit(mouse_x, mouse_y, (wx, wy, ww, wh)) {
                                bring_to_front(&mut order, kind);
                                win_handle_click(kind, fb, mouse_x, mouse_y, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                                need_redraw = true;
                                handled = true;
                                break;
                            }
                        }
                        if !handled {
                            dragging = None;
                        }
                    }
                }
                prev_buttons = buttons;
            }
        }

        if let Some(sc) = unsafe { keyboard_ps2::read_byte() } {
            had_activity = true;
            if sc == 0xE0 {
                extended = true;
                continue;
            }
            let released = (sc & 0x80) != 0;
            let code = sc & 0x7F;
            
            // Ctrl+Alt+Del - перезавантаження
            if !released {
                if code == 0x1D {
                    ctrl = true;
                } else if code == 0x38 {
                    alt = true;
                } else if code == 0x53 && ctrl && alt {
                    graceful_reboot(fb);
                }
            } else {
                if code == 0x1D {
                    ctrl = false;
                } else if code == 0x38 {
                    alt = false;
                }
            }
            
            if !released && code == 0x01 {
                if start_menu.is_visible() {
                    start_menu.hide(fb);
                    need_redraw = true;
                } else {
                    for kind in order.iter().rev().copied() {
                        if win_visible(kind, &console, &explorer, &clipboard, &calculator) {
                            win_hide(kind, fb, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                            need_redraw = true;
                            break;
                        }
                    }
                }
                continue;
            }
            if extended {
                extended = false;
                if code == 0x1D {
                    ctrl = !released;
                    continue;
                }
                if code == 0x48 {
                    if !released && clipboard.is_visible() {
                        clipboard.scroll_up(fb);
                        need_redraw = true;
                    }
                    continue;
                }
                if code == 0x50 {
                    if !released && clipboard.is_visible() {
                        clipboard.scroll_down(fb);
                        need_redraw = true;
                    }
                    continue;
                }
                if code == 0x5B || code == 0x5C {
                    if !released {
                        if start_menu.is_visible() {
                            start_menu.hide(fb);
                        } else {
                            start_menu.show(fb);
                        }
                        need_redraw = true;
                    }
                    win = !released;
                }
                continue;
            }
            if start_menu.is_visible() {
                continue;
            }
            
            // Визначаємо активне вікно (перше видиме з кінця order)
            let mut focused: Option<WinKind> = None;
            for kind in order.iter().rev().copied() {
                if win_visible(kind, &console, &explorer, &clipboard, &calculator) {
                    focused = Some(kind);
                    break;
                }
            }
            
            if code == 0x2A || code == 0x36 {
                shift = !released;
                continue;
            }
            if code == 0x1D {
                ctrl = !released;
                continue;
            }
            if released {
                continue;
            }
            
            // Win+R для консолі
            if win && code == 0x13 {
                if !console.is_visible() {
                    if start_menu.is_visible() {
                        start_menu.hide(fb);
                    }
                    win_show(WinKind::Console, fb, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                    bring_to_front(&mut order, WinKind::Console);
                    need_redraw = true;
                }
                continue;
            }
            
            match focused {
                Some(WinKind::Console) => {
                    if ctrl {
                        if code == 0x2E {
                            console.copy_input();
                            continue;
                        }
                        if code == 0x2F {
                            console.paste_clipboard(fb);
                            need_redraw = true;
                            continue;
                        }
                    }
                    if let Some(ch) = keyboard_ps2::scancode_to_ascii(code, shift) {
                        if console.handle_char(fb, ch) {
                            need_redraw = true;
                        }
                        // Обробка дій консолі
                        let action = console.take_action();
                        match action {
                            ConsoleAction::OpenExplorer => {
                                win_show(WinKind::Explorer, fb, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                                bring_to_front(&mut order, WinKind::Explorer);
                                need_redraw = true;
                            }
                            ConsoleAction::OpenClipboard => {
                                win_show(WinKind::Clipboard, fb, &mut console, &mut explorer, &mut clipboard, &mut calculator);
                                bring_to_front(&mut order, WinKind::Clipboard);
                                need_redraw = true;
                            }
                            _ => {}
                        }
                    }
                }
                Some(WinKind::Calculator) => {
                    if let Some(ch) = keyboard_ps2::scancode_to_ascii(code, shift) {
                        calculator.handle_char(fb, ch);
                        need_redraw = true;
                    }
                }
                _ => {}
            }
        }
        let mut did_present = false;
        if need_redraw {
            if let Some(opt) = optimizer::get_optimizer() {
                opt.add_dirty_rect(0, 0, fb.width, fb.height);
            }
            redraw_all(
                fb,
                cursor_raw,
                mouse_x,
                mouse_y,
                &mut console,
                &mut explorer,
                &mut clipboard,
                &mut calculator,
                &start_menu,
                &order,
                last_time,
            );
            need_redraw = false;
            status_only_redraw = false;
            did_present = true;
            if let Some(opt) = optimizer::get_optimizer() {
                opt.mark_clean();
                opt.reset_hang_protection();
                opt.end_frame();
            }
        } else if status_only_redraw && allow_frame {
            redraw_status_only(fb, cursor_raw, mouse_x, mouse_y, last_time);
            status_only_redraw = false;
            did_present = true;
            if let Some(opt) = optimizer::get_optimizer() {
                let y = fb.height.saturating_sub(status_bar::BAR_H);
                opt.add_dirty_rect(0, y, fb.width, status_bar::BAR_H);
                opt.end_frame();
            }
        }

        if !did_present && !had_activity {
            unsafe { asm!("pause"); }
        }
    }
}

// --- Допоміжні функції для управління вікнами ---

#[derive(Copy, Clone, PartialEq)]
enum WinKind {
    Console,
    Explorer,
    Clipboard,
    Calculator,
}

fn bring_to_front(order: &mut [WinKind; 4], kind: WinKind) {
    let mut idx = 0;
    while idx < order.len() {
        if order[idx] == kind { break; }
        idx += 1;
    }
    if idx >= order.len() { return; }
    let last = order.len() - 1;
    for i in idx..last {
        order[i] = order[i + 1];
    }
    order[last] = kind;
}

fn win_visible(kind: WinKind, console: &Console, explorer: &Explorer, clipboard: &ClipboardWindow, calculator: &Calculator) -> bool {
    match kind {
        WinKind::Console => console.is_visible(),
        WinKind::Explorer => explorer.is_visible(),
        WinKind::Clipboard => clipboard.is_visible(),
        WinKind::Calculator => calculator.is_visible(),
    }
}

fn win_rect(
    kind: WinKind,
    fb: &Framebuffer,
    console: &Console,
    explorer: &Explorer,
    clipboard: &ClipboardWindow,
    calculator: &Calculator,
) -> (usize, usize, usize, usize) {
    match kind {
        WinKind::Console => console.rect(fb),
        WinKind::Explorer => explorer.rect(fb),
        WinKind::Clipboard => clipboard.rect(fb),
        WinKind::Calculator => calculator.rect(fb),
    }
}

fn win_set_pos(
    kind: WinKind,
    x: usize,
    y: usize,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
) {
    match kind {
        WinKind::Console => console.set_pos(x, y),
        WinKind::Explorer => explorer.set_pos(x, y),
        WinKind::Clipboard => clipboard.set_pos(x, y),
        WinKind::Calculator => calculator.set_pos(x, y),
    }
}

fn win_draw(kind: WinKind, fb: &Framebuffer, console: &mut Console, explorer: &mut Explorer, clipboard: &mut ClipboardWindow, calculator: &mut Calculator) {
    match kind {
        WinKind::Console => console.redraw(fb),
        WinKind::Explorer => explorer.redraw(fb),
        WinKind::Clipboard => clipboard.redraw(fb),
        WinKind::Calculator => calculator.redraw(fb),
    }
}

fn win_handle_click(
    kind: WinKind,
    fb: &Framebuffer,
    x: usize,
    y: usize,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
) -> bool {
    match kind {
        WinKind::Console => console.handle_click(fb, x, y),
        WinKind::Explorer => explorer.handle_click(fb, x, y),
        WinKind::Clipboard => clipboard.handle_click(fb, x, y),
        WinKind::Calculator => {
            calculator.handle_click(fb, x, y);
            true
        }
    }
}

fn win_show(
    kind: WinKind,
    fb: &Framebuffer,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
) {
    match kind {
        WinKind::Console => console.show(fb),
        WinKind::Explorer => explorer.show(fb),
        WinKind::Clipboard => clipboard.show(fb),
        WinKind::Calculator => calculator.show(),
    }
}

fn win_hide(
    kind: WinKind,
    fb: &Framebuffer,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
) {
    match kind {
        WinKind::Console => console.hide(fb),
        WinKind::Explorer => explorer.hide(fb),
        WinKind::Clipboard => clipboard.hide(fb),
        WinKind::Calculator => calculator.hide(),
    }
}

fn redraw_all(
    fb: &Framebuffer,
    cursor_raw: Option<CursorRaw>,
    mouse_x: usize,
    mouse_y: usize,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
    start_menu: &StartMenu,
    order: &[WinKind; 4],
    now: Option<rtc::RtcTime>,
) {
    desktop::restore(fb);

    for kind in order.iter().copied() {
        if win_visible(kind, console, explorer, clipboard, calculator) {
            win_draw(kind, fb, console, explorer, clipboard, calculator);
        }
    }

    if start_menu.is_visible() {
        start_menu.refresh(fb);
    }

    if system::ui_settings().status_bar {
        if let Some(t) = now {
            status_bar::draw(fb, t);
        }
    }

    cursor::save_background(fb, mouse_x, mouse_y);
    cursor::draw(fb, mouse_x, mouse_y, cursor_raw);
    display::present(fb);
}

fn redraw_status_only(
    fb: &Framebuffer,
    cursor_raw: Option<CursorRaw>,
    mouse_x: usize,
    mouse_y: usize,
    now: Option<rtc::RtcTime>,
) {
    if !system::ui_settings().status_bar {
        return;
    }
    let t = match now {
        Some(v) => v,
        None => return,
    };

    cursor::restore_background(fb, mouse_x, mouse_y);
    status_bar::draw(fb, t);
    cursor::save_background(fb, mouse_x, mouse_y);
    cursor::draw(fb, mouse_x, mouse_y, cursor_raw);

    let bar_y = fb.height.saturating_sub(status_bar::BAR_H);
    display::present_rect(fb, 0, bar_y, fb.width, status_bar::BAR_H);
    display::present_rect(fb, mouse_x, mouse_y, cursor::CURSOR_W, cursor::CURSOR_H);
}

fn power_message(fb: &Framebuffer, msg: &[u8]) {
    let ui = system::ui_settings();
    let (bg, text) = if ui.dark {
        (0x00101010, 0x00FFFFFF)
    } else {
        (0x00F0F0F0, 0x00000000)
    };
    display::fill_rect(fb, 0, 0, fb.width, fb.height, bg);
    let mut writer = crate::TextWriter::new(*fb);
    writer.set_color(text);
    let text_w = msg.len() * 8;
    let x = fb.width.saturating_sub(text_w) / 2;
    let y = fb.height / 2;
    writer.set_pos(x, y);
    writer.write_bytes(msg);
    display::present(fb);
}

// Функції graceful_shutdown та graceful_reboot залишаються без змін,
// як у попередній версії файлу.
fn graceful_shutdown(fb: &Framebuffer) -> ! {
    power_message(fb, b"Shutting down...");
    for _ in 0..2000000 { unsafe { asm!("pause"); } }
    unsafe {
        asm!("out dx, ax", in("ax") 0x2000u16, in("dx") 0x604u16, options(nomem, nostack));
        asm!("out dx, ax", in("ax") 0x2000u16, in("dx") 0x8900u16, options(nomem, nostack));
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
    }
    loop { unsafe { asm!("hlt"); } }
}

fn graceful_reboot(fb: &Framebuffer) -> ! {
    power_message(fb, b"Restarting...");
    for _ in 0..2000000 { unsafe { asm!("pause"); } }
    unsafe {
        let mut timeout = 0xFFFF;
        while timeout > 0 {
            let status: u8;
            asm!("in al, dx", in("dx") 0x64u16, out("al") status);
            if status & 0x02 == 0 { break; }
            timeout -= 1;
            asm!("pause");
        }
        asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8, options(nomem, nostack));
    }
    for _ in 0..50000 { unsafe { asm!("pause"); } }
    unsafe { outb(0xCF9, 0x06); }
    for _ in 0..50000 { unsafe { asm!("pause"); } }
    unsafe { outb(0xCF9, 0x0E); }
    loop { unsafe { asm!("hlt"); } }
}