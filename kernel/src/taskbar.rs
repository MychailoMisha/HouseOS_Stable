// taskbar.rs

use crate::display::{self, Framebuffer};
use crate::status_bar;
use crate::system;

pub const HIT_START: usize = usize::MAX;

const LEFT_PAD: usize = 8;
const GAP: usize = 6;
const START_W: usize = 64;
const BTN_H: usize = 24;

/// Опис одного запису на панелі завдань
#[derive(Copy, Clone)]
pub struct TaskbarEntry {
    /// Індекс вікна (0..WIN_COUNT) для зіставлення з WinKind
    pub index: usize,
    /// Підпис кнопки
    pub label: &'static [u8],
    /// Чи вікно зараз видиме на екрані
    pub visible: bool,
}

/// Малює панель завдань лише для відкритих вікон
pub fn draw(
    fb: &Framebuffer,
    entries: &[TaskbarEntry],
    focused: Option<usize>,  // індекс сфокусованого вікна (такий же, як index)
    start_open: bool,
) {
    let settings = system::ui_settings();
    let y = fb.height.saturating_sub(status_bar::BAR_H) + (status_bar::BAR_H.saturating_sub(BTN_H)) / 2;

    // Кнопка «Пуск»
    let start_rect = start_rect(fb);
    let start_bg = if start_open {
        blend_rgb(settings.accent, 0x00FFFFFF, if settings.dark { 30 } else { 42 })
    } else if settings.dark {
        0x00333A45
    } else {
        0x00D9E5F3
    };
    display::fill_rect(fb, start_rect.0, y, start_rect.2, start_rect.3, start_bg);

    let mut writer = crate::TextWriter::new(*fb);
    writer.set_color(if settings.dark { 0x00F3F5F8 } else { 0x00131A28 });
    writer.set_pos(start_rect.0 + 10, y + 8);
    writer.write_bytes(b"Start");

    // Кнопки відкритих застосунків
    let count = entries.len();
    if count == 0 {
        return;
    }

    // Ширина однієї кнопки, щоб вмістити всі
    let x0 = start_rect.0 + start_rect.2 + GAP;
    let avail = fb.width.saturating_sub(x0 + LEFT_PAD).saturating_sub(120); // резерв під годинник
    let gaps = GAP.saturating_mul(count.saturating_sub(1));
    let mut btn_w = 96usize;
    let needed = btn_w.saturating_mul(count).saturating_add(gaps);
    if needed > avail {
        btn_w = avail.saturating_sub(gaps) / count;
        if btn_w < 52 {
            btn_w = 52;
        }
    }

    for (i, entry) in entries.iter().enumerate() {
        let x = x0 + i.saturating_mul(btn_w + GAP);
        let rect = (x, y, btn_w, BTN_H);

        let bg = if Some(entry.index) == focused {
            blend_rgb(settings.accent, 0x00FFFFFF, if settings.dark { 26 } else { 45 })
        } else if entry.visible {
            if settings.dark { 0x0039424F } else { 0x00E1ECF9 }
        } else {
            // невидиме, але відкрите
            if settings.dark { 0x002B313A } else { 0x00CEDAEA }
        };
        display::fill_rect(fb, rect.0, rect.1, rect.2, rect.3, bg);

        let text_color = if Some(entry.index) == focused {
            0x00FFFFFF
        } else if settings.dark {
            0x00E7ECF3
        } else {
            0x00172233
        };
        writer.set_color(text_color);
        writer.set_pos(rect.0 + 8, y + 8);
        let max_chars = rect.2.saturating_sub(12) / 8;
        let len = entry.label.len().min(max_chars);
        writer.write_bytes(&entry.label[..len]);
    }
}

/// Перевіряє влучання миші; повертає індекс вікна (його оригінальний index)
pub fn hit_test(fb: &Framebuffer, x: usize, y: usize, entries: &[TaskbarEntry]) -> Option<usize> {
    let bar_y = fb.height.saturating_sub(status_bar::BAR_H);
    if y < bar_y || y >= fb.height {
        return None;
    }

    let start = start_rect(fb);
    if hit(x, y, start) {
        return Some(HIT_START);
    }

    let count = entries.len();
    if count == 0 {
        return None;
    }

    let x0 = start.0 + start.2 + GAP;
    let avail = fb.width.saturating_sub(x0 + LEFT_PAD).saturating_sub(120);
    let gaps = GAP.saturating_mul(count.saturating_sub(1));
    let mut btn_w = 96usize;
    let needed = btn_w.saturating_mul(count).saturating_add(gaps);
    if needed > avail {
        btn_w = avail.saturating_sub(gaps) / count;
        if btn_w < 52 {
            btn_w = 52;
        }
    }

    for (i, entry) in entries.iter().enumerate() {
        let rx = x0 + i.saturating_mul(btn_w + GAP);
        let ry = fb.height.saturating_sub(status_bar::BAR_H) + (status_bar::BAR_H.saturating_sub(BTN_H)) / 2;
        if hit(x, y, (rx, ry, btn_w, BTN_H)) {
            return Some(entry.index);
        }
    }

    None
}

pub fn start_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let y = fb.height.saturating_sub(status_bar::BAR_H) + (status_bar::BAR_H.saturating_sub(BTN_H)) / 2;
    (LEFT_PAD, y, START_W, BTN_H)
}

fn hit(px: usize, py: usize, r: (usize, usize, usize, usize)) -> bool {
    px >= r.0 && py >= r.1 && px < r.0 + r.2 && py < r.1 + r.3
}

fn blend_rgb(base: u32, mix: u32, mix_strength: u8) -> u32 {
    let s = mix_strength as u32;
    let inv = 255u32.saturating_sub(s);
    let br = (base >> 16) & 0xFF;
    let bg = (base >> 8) & 0xFF;
    let bb = base & 0xFF;
    let mr = (mix >> 16) & 0xFF;
    let mg = (mix >> 8) & 0xFF;
    let mb = mix & 0xFF;
    let r = (br * inv + mr * s) / 255;
    let g = (bg * inv + mg * s) / 255;
    let b = (bb * inv + mb * s) / 255;
    (r << 16) | (g << 8) | b
}