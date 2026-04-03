use crate::display::{self, Framebuffer};
use crate::fat32::{DirEntry, Fat32};
use crate::system;
use crate::window;

const LINE_HEIGHT: usize = 12;
const PAD: usize = 10;
const TOOLBAR_H: usize = 18;
const MAX_ENTRIES: usize = 96;
const MAX_PATH: usize = 12;

const ENTRY_BACK: u8 = 0;
const ENTRY_DIR: u8 = 1;
const ENTRY_FILE: u8 = 2;

#[derive(Copy, Clone, PartialEq)]
enum ExplorerView {
    Root,
    Bin,
}

#[derive(Copy, Clone)]
struct FileEntry {
    name: [u8; 24],
    name_len: usize,
    kind: u8,
    cluster: u32,
    size: u32,
}

impl FileEntry {
    const EMPTY: FileEntry = FileEntry {
        name: [0u8; 24],
        name_len: 0,
        kind: ENTRY_FILE,
        cluster: 0,
        size: 0,
    };

    fn back() -> Self {
        let mut e = Self::EMPTY;
        e.kind = ENTRY_BACK;
        e.name[0] = b'<';
        e.name[1] = b'-';
        e.name[2] = b' ';
        e.name[3] = b'B';
        e.name[4] = b'a';
        e.name[5] = b'c';
        e.name[6] = b'k';
        e.name_len = 7;
        e
    }
}

#[derive(Copy, Clone)]
struct PathItem {
    name: [u8; 24],
    len: usize,
}

impl PathItem {
    const EMPTY: PathItem = PathItem {
        name: [0u8; 24],
        len: 0,
    };
}

struct Layout {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    content_x: usize,
    content_y: usize,
    content_w: usize,
    content_h: usize,
    toolbar_y: usize,
    body_y: usize,
    body_h: usize,
    sidebar_w: usize,
    list_x: usize,
    list_header_y: usize,
    list_start_y: usize,
    list_w: usize,
    max_lines: usize,
}

pub struct Explorer {
    visible: bool,
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,
    view: ExplorerView,
    fs_img: Option<crate::ModuleRange>,
    current_cluster: u32,
    path: [PathItem; MAX_PATH],
    cluster_stack: [u32; MAX_PATH],
    depth: usize,
    entries: [FileEntry; MAX_ENTRIES],
    entry_count: usize,
    visible_count: usize,
    entries_dirty: bool,
}

