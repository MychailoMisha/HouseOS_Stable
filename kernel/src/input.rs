// input.rs

use crate::browser::Browser;
use crate::calculator::Calculator;
use crate::clipboard::ClipboardWindow;
use crate::commands::ConsoleAction;
use crate::console::Console;
use crate::cursor::{self, CursorRaw};
use crate::desktop;
use crate::display::{self, Framebuffer};
use crate::drivers::input::{keyboard_ps2, mouse_ps2};
use crate::drivers::port_io::outb;
use crate::explorer::{Explorer, ExplorerAction};
use crate::notepad::Notepad;
use crate::optimizer;
use crate::rtc;
use crate::start_menu::{StartAction, StartMenu};
use crate::status_bar;
use crate::system;
use crate::taskbar::{self, TaskbarEntry};
use crate::window;
use core::arch::asm;

const WIN_COUNT: usize = 6;
const TASKBAR_LABELS: [&[u8]; WIN_COUNT] = [
    b"Terminal",
    b"Explorer",
    b"Clipboard",
    b"Calculator",
    b"Browser",
    b"Notepad",
];

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
    let mut browser = Browser::new(*fb);
    let mut notepad = Notepad::new(*fb, fs_img);
    let mut start_menu = StartMenu::new(*fb);
    status_bar::init(fb);

    let mut status_visible = true;
    let mut last_time: Option<rtc::RtcTime> = None;
    let mut order = [
        WinKind::Console,
        WinKind::Explorer,
        WinKind::Clipboard,
        WinKind::Calculator,
        WinKind::Browser,
        WinKind::Notepad,
    ];
    // Масив «відкритості» (true – застосунок запущено, не закрито хрестиком)
    let mut open = [false; WIN_COUNT];

    let mut dragging: Option<(WinKind, usize, usize)> = None;
    let mut need_redraw = true;
    let mut status_only_redraw = false;
    let mut maximized = [false; WIN_COUNT];
    let mut restore_rects = [(0usize, 0usize, 0usize, 0usize); WIN_COUNT];

    loop {
        let mut had_activity = false;
        let settings = system::ui_settings();
        if settings.status_bar {
            if let Some(now) = rtc::read_time() {
                if last_time.map(|t| t.sec) != Some(now.sec) || !status_visible {
                    last_time = Some(now);
                    status_visible = true;
                    if !need_redraw {
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
                        if !maximized[win_index(kind)] {
                            let (wx, wy, ww, wh) = win_rect(
                                kind,
                                fb,
                                &console,
                                &explorer,
                                &clipboard,
                                &calculator,
                                &browser,
                                &notepad,
                            );
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
                                win_set_pos(
                                    kind,
                                    nx,
                                    ny,
                                    &mut console,
                                    &mut explorer,
                                    &mut clipboard,
                                    &mut calculator,
                                    &mut browser,
                                    &mut notepad,
                                );
                                need_redraw = true;
                            }
                        }
                    }
                }

                if left && !prev_left {
                    let mut handled = false;

                    // Побудова списку відкритих вікон для панелі завдань
                    let visibility = visibility_state(
                        &console,
                        &explorer,
                        &clipboard,
                        &calculator,
                        &browser,
                        &notepad,
                    );
                    let mut entries_buf = [TaskbarEntry { index: 0, label: &[], visible: false }; WIN_COUNT];
                    let mut entry_count = 0;
                    for i in 0..WIN_COUNT {
                        if open[i] {
                            entries_buf[entry_count] = TaskbarEntry {
                                index: i,
                                label: TASKBAR_LABELS[i],
                                visible: visibility[i],
                            };
                            entry_count += 1;
                        }
                    }
                    let entries = &entries_buf[..entry_count];

                    if system::ui_settings().status_bar {
                        if let Some(hit) = taskbar::hit_test(fb, mouse_x, mouse_y, entries) {
                            if hit == taskbar::HIT_START {
                                if start_menu.is_visible() {
                                    start_menu.hide(fb);
                                } else {
                                    start_menu.show(fb);
                                }
                                need_redraw = true;
                                handled = true;
                            } else {
                                // hit містить оригінальний індекс вікна
                                let kind = kind_from_index(hit);
                                let focused = focused_window(
                                    &order,
                                    &console,
                                    &explorer,
                                    &clipboard,
                                    &calculator,
                                    &browser,
                                    &notepad,
                                );
                                if win_visible(
                                    kind,
                                    &console,
                                    &explorer,
                                    &clipboard,
                                    &calculator,
                                    &browser,
                                    &notepad,
                                ) {
                                    if focused == Some(kind) {
                                        win_hide(
                                            kind,
                                            fb,
                                            &mut console,
                                            &mut explorer,
                                            &mut clipboard,
                                            &mut calculator,
                                            &mut browser,
                                            &mut notepad,
                                        );
                                    } else {
                                        bring_to_front(&mut order, kind);
                                    }
                                } else {
                                    win_show(
                                        kind,
                                        fb,
                                        &mut console,
                                        &mut explorer,
                                        &mut clipboard,
                                        &mut calculator,
                                        &mut browser,
                                        &mut notepad,
                                    );
                                    bring_to_front(&mut order, kind);
                                }
                                need_redraw = true;
                                handled = true;
                            }
                        }
                    }

                    if !handled && start_menu.is_visible() {
                        if let Some(action) = start_menu.handle_click(fb, mouse_x, mouse_y) {
                            handle_start_action(
                                action,
                                fb,
                                &mut start_menu,
                                &mut console,
                                &mut explorer,
                                &mut clipboard,
                                &mut calculator,
                                &mut browser,
                                &mut notepad,
                                &mut order,
                                &mut status_visible,
                                &mut open,
                            );
                            need_redraw = true;
                            handled = true;
                        }
                    }

                    if !handled {
                        for kind in order.iter().rev().copied() {
                            if !open[win_index(kind)] {
                                continue;
                            }
                            if !win_visible(
                                kind,
                                &console,
                                &explorer,
                                &clipboard,
                                &calculator,
                                &browser,
                                &notepad,
                            ) {
                                continue;
                            }

                            let (wx, wy, ww, wh) = win_rect(
                                kind,
                                fb,
                                &console,
                                &explorer,
                                &clipboard,
                                &calculator,
                                &browser,
                                &notepad,
                            );
                            let close = window::close_rect(wx, wy, ww);
                            if window::hit(mouse_x, mouse_y, close) {
                                // Закриття хрестиком – прибираємо з панелі
                                open[win_index(kind)] = false;
                                win_hide(
                                    kind,
                                    fb,
                                    &mut console,
                                    &mut explorer,
                                    &mut clipboard,
                                    &mut calculator,
                                    &mut browser,
                                    &mut notepad,
                                );
                                maximized[win_index(kind)] = false;
                                need_redraw = true;
                                handled = true;
                                break;
                            }

                            let min_btn = window::minimize_rect(wx, wy, ww);
                            if window::hit(mouse_x, mouse_y, min_btn) {
                                // Згортання – залишаємо відкритим
                                win_hide(
                                    kind,
                                    fb,
                                    &mut console,
                                    &mut explorer,
                                    &mut clipboard,
                                    &mut calculator,
                                    &mut browser,
                                    &mut notepad,
                                );
                                need_redraw = true;
                                handled = true;
                                break;
                            }

                            let max_btn = window::maximize_rect(wx, wy, ww);
                            if window::hit(mouse_x, mouse_y, max_btn) {
                                toggle_maximize(
                                    kind,
                                    fb,
                                    &mut maximized,
                                    &mut restore_rects,
                                    &mut console,
                                    &mut explorer,
                                    &mut clipboard,
                                    &mut calculator,
                                    &mut browser,
                                    &mut notepad,
                                );
                                bring_to_front(&mut order, kind);
                                need_redraw = true;
                                handled = true;
                                break;
                            }

                            let drag_header = window::drag_header_rect(wx, wy, ww);
                            if !maximized[win_index(kind)]
                                && window::hit(mouse_x, mouse_y, drag_header)
                            {
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
                                win_handle_click(
                                    kind,
                                    fb,
                                    mouse_x,
                                    mouse_y,
                                    &mut console,
                                    &mut explorer,
                                    &mut clipboard,
                                    &mut calculator,
                                    &mut browser,
                                    &mut notepad,
                                );
                                need_redraw = true;
                                handled = true;
                                break;
                            }
                        }
                    }

                    if !handled {
                        dragging = None;
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

            if !released {
                if code == 0x1D {
                    ctrl = true;
                } else if code == 0x38 {
                    alt = true;
                } else if code == 0x53 && ctrl && alt {
                    graceful_reboot(fb);
                }
            } else if code == 0x1D {
                ctrl = false;
            } else if code == 0x38 {
                alt = false;
            }

            if !released && code == 0x01 {
                if start_menu.is_visible() {
                    start_menu.hide(fb);
                    need_redraw = true;
                } else {
                    for kind in order.iter().rev().copied() {
                        if open[win_index(kind)] && win_visible(
                            kind,
                            &console,
                            &explorer,
                            &clipboard,
                            &calculator,
                            &browser,
                            &notepad,
                        ) {
                            // Esc згортає, а не закриває
                            win_hide(
                                kind,
                                fb,
                                &mut console,
                                &mut explorer,
                                &mut clipboard,
                                &mut calculator,
                                &mut browser,
                                &mut notepad,
                            );
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

            let focused = focused_window(
                &order,
                &console,
                &explorer,
                &clipboard,
                &calculator,
                &browser,
                &notepad,
            );

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

            if win && code == 0x13 {
                if !console.is_visible() {
                    if start_menu.is_visible() {
                        start_menu.hide(fb);
                    }
                    win_show(
                        WinKind::Console,
                        fb,
                        &mut console,
                        &mut explorer,
                        &mut clipboard,
                        &mut calculator,
                        &mut browser,
                        &mut notepad,
                    );
                    open[win_index(WinKind::Console)] = true;
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
                        let action = console.take_action();
                        match action {
                            ConsoleAction::OpenExplorer => {
                                win_show(
                                    WinKind::Explorer,
                                    fb,
                                    &mut console,
                                    &mut explorer,
                                    &mut clipboard,
                                    &mut calculator,
                                    &mut browser,
                                    &mut notepad,
                                );
                                open[win_index(WinKind::Explorer)] = true;
                                bring_to_front(&mut order, WinKind::Explorer);
                                need_redraw = true;
                            }
                            ConsoleAction::OpenClipboard => {
                                win_show(
                                    WinKind::Clipboard,
                                    fb,
                                    &mut console,
                                    &mut explorer,
                                    &mut clipboard,
                                    &mut calculator,
                                    &mut browser,
                                    &mut notepad,
                                );
                                open[win_index(WinKind::Clipboard)] = true;
                                bring_to_front(&mut order, WinKind::Clipboard);
                                need_redraw = true;
                            }
                            ConsoleAction::OpenNotepad => {
                                notepad.open_empty(fb);
                                open[win_index(WinKind::Notepad)] = true;
                                bring_to_front(&mut order, WinKind::Notepad);
                                need_redraw = true;
                            }
                            ConsoleAction::OpenBrowser => {
                                browser.show(fb);
                                open[win_index(WinKind::Browser)] = true;
                                bring_to_front(&mut order, WinKind::Browser);
                                need_redraw = true;
                            }
                            ConsoleAction::Reboot => {
                                graceful_reboot(fb);
                            }
                            ConsoleAction::Shutdown => {
                                graceful_shutdown(fb);
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
                Some(WinKind::Browser) => {
                    if let Some(ch) = keyboard_ps2::scancode_to_ascii(code, shift) {
                        browser.handle_char(ch);
                        need_redraw = true;
                    }
                }
                Some(WinKind::Notepad) => {
                    if let Some(ch) = keyboard_ps2::scancode_to_ascii(code, shift) {
                        notepad.handle_char(ch);
                        need_redraw = true;
                    }
                }
                _ => {}
            }
        }

        if handle_explorer_action(fb, &mut explorer, &mut notepad, &mut order, &mut open) {
            need_redraw = true;
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
                &mut browser,
                &mut notepad,
                &start_menu,
                &order,
                last_time,
                &open,
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
            redraw_status_only(
                fb,
                cursor_raw,
                mouse_x,
                mouse_y,
                last_time,
                &console,
                &explorer,
                &clipboard,
                &calculator,
                &browser,
                &notepad,
                &order,
                start_menu.is_visible(),
                &open,
            );
            status_only_redraw = false;
            did_present = true;
            if let Some(opt) = optimizer::get_optimizer() {
                let y = fb.height.saturating_sub(status_bar::BAR_H);
                opt.add_dirty_rect(0, y, fb.width, status_bar::BAR_H);
                opt.end_frame();
            }
        }

        if !did_present && !had_activity {
            unsafe {
                asm!("pause");
            }
        }
    }
}

// ... (інші функції, що не змінилися, залишаються без змін, але тут ми їх теж наводимо для повноти)

#[derive(Copy, Clone, PartialEq)]
enum WinKind {
    Console,
    Explorer,
    Clipboard,
    Calculator,
    Browser,
    Notepad,
}

fn win_index(kind: WinKind) -> usize {
    match kind {
        WinKind::Console => 0,
        WinKind::Explorer => 1,
        WinKind::Clipboard => 2,
        WinKind::Calculator => 3,
        WinKind::Browser => 4,
        WinKind::Notepad => 5,
    }
}

fn kind_from_index(idx: usize) -> WinKind {
    match idx {
        0 => WinKind::Console,
        1 => WinKind::Explorer,
        2 => WinKind::Clipboard,
        3 => WinKind::Calculator,
        4 => WinKind::Browser,
        _ => WinKind::Notepad,
    }
}

fn bring_to_front(order: &mut [WinKind; WIN_COUNT], kind: WinKind) {
    let mut idx = 0;
    while idx < order.len() {
        if order[idx] == kind {
            break;
        }
        idx += 1;
    }
    if idx >= order.len() {
        return;
    }
    let last = order.len() - 1;
    for i in idx..last {
        order[i] = order[i + 1];
    }
    order[last] = kind;
}

fn focused_window(
    order: &[WinKind; WIN_COUNT],
    console: &Console,
    explorer: &Explorer,
    clipboard: &ClipboardWindow,
    calculator: &Calculator,
    browser: &Browser,
    notepad: &Notepad,
) -> Option<WinKind> {
    for kind in order.iter().rev().copied() {
        if win_visible(kind, console, explorer, clipboard, calculator, browser, notepad) {
            return Some(kind);
        }
    }
    None
}

fn visibility_state(
    console: &Console,
    explorer: &Explorer,
    clipboard: &ClipboardWindow,
    calculator: &Calculator,
    browser: &Browser,
    notepad: &Notepad,
) -> [bool; WIN_COUNT] {
    [
        console.is_visible(),
        explorer.is_visible(),
        clipboard.is_visible(),
        calculator.is_visible(),
        browser.is_visible(),
        notepad.is_visible(),
    ]
}

fn handle_start_action(
    action: StartAction,
    fb: &Framebuffer,
    start_menu: &mut StartMenu,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
    browser: &mut Browser,
    notepad: &mut Notepad,
    order: &mut [WinKind; WIN_COUNT],
    status_visible: &mut bool,
    open: &mut [bool; WIN_COUNT],
) {
    match action {
        StartAction::OpenConsole => {
            start_menu.hide(fb);
            win_show(
                WinKind::Console,
                fb,
                console,
                explorer,
                clipboard,
                calculator,
                browser,
                notepad,
            );
            open[win_index(WinKind::Console)] = true;
            bring_to_front(order, WinKind::Console);
        }
        StartAction::OpenExplorer => {
            start_menu.hide(fb);
            win_show(
                WinKind::Explorer,
                fb,
                console,
                explorer,
                clipboard,
                calculator,
                browser,
                notepad,
            );
            open[win_index(WinKind::Explorer)] = true;
            bring_to_front(order, WinKind::Explorer);
        }
        StartAction::OpenClipboard => {
            start_menu.hide(fb);
            win_show(
                WinKind::Clipboard,
                fb,
                console,
                explorer,
                clipboard,
                calculator,
                browser,
                notepad,
            );
            open[win_index(WinKind::Clipboard)] = true;
            bring_to_front(order, WinKind::Clipboard);
        }
        StartAction::OpenNotepad => {
            start_menu.hide(fb);
            notepad.open_empty(fb);
            open[win_index(WinKind::Notepad)] = true;
            bring_to_front(order, WinKind::Notepad);
        }
        StartAction::OpenBrowser => {
            start_menu.hide(fb);
            browser.show(fb);
            open[win_index(WinKind::Browser)] = true;
            bring_to_front(order, WinKind::Browser);
        }
        StartAction::OpenBin => {
            start_menu.hide(fb);
            explorer.show_bin(fb);
            open[win_index(WinKind::Explorer)] = true;
            bring_to_front(order, WinKind::Explorer);
        }
        StartAction::OpenCalculator => {
            start_menu.hide(fb);
            win_show(
                WinKind::Calculator,
                fb,
                console,
                explorer,
                clipboard,
                calculator,
                browser,
                notepad,
            );
            open[win_index(WinKind::Calculator)] = true;
            bring_to_front(order, WinKind::Calculator);
        }
        StartAction::ToggleTheme => {
            system::toggle_theme();
            *status_visible = false;
        }
        StartAction::Reboot => {
            graceful_reboot(fb);
        }
        StartAction::Shutdown => {
            graceful_shutdown(fb);
        }
    }
}

fn handle_explorer_action(
    fb: &Framebuffer,
    explorer: &mut Explorer,
    notepad: &mut Notepad,
    order: &mut [WinKind; WIN_COUNT],
    open: &mut [bool; WIN_COUNT],
) -> bool {
    match explorer.take_action() {
        ExplorerAction::OpenTextFile {
            name,
            name_len,
            cluster,
            size,
        } => {
            notepad.open_file(fb, cluster, size, &name[..name_len]);
            open[win_index(WinKind::Notepad)] = true;
            bring_to_front(order, WinKind::Notepad);
            true
        }
        ExplorerAction::None => false,
    }
}

fn toggle_maximize(
    kind: WinKind,
    fb: &Framebuffer,
    maximized: &mut [bool; WIN_COUNT],
    restore_rects: &mut [(usize, usize, usize, usize); WIN_COUNT],
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
    browser: &mut Browser,
    notepad: &mut Notepad,
) {
    let idx = win_index(kind);
    if maximized[idx] {
        let r = restore_rects[idx];
        if r.2 > 0 && r.3 > 0 {
            win_set_rect(
                kind,
                r.0,
                r.1,
                r.2,
                r.3,
                console,
                explorer,
                clipboard,
                calculator,
                browser,
                notepad,
            );
        }
        maximized[idx] = false;
    } else {
        restore_rects[idx] = win_rect(
            kind,
            fb,
            console,
            explorer,
            clipboard,
            calculator,
            browser,
            notepad,
        );
        let taskbar_h = if system::ui_settings().status_bar {
            status_bar::BAR_H
        } else {
            0
        };
        let h = fb.height.saturating_sub(taskbar_h);
        win_set_rect(
            kind,
            0,
            0,
            fb.width,
            h,
            console,
            explorer,
            clipboard,
            calculator,
            browser,
            notepad,
        );
        maximized[idx] = true;
    }
}

fn win_visible(
    kind: WinKind,
    console: &Console,
    explorer: &Explorer,
    clipboard: &ClipboardWindow,
    calculator: &Calculator,
    browser: &Browser,
    notepad: &Notepad,
) -> bool {
    match kind {
        WinKind::Console => console.is_visible(),
        WinKind::Explorer => explorer.is_visible(),
        WinKind::Clipboard => clipboard.is_visible(),
        WinKind::Calculator => calculator.is_visible(),
        WinKind::Browser => browser.is_visible(),
        WinKind::Notepad => notepad.is_visible(),
    }
}

fn win_rect(
    kind: WinKind,
    fb: &Framebuffer,
    console: &Console,
    explorer: &Explorer,
    clipboard: &ClipboardWindow,
    calculator: &Calculator,
    browser: &Browser,
    notepad: &Notepad,
) -> (usize, usize, usize, usize) {
    match kind {
        WinKind::Console => console.rect(fb),
        WinKind::Explorer => explorer.rect(fb),
        WinKind::Clipboard => clipboard.rect(fb),
        WinKind::Calculator => calculator.rect(fb),
        WinKind::Browser => browser.rect(fb),
        WinKind::Notepad => notepad.rect(fb),
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
    browser: &mut Browser,
    notepad: &mut Notepad,
) {
    match kind {
        WinKind::Console => console.set_pos(x, y),
        WinKind::Explorer => explorer.set_pos(x, y),
        WinKind::Clipboard => clipboard.set_pos(x, y),
        WinKind::Calculator => calculator.set_pos(x, y),
        WinKind::Browser => browser.set_pos(x, y),
        WinKind::Notepad => notepad.set_pos(x, y),
    }
}

fn win_set_rect(
    kind: WinKind,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
    browser: &mut Browser,
    notepad: &mut Notepad,
) {
    match kind {
        WinKind::Console => console.set_rect(x, y, w, h),
        WinKind::Explorer => explorer.set_rect(x, y, w, h),
        WinKind::Clipboard => clipboard.set_rect(x, y, w, h),
        WinKind::Calculator => calculator.set_rect(x, y, w, h),
        WinKind::Browser => browser.set_rect(x, y, w, h),
        WinKind::Notepad => notepad.set_rect(x, y, w, h),
    }
}

fn win_draw(
    kind: WinKind,
    fb: &Framebuffer,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
    browser: &mut Browser,
    notepad: &mut Notepad,
) {
    match kind {
        WinKind::Console => console.redraw(fb),
        WinKind::Explorer => explorer.redraw(fb),
        WinKind::Clipboard => clipboard.redraw(fb),
        WinKind::Calculator => calculator.redraw(fb),
        WinKind::Browser => browser.redraw(fb),
        WinKind::Notepad => notepad.redraw(fb),
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
    browser: &mut Browser,
    notepad: &mut Notepad,
) -> bool {
    match kind {
        WinKind::Console => console.handle_click(fb, x, y),
        WinKind::Explorer => explorer.handle_click(fb, x, y),
        WinKind::Clipboard => clipboard.handle_click(fb, x, y),
        WinKind::Calculator => {
            calculator.handle_click(fb, x, y);
            true
        }
        WinKind::Browser => browser.handle_click(fb, x, y),
        WinKind::Notepad => notepad.handle_click(fb, x, y),
    }
}

fn win_show(
    kind: WinKind,
    fb: &Framebuffer,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
    browser: &mut Browser,
    notepad: &mut Notepad,
) {
    match kind {
        WinKind::Console => console.show(fb),
        WinKind::Explorer => explorer.show(fb),
        WinKind::Clipboard => clipboard.show(fb),
        WinKind::Calculator => calculator.show(),
        WinKind::Browser => browser.show(fb),
        WinKind::Notepad => notepad.show(fb),
    }
}

fn win_hide(
    kind: WinKind,
    fb: &Framebuffer,
    console: &mut Console,
    explorer: &mut Explorer,
    clipboard: &mut ClipboardWindow,
    calculator: &mut Calculator,
    browser: &mut Browser,
    notepad: &mut Notepad,
) {
    match kind {
        WinKind::Console => console.hide(fb),
        WinKind::Explorer => explorer.hide(fb),
        WinKind::Clipboard => clipboard.hide(fb),
        WinKind::Calculator => calculator.hide(),
        WinKind::Browser => browser.hide(),
        WinKind::Notepad => notepad.hide(),
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
    browser: &mut Browser,
    notepad: &mut Notepad,
    start_menu: &StartMenu,
    order: &[WinKind; WIN_COUNT],
    now: Option<rtc::RtcTime>,
    open: &[bool; WIN_COUNT],
) {
    desktop::restore(fb);

    for kind in order.iter().copied() {
        if open[win_index(kind)] && win_visible(kind, console, explorer, clipboard, calculator, browser, notepad) {
            win_draw(kind, fb, console, explorer, clipboard, calculator, browser, notepad);
        }
    }

    if system::ui_settings().status_bar {
        if let Some(t) = now {
            status_bar::draw(fb, t);
        }
        let visibility = visibility_state(console, explorer, clipboard, calculator, browser, notepad);
        let mut entries_buf = [TaskbarEntry { index: 0, label: &[], visible: false }; WIN_COUNT];
        let mut entry_count = 0;
        for i in 0..WIN_COUNT {
            if open[i] {
                entries_buf[entry_count] = TaskbarEntry {
                    index: i,
                    label: TASKBAR_LABELS[i],
                    visible: visibility[i],
                };
                entry_count += 1;
            }
        }
        let entries = &entries_buf[..entry_count];
        let focused = focused_window(order, console, explorer, clipboard, calculator, browser, notepad)
            .map(|kind| win_index(kind))
            .filter(|&idx| open[idx]);
        taskbar::draw(fb, entries, focused, start_menu.is_visible());
    }

    if start_menu.is_visible() {
        start_menu.refresh(fb);
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
    console: &Console,
    explorer: &Explorer,
    clipboard: &ClipboardWindow,
    calculator: &Calculator,
    browser: &Browser,
    notepad: &Notepad,
    order: &[WinKind; WIN_COUNT],
    start_visible: bool,
    open: &[bool; WIN_COUNT],
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

    let visibility = visibility_state(console, explorer, clipboard, calculator, browser, notepad);
    let mut entries_buf = [TaskbarEntry { index: 0, label: &[], visible: false }; WIN_COUNT];
    let mut entry_count = 0;
    for i in 0..WIN_COUNT {
        if open[i] {
            entries_buf[entry_count] = TaskbarEntry {
                index: i,
                label: TASKBAR_LABELS[i],
                visible: visibility[i],
            };
            entry_count += 1;
        }
    }
    let entries = &entries_buf[..entry_count];
    let focused = focused_window(order, console, explorer, clipboard, calculator, browser, notepad)
        .map(|kind| win_index(kind))
        .filter(|&idx| open[idx]);
    taskbar::draw(fb, entries, focused, start_visible);

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

fn graceful_shutdown(fb: &Framebuffer) -> ! {
    power_message(fb, b"Shutting down...");
    for _ in 0..2000000 {
        unsafe {
            asm!("pause");
        }
    }
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
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

fn graceful_reboot(fb: &Framebuffer) -> ! {
    power_message(fb, b"Restarting...");
    for _ in 0..2000000 {
        unsafe {
            asm!("pause");
        }
    }
    unsafe {
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
        asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8, options(nomem, nostack));
    }
    for _ in 0..50000 {
        unsafe {
            asm!("pause");
        }
    }
    unsafe {
        outb(0xCF9, 0x06);
    }
    for _ in 0..50000 {
        unsafe {
            asm!("pause");
        }
    }
    unsafe {
        outb(0xCF9, 0x0E);
    }
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}