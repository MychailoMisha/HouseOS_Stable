use crate::display::{self, Framebuffer};
use crate::status_bar;
use crate::system;

pub const HIT_START: usize = usize::MAX;

const LEFT_PAD: usize = 8;
const GAP: usize = 6;
const START_W: usize = 64;
const BTN_H: usize = 24;
const RIGHT_RESERVE: usize = 120;

pub fn draw(
    fb: &Framebuffer,
    labels: &[&[u8]],
    visible: &[bool],
    focused: Option<usize>,
    start_open: bool,
) {
    if labels.len() != visible.len() {
        return;
    }

    let settings = system::ui_settings();
    let y = fb.height.saturating_sub(status_bar::BAR_H) + (status_bar::BAR_H.saturating_sub(BTN_H)) / 2;

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

    for i in 0..labels.len() {
        let rect = app_rect(fb, i, labels.len());
        let bg = if Some(i) == focused {
            blend_rgb(settings.accent, 0x00FFFFFF, if settings.dark { 26 } else { 45 })
        } else if visible[i] {
            if settings.dark { 0x0039424F } else { 0x00E1ECF9 }
        } else if settings.dark {
            0x002B313A
        } else {
            0x00CEDAEA
        };
        display::fill_rect(fb, rect.0, y, rect.2, rect.3, bg);

        let text_color = if Some(i) == focused {
            0x00FFFFFF
        } else if settings.dark {
            0x00E7ECF3
        } else {
            0x00172233
        };
        writer.set_color(text_color);
        writer.set_pos(rect.0 + 8, y + 8);
        let label = labels[i];
        let max_chars = rect.2.saturating_sub(12) / 8;
        let len = label.len().min(max_chars);
        writer.write_bytes(&label[..len]);
    }
}

pub fn hit_test(fb: &Framebuffer, x: usize, y: usize, app_count: usize) -> Option<usize> {
    let bar_y = fb.height.saturating_sub(status_bar::BAR_H);
    if y < bar_y || y >= fb.height {
        return None;
    }

    let start = start_rect(fb);
    if hit(x, y, start) {
        return Some(HIT_START);
    }

    for i in 0..app_count {
        if hit(x, y, app_rect(fb, i, app_count)) {
            return Some(i);
        }
    }

    None
}

pub fn start_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let y = fb.height.saturating_sub(status_bar::BAR_H) + (status_bar::BAR_H.saturating_sub(BTN_H)) / 2;
    (LEFT_PAD, y, START_W, BTN_H)
}

pub fn app_rect(fb: &Framebuffer, index: usize, count: usize) -> (usize, usize, usize, usize) {
    let y = fb.height.saturating_sub(status_bar::BAR_H) + (status_bar::BAR_H.saturating_sub(BTN_H)) / 2;
    let start = start_rect(fb);
    let x0 = start.0 + start.2 + GAP;
    let total = count.max(1);

    let avail = fb
        .width
        .saturating_sub(x0 + LEFT_PAD)
        .saturating_sub(RIGHT_RESERVE);

    let gaps = GAP.saturating_mul(total.saturating_sub(1));
    let mut btn_w = 96usize;
    let needed = btn_w.saturating_mul(total).saturating_add(gaps);
    if needed > avail {
        btn_w = avail.saturating_sub(gaps) / total;
        if btn_w < 52 {
            btn_w = 52;
        }
    }

    let x = x0 + index.saturating_mul(btn_w + GAP);
    (x, y, btn_w, BTN_H)
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
