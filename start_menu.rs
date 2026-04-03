use crate::display::{self, Framebuffer};
use crate::system;

const LINE_HEIGHT: usize = 18;
const PAD: usize = 12;
const HEADER_H: usize = 28;
const CLOSE_SIZE: usize = 14;
const BAR_H: usize = 26;

#[derive(Copy, Clone)]
pub enum StartAction {
    OpenConsole,
    OpenExplorer,
    OpenClipboard,
    OpenBin,
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
        if !self.visible {
            return;
        }
        self.visible = false;
    }

    pub fn handle_click(
        &mut self,
        fb: &Framebuffer,
        x: usize,
        y: usize,
    ) -> Option<StartAction> {
        if !self.visible {
            return None;
        }
        let (wx, wy, ww, _) = self.rect(fb);
        let (cx, cy, cw, ch) = close_rect(wx, wy, ww);
        if hit(x, y, cx, cy, cw, ch) {
            self.hide(fb);
            return None;
        }
        let list_x = wx + PAD;
        let list_y = wy + HEADER_H + PAD;
        if x >= list_x && x < wx + ww {
            if y >= list_y {
                let row = (y - list_y) / LINE_HEIGHT;
                return match row {
                    0 => Some(StartAction::OpenConsole),
                    1 => Some(StartAction::OpenExplorer),
                    2 => Some(StartAction::OpenClipboard),
                    3 => Some(StartAction::OpenBin),
                    4 => Some(StartAction::ToggleTheme),
                    5 => Some(StartAction::Reboot),
                    6 => Some(StartAction::Shutdown),
                    _ => None,
                };
            }
        }
        None
    }

    pub fn refresh(&self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        self.redraw(fb);
    }

    fn redraw(&self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        let accent = ui.accent;
        let (border, bg, header_bg, header_text, text) = if ui.dark {
            (
                0x00363636,
                0x00212121,
                0x002A2A2A,
                0x00FFFFFF,
                0x00E6E6E6,
            )
        } else {
            (
                0x00D0D0D0,
                0x00FFFFFF,
                0x00F7F7F7,
                0x00000000,
                0x00111111,
            )
        };

        display::fill_rect(fb, x, y, w, h, bg);
        display::fill_rect(fb, x, y, w, 1, border);
        display::fill_rect(fb, x, y + h.saturating_sub(1), w, 1, border);
        display::fill_rect(fb, x, y, 1, h, border);
        display::fill_rect(fb, x + w.saturating_sub(1), y, 1, h, border);

        display::fill_rect(fb, x + 1, y + 1, w.saturating_sub(2), HEADER_H, header_bg);
        display::fill_rect(
            fb,
            x + 1,
            y + HEADER_H.saturating_sub(2),
            w.saturating_sub(2),
            2,
            accent,
        );
        let (cx, cy, cw, ch) = close_rect(x, y, w);
        display::fill_rect(fb, cx, cy, cw, ch, 0x00E81123);

        let mut writer = crate::TextWriter::new(*fb);
        writer.set_color(header_text);
        writer.set_pos(x + PAD, y + 8);
        writer.write_bytes(b"Start");
        writer.set_pos(cx + 4, cy + 3);
        writer.write_bytes(b"X");

        writer.set_color(text);
        let list_x = x + PAD;
        let list_y = y + HEADER_H + PAD;
        writer.set_pos(list_x, list_y);
        writer.write_bytes(b"Console");
        writer.set_pos(list_x, list_y + LINE_HEIGHT);
        writer.write_bytes(b"Explorer");
        writer.set_pos(list_x, list_y + LINE_HEIGHT * 2);
        writer.write_bytes(b"Clipboard");
        writer.set_pos(list_x, list_y + LINE_HEIGHT * 3);
        writer.write_bytes(b"Recycle Bin");
        let theme_label: &[u8] = if ui.dark {
            b"Theme: Dark"
        } else {
            b"Theme: Light"
        };
        writer.set_pos(list_x, list_y + LINE_HEIGHT * 4);
        writer.write_bytes(theme_label);
        writer.set_pos(list_x, list_y + LINE_HEIGHT * 5);
        writer.write_bytes(b"Restart");
        writer.set_pos(list_x, list_y + LINE_HEIGHT * 6);
        writer.write_bytes(b"Shutdown");
    }

    pub fn rect(&self, fb: &Framebuffer) -> (usize, usize, usize, usize) {
        if self.win_w == 0 || self.win_h == 0 {
            return calc_rect(fb);
        }
        (self.win_x, self.win_y, self.win_w, self.win_h)
    }
}

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let mut w = fb.width / 3;
    let mut h = fb.height / 3;
    if w < 220 {
        w = 220;
    }
    if w > 320 {
        w = 320;
    }
    if h < 220 {
        h = 220;
    }
    if h > 320 {
        h = 320;
    }
    let x = 16usize;
    let y = fb.height.saturating_sub(h + BAR_H + 10);
    (x, y, w, h)
}

fn close_rect(x: usize, y: usize, w: usize) -> (usize, usize, usize, usize) {
    let cx = x + w.saturating_sub(CLOSE_SIZE + 6);
    let cy = y + (HEADER_H.saturating_sub(CLOSE_SIZE)) / 2 + 1;
    (cx, cy, CLOSE_SIZE, CLOSE_SIZE)
}

fn hit(px: usize, py: usize, x: usize, y: usize, w: usize, h: usize) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}