impl Explorer {
    pub fn new(_fb: Framebuffer, fs_img: Option<crate::ModuleRange>) -> Self {
        Self {
            visible: false,
            win_x: 0,
            win_y: 0,
            win_w: 0,
            win_h: 0,
            view: ExplorerView::Root,
            fs_img,
            current_cluster: 2,
            path: [PathItem::EMPTY; MAX_PATH],
            cluster_stack: [0u32; MAX_PATH],
            depth: 0,
            entries: [FileEntry::EMPTY; MAX_ENTRIES],
            entry_count: 0,
            visible_count: 0,
            entries_dirty: true,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, fb: &Framebuffer) {
        self.visible = true;
        if self.win_w == 0 || self.win_h == 0 {
            let (x, y, w, h) = calc_rect(fb);
            self.win_x = x;
            self.win_y = y;
            self.win_w = w;
            self.win_h = h;
        }
        self.view = ExplorerView::Root;
        self.reset_path();
        self.entries_dirty = true;
    }

    pub fn show_bin(&mut self, fb: &Framebuffer) {
        self.visible = true;
        if self.win_w == 0 || self.win_h == 0 {
            let (x, y, w, h) = calc_rect(fb);
            self.win_x = x;
            self.win_y = y;
            self.win_w = w;
            self.win_h = h;
        }
        self.view = ExplorerView::Bin;
        self.entries_dirty = true;
    }

    pub fn hide(&mut self, _fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        self.visible = false;
    }

    pub fn handle_click(&mut self, fb: &Framebuffer, x: usize, y: usize) -> bool {
        if !self.visible {
            return false;
        }
        let layout = self.layout(fb, self.content_rect(fb));

        let sidebar_x = layout.x + 1;
        let sidebar_y = layout.body_y + PAD;
        let sidebar_h = layout.body_h.saturating_sub(PAD * 2);
        if hit(x, y, sidebar_x, sidebar_y, layout.sidebar_w, sidebar_h) {
            let row = (y.saturating_sub(sidebar_y)) / LINE_HEIGHT;
            if row == 0 {
                self.view = ExplorerView::Root;
                self.reset_path();
                self.entries_dirty = true;
                self.redraw(fb);
                return true;
            }
            if row == 1 {
                self.view = ExplorerView::Bin;
                self.entries_dirty = true;
                self.redraw(fb);
                return true;
            }
        }

        if self.view == ExplorerView::Root {
            let list_y = layout.list_start_y;
            if x >= layout.list_x && x < layout.x + layout.w {
                if y >= list_y {
                    let row = (y - list_y) / LINE_HEIGHT;
                    if row < self.visible_count {
                        let entry = self.entries[row];
                        if entry.kind == ENTRY_BACK {
                            if self.depth > 0 {
                                self.depth -= 1;
                                self.current_cluster = self.cluster_stack[self.depth];
                                self.entries_dirty = true;
                                self.redraw(fb);
                                return true;
                            }
                        } else if entry.kind == ENTRY_DIR && entry.cluster >= 2 {
                            if self.depth < MAX_PATH {
                                self.cluster_stack[self.depth] = self.current_cluster;
                                self.path[self.depth] = PathItem {
                                    name: entry.name,
                                    len: entry.name_len,
                                };
                                self.depth += 1;
                                self.current_cluster = entry.cluster;
                                self.entries_dirty = true;
                                self.redraw(fb);
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn reset_path(&mut self) {
        self.depth = 0;
        self.current_cluster = 2;
        self.entries_dirty = true;
        let fs_img = self.fs_img;
        if let Some(fs) = fs_img.and_then(Fat32::new) {
            self.current_cluster = fs.root_cluster();
        }
    }

    pub fn redraw(&mut self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }
        let (x, y, w, h) = self.rect(fb);
        let chrome = window::draw_window(
            fb,
            x,
            y,
            w,
            h,
            match self.view {
                ExplorerView::Root => b"Explorer",
                ExplorerView::Bin => b"Recycle Bin",
            },
        );
        let layout = self.layout(fb, (chrome.content_x, chrome.content_y, chrome.content_w, chrome.content_h));
        let ui = system::ui_settings();
        let accent = ui.accent;
        let (toolbar, sidebar, list_alt, text, muted) = if ui.dark {
            (0x00262626, 0x00262626, 0x00292F36, 0x00E6E6E6, 0x00989898)
        } else {
            (0x00F3F3F3, 0x00F7F7F7, 0x00EEF5FF, 0x00111111, 0x006A6A6A)
        };

        display::fill_rect(
            fb,
            layout.content_x,
            layout.toolbar_y,
            layout.content_w,
            TOOLBAR_H,
            toolbar,
        );
        display::fill_rect(
            fb,
            layout.content_x,
            layout.body_y,
            layout.sidebar_w,
            layout.body_h,
            sidebar,
        );

        let mut writer = crate::TextWriter::new(*fb);
        writer.set_color(muted);
        writer.set_pos(layout.content_x + PAD, layout.toolbar_y + 4);
        writer.write_bytes(b"Path: ");
        writer.set_color(text);
        if self.view == ExplorerView::Bin {
            writer.write_bytes(b"Recycle Bin");
        } else {
            writer.write_bytes(b"C:\\");
            for i in 0..self.depth {
                let item = self.path[i];
                if item.len > 0 {
                    writer.write_bytes(&item.name[..item.len]);
                    if i + 1 < self.depth {
                        writer.write_bytes(b"\\");
                    }
                }
            }
        }

        let sidebar_x = layout.content_x + PAD;
        let sidebar_y = layout.body_y + PAD;
        writer.set_color(muted);
        writer.set_pos(sidebar_x, sidebar_y);
        writer.write_bytes(b"Places");

        let item_y = sidebar_y + LINE_HEIGHT + 4;
        draw_sidebar_item(
            fb,
            &mut writer,
            sidebar_x,
            item_y,
            layout.sidebar_w.saturating_sub(PAD * 2),
            b"Files",
            self.view == ExplorerView::Root,
            accent,
            text,
        );
        draw_sidebar_item(
            fb,
            &mut writer,
            sidebar_x,
            item_y + LINE_HEIGHT,
            layout.sidebar_w.saturating_sub(PAD * 2),
            b"Recycle Bin",
            self.view == ExplorerView::Bin,
            accent,
            text,
        );

        writer.set_color(muted);
        writer.set_pos(layout.list_x, layout.list_header_y);
        writer.write_bytes(b"Name");
        writer.set_pos(layout.list_x + layout.list_w.saturating_sub(40), layout.list_header_y);
        writer.write_bytes(b"Size");

        if self.view == ExplorerView::Root {
            let fs_img = self.fs_img;
        if let Some(fs) = fs_img.and_then(Fat32::new) {
                if self.entries_dirty {
                    self.rebuild_entries(&fs, layout.max_lines);
                    self.entries_dirty = false;
                } else {
                    self.visible_count = self.entry_count.min(layout.max_lines);
                }
                for i in 0..self.visible_count {
                    let row_y = layout.list_start_y + i * LINE_HEIGHT;
                    if (i & 1) == 1 {
                        display::fill_rect(
                            fb,
                            layout.list_x.saturating_sub(2),
                            row_y.saturating_sub(2),
                            layout.list_w + 4,
                            LINE_HEIGHT,
                            list_alt,
                        );
                    }
                    let entry = self.entries[i];
                    if entry.kind == ENTRY_BACK {
                        writer.set_color(muted);
                        writer.set_pos(layout.list_x, row_y);
                        writer.write_bytes(&entry.name[..entry.name_len]);
                        continue;
                    }
                    let is_dir = entry.kind == ENTRY_DIR;
                    writer.set_color(if is_dir { accent } else { text });
                    writer.set_pos(layout.list_x, row_y);
                    if is_dir {
                        writer.write_bytes(b"[DIR] ");
                    } else {
                        writer.write_bytes(b"      ");
                    }
                    writer.write_bytes(&entry.name[..entry.name_len]);
                    if is_dir {
                        writer.write_bytes(b"/");
                    } else {
                        let mut buf = [0u8; 12];
                        let len = write_u32(&mut buf, entry.size);
                        if len > 0 {
                            let size_x =
                                layout.list_x + layout.list_w.saturating_sub(len * 8);
                            writer.set_color(muted);
                            writer.set_pos(size_x, row_y);
                            writer.write_bytes(&buf[..len]);
                        }
                    }
                }
                if self.entry_count == 0 {
                    writer.set_color(muted);
                    writer.set_pos(layout.list_x, layout.list_start_y);
                    writer.write_bytes(b"(empty)");
                }
            } else {
                writer.set_color(muted);
                writer.set_pos(layout.list_x, layout.list_start_y);
                writer.write_bytes(b"(no FAT32 image)");
            }
        } else {
            writer.set_color(muted);
            writer.set_pos(layout.list_x, layout.list_start_y);
            writer.write_bytes(b"(empty)");
        }
    }

    fn rebuild_entries(&mut self, fs: &Fat32, max_lines: usize) {
        self.entry_count = 0;
        self.visible_count = 0;

        if self.depth > 0 && self.entry_count < MAX_ENTRIES {
            self.entries[0] = FileEntry::back();
            self.entry_count = 1;
        }

        let mut dir_buf = [DirEntry::EMPTY; MAX_ENTRIES];
        let count = fs.list_dir(self.current_cluster, &mut dir_buf);
        for i in 0..count {
            if self.entry_count >= MAX_ENTRIES {
                break;
            }
            let d = dir_buf[i];
            self.entries[self.entry_count] = FileEntry {
                name: d.name,
                name_len: d.name_len,
                kind: if d.is_dir { ENTRY_DIR } else { ENTRY_FILE },
                cluster: d.cluster,
                size: d.size,
            };
            self.entry_count += 1;
        }

        if self.entry_count > max_lines {
            self.visible_count = max_lines;
        } else {
            self.visible_count = self.entry_count;
        }
    }

    fn layout(&self, fb: &Framebuffer, content: (usize, usize, usize, usize)) -> Layout {
        let (x, y, w, h) = self.rect(fb);
        let content_x = content.0;
        let content_y = content.1;
        let content_w = content.2;
        let content_h = content.3;
        let toolbar_y = content_y;
        let body_y = content_y + TOOLBAR_H;
        let body_h = content_h.saturating_sub(TOOLBAR_H);
        let sidebar_w = (content_w / 3).max(150).min(content_w.saturating_sub(140));
        let list_x = content_x + sidebar_w + PAD;
        let list_header_y = body_y + PAD;
        let list_start_y = list_header_y + LINE_HEIGHT;
        let list_w = content_w.saturating_sub(sidebar_w + PAD * 2 + 2);
        let list_h = body_h.saturating_sub(PAD * 2 + LINE_HEIGHT);
        let max_lines = if LINE_HEIGHT == 0 {
            0
        } else {
            list_h / LINE_HEIGHT
        };
        Layout {
            x,
            y,
            w,
            h,
            content_x,
            content_y,
            content_w,
            content_h,
            toolbar_y,
            body_y,
            body_h,
            sidebar_w,
            list_x,
            list_header_y,
            list_start_y,
            list_w,
            max_lines,
        }
    }

    pub fn rect(&self, fb: &Framebuffer) -> (usize, usize, usize, usize) {
        if self.win_w == 0 || self.win_h == 0 {
            return calc_rect(fb);
        }
        (self.win_x, self.win_y, self.win_w, self.win_h)
    }

    pub fn set_pos(&mut self, x: usize, y: usize) {
        self.win_x = x;
        self.win_y = y;
    }

    fn content_rect(&self, fb: &Framebuffer) -> (usize, usize, usize, usize) {
        let (x, y, w, h) = self.rect(fb);
        let content_x = x + 1;
        let content_y = y + window::HEADER_H + 1;
        let content_w = w.saturating_sub(2);
        let content_h = h.saturating_sub(window::HEADER_H + 2);
        (content_x, content_y, content_w, content_h)
    }
}

fn draw_sidebar_item(
    fb: &Framebuffer,
    writer: &mut crate::TextWriter,
    x: usize,
    y: usize,
    w: usize,
    label: &[u8],
    active: bool,
    accent: u32,
    text: u32,
) {
    if active {
        display::fill_rect(fb, x.saturating_sub(4), y.saturating_sub(2), w + 8, LINE_HEIGHT, accent);
        writer.set_color(0x00FFFFFF);
    } else {
        writer.set_color(text);
    }
    writer.set_pos(x, y);
    writer.write_bytes(label);
}

fn write_u32(buf: &mut [u8], mut val: u32) -> usize {
    if buf.is_empty() {
        return 0;
    }
    if val == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 10];
    let mut n = 0usize;
    while val > 0 && n < tmp.len() {
        tmp[n] = (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    let mut out = 0usize;
    while n > 0 && out < buf.len() {
        n -= 1;
        buf[out] = b'0' + tmp[n];
        out += 1;
    }
    out
}

fn calc_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let w = fb.width / 2;
    let h = fb.height / 2;
    if w == 0 || h == 0 {
        return (0, 0, 0, 0);
    }
    let x = (fb.width - w) / 2;
    let y = (fb.height - h) / 2;
    (x, y, w, h)
}

fn hit(px: usize, py: usize, x: usize, y: usize, w: usize, h: usize) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}
