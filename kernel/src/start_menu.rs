use crate::display::{self, Framebuffer};
use crate::system;

const LINE_HEIGHT: usize = 34;
const PAD: usize = 16;
const BAR_H: usize = 26;
const ICON_SIZE: usize = 18;
const CORNER_RADIUS: usize = 16;

#[derive(Copy, Clone)]
pub enum StartAction {
    OpenConsole,
    OpenExplorer,
    OpenClipboard,
    OpenNotepad,
    OpenBrowser,
    OpenBin,
    OpenCalculator,
    ToggleTheme,
    Reboot,
    Shutdown,
}

pub struct StartMenu {
    visible: bool,
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,
}

impl StartMenu {
    pub fn new(_fb: Framebuffer) -> Self {
        Self {
            visible: false,
            win_x: 0,
            win_y: 0,
            win_w: 0,
            win_h: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, fb: &Framebuffer) {
        self.visible = true;
        let (x, y, w, h) = calc_rect(fb);
        self.win_x = x;
        self.win_y = y;
        self.win_w = w;
        self.win_h = h;
    }

    pub fn hide(&mut self, _fb: &Framebuffer) {
        self.visible = false;
    }

    pub fn handle_click(&mut self, fb: &Framebuffer, x: usize, y: usize) -> Option<StartAction> {
        if !self.visible {
            return None;
        }
        let (wx, wy, ww, _) = self.rect(fb);
        let list_y = wy + PAD;

        if x >= wx && x < wx + ww && y >= list_y {
            let row = (y - list_y) / LINE_HEIGHT;
            return match row {
                0 => Some(StartAction::OpenConsole),
                1 => Some(StartAction::OpenExplorer),
                2 => Some(StartAction::OpenClipboard),
                3 => Some(StartAction::OpenNotepad),
                4 => Some(StartAction::OpenBrowser),
                5 => Some(StartAction::OpenBin),
                6 => Some(StartAction::OpenCalculator),
                7 => None,
                8 => Some(StartAction::ToggleTheme),
                9 => Some(StartAction::Reboot),
                10 => Some(StartAction::Shutdown),
                _ => None,
            };
        }
        None
    }

    pub fn refresh(&self, fb: &Framebuffer) {
        if self.visible {
            self.redraw(fb);
        }
    }

    fn redraw(&self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }

        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        let accent = ui.accent;
        let is_dark = ui.dark;

        draw_rounded_rect(fb, x + 2, y + 2, w, h, CORNER_RADIUS, 0x00101010);
        draw_rounded_rect(
            fb,
            x,
            y,
            w,
            h,
            CORNER_RADIUS,
            if is_dark { 0x001E1E1E } else { 0x00FFFFFF },
        );
        draw_rounded_rect_outline(
            fb,
            x,
            y,
            w,
            h,
            CORNER_RADIUS,
            1,
            if is_dark { 0x003A3A3A } else { 0x00E0E0E0 },
        );

        let mut writer = crate::TextWriter::new(*fb);
        let text_color = if is_dark { 0x00E0E0E0 } else { 0x00202020 };
        let secondary_text = if is_dark { 0x00808080 } else { 0x00707070 };

        let list_x = x + PAD;
        let list_y = y + PAD;
        let items = [
            ("Console", ">"),
            ("Explorer", "#"),
            ("Clipboard", "@"),
            ("Notepad", "N"),
            ("Browser", "W"),
            ("Recycle Bin", "%"),
            ("Calculator", "C"),
        ];

        let mut row = 0usize;
        for (label, icon) in items.iter() {
            let row_y = list_y + row * LINE_HEIGHT;
            writer.set_color(accent);
            writer.set_pos(list_x + 4, row_y + 8);
            writer.write_bytes(icon.as_bytes());

            writer.set_color(text_color);
            writer.set_pos(list_x + ICON_SIZE + 16, row_y + 8);
            writer.write_bytes(label.as_bytes());
            row += 1;
        }

        let sep_y = list_y + row * LINE_HEIGHT + 6;
        display::fill_rect(
            fb,
            x + PAD,
            sep_y,
            w.saturating_sub(PAD * 2),
            1,
            if is_dark { 0x00333333 } else { 0x00F0F0F0 },
        );
        row += 1;

        let actions_after = [
            (if is_dark { "Light Mode" } else { "Dark Mode" }, "T"),
            ("Restart", "R"),
            ("Shutdown", "S"),
        ];

        for (label, icon) in actions_after.iter() {
            let row_y = list_y + row * LINE_HEIGHT;
            writer.set_color(secondary_text);
            writer.set_pos(list_x + 4, row_y + 8);
            writer.write_bytes(icon.as_bytes());

            writer.set_color(text_color);
            writer.set_pos(list_x + ICON_SIZE + 16, row_y + 8);
            writer.write_bytes(label.as_bytes());
            row += 1;
        }
    }

    pub fn rect(&self, fb: &Framebuffer) -> (usize, usize, usize, usize) {
        if self.win_w == 0 {
            calc_rect(fb)
        } else {
            (self.win_x, self.win_y, self.win_w, self.win_h)
        }
    }
}

fn draw_rounded_rect(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    color: u32,
) {
    for dy in 0..h {
        for dx in 0..w {
            if is_inside_rounded(dx, dy, w, h, radius) {
                display::put_pixel(fb, x + dx, y + dy, color);
            }
        }
    }
}

fn draw_rounded_rect_outline(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    thickness: usize,
    color: u32,
) {
    for dy in 0..h {
        for dx in 0..w {
            let inside = is_inside_rounded(dx, dy, w, h, radius);
            let inside_inner = is_inside_rounded(
                dx + thickness,
                dy + thickness,
                w.saturating_sub(2 * thickness),
                h.saturating_sub(2 * thickness),
                radius.saturating_sub(thickness),
            );
            if inside && !inside_inner {
                display::put_pixel(fb, x + dx, y + dy, color);
            }
        }
    }
}

fn is_inside_rounded(dx: usize, dy: usize, w: usize, h: usize, r: usize) -> bool {
    if dx < r && dy < r {
        return (dx as isize - r as isize).pow(2) + (dy as isize - r as isize).pow(2)
            <= (r * r) as isize;
    }
    if dx >= w.saturating_sub(r) && dy < r {
        return (dx as isize - (w - r) as isize).pow(2) + (dy as isize - r as isize).pow(2)
            <= (r * r) as isize;
    }
    if dx < r && dy >= h.saturating_sub(r) {
        return (dx as isize - r as isize).pow(2) + (dy as isize - (h - r) as isize).pow(2)
            <= (r * r) as isize;
    }
    if dx >= w.saturating_sub(r) && dy >= h.saturating_sub(r) {
        return (dx as isize - (w - r) as isize).pow(2) + (dy as isize - (h - r) as isize).pow(2)
            <= (r * r) as isize;
    }
    true
}

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let w = 290;
    let h = 452;
    let x = 20;
    let y = fb.height.saturating_sub(h + BAR_H + 20);
    (x, y, w, h)
}
