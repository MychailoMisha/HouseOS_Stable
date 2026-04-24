use crate::display::{self, Framebuffer};
use crate::fat32::{DirEntry, Fat32};
use crate::system;
use crate::window;

const LINE_HEIGHT: usize = 18;
const PAD: usize = 12;
const TOOLBAR_H: usize = 30;
const SCROLL_W: usize = 14;
const MAX_ENTRIES: usize = 128;
const MAX_PATH: usize = 12;
const MAX_RECYCLE: usize = 96;
const MAX_NAME: usize = 24;

const ENTRY_BACK: u8 = 0;
const ENTRY_DIR: u8 = 1;
const ENTRY_FILE: u8 = 2;

const REC_NONE: u8 = 0;
const REC_BIN: u8 = 1;
const REC_PURGED: u8 = 2;

#[derive(Copy, Clone, PartialEq)]
enum ExplorerView {
    Root,
    Bin,
}

#[derive(Copy, Clone, PartialEq)]
pub enum ExplorerAction {
    None,
    OpenTextFile {
        name: [u8; MAX_NAME],
        name_len: usize,
        cluster: u32,
        size: u32,
    },
}

#[derive(Copy, Clone)]
struct FileEntry {
    name: [u8; MAX_NAME],
    name_len: usize,
    kind: u8,
    cluster: u32,
    size: u32,
    parent_cluster: u32,
    recycle_idx: usize,
}

impl FileEntry {
    const EMPTY: FileEntry = FileEntry {
        name: [0u8; MAX_NAME],
        name_len: 0,
        kind: ENTRY_FILE,
        cluster: 0,
        size: 0,
        parent_cluster: 0,
        recycle_idx: usize::MAX,
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
    name: [u8; MAX_NAME],
    len: usize,
}

impl PathItem {
    const EMPTY: PathItem = PathItem {
        name: [0u8; MAX_NAME],
        len: 0,
    };
}

#[derive(Copy, Clone)]
struct RecycleEntry {
    state: u8,
    name: [u8; MAX_NAME],
    name_len: usize,
    kind: u8,
    cluster: u32,
    size: u32,
    parent_cluster: u32,
}

impl RecycleEntry {
    const EMPTY: RecycleEntry = RecycleEntry {
        state: REC_NONE,
        name: [0u8; MAX_NAME],
        name_len: 0,
        kind: ENTRY_FILE,
        cluster: 0,
        size: 0,
        parent_cluster: 0,
    };
}

struct Layout {
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
    list_h: usize,
    max_lines: usize,
    back_btn: (usize, usize, usize, usize),
    action_btn: (usize, usize, usize, usize),
    purge_btn: (usize, usize, usize, usize),
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
    scroll_offset: usize,
    entries_dirty: bool,
    selected_entry: Option<usize>,
    recycle: [RecycleEntry; MAX_RECYCLE],
    action: ExplorerAction,
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
            scroll_offset: 0,
            entries_dirty: true,
            selected_entry: None,
            recycle: [RecycleEntry::EMPTY; MAX_RECYCLE],
            action: ExplorerAction::None,
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
        self.selected_entry = None;
        self.action = ExplorerAction::None;
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
        self.selected_entry = None;
        self.action = ExplorerAction::None;
    }

    pub fn hide(&mut self, _fb: &Framebuffer) {
        self.visible = false;
    }

    pub fn take_action(&mut self) -> ExplorerAction {
        core::mem::replace(&mut self.action, ExplorerAction::None)
    }

    pub fn handle_click(&mut self, fb: &Framebuffer, x: usize, y: usize) -> bool {
        if !self.visible {
            return false;
        }

        let layout = self.layout(fb, self.content_rect(fb));

        let sidebar_x = layout.content_x + 1;
        let sidebar_y = layout.body_y + PAD;
        let sidebar_h = layout.body_h.saturating_sub(PAD * 2);
        if hit(x, y, sidebar_x, sidebar_y, layout.sidebar_w, sidebar_h) {
            let row = (y.saturating_sub(sidebar_y + LINE_HEIGHT + 4)) / LINE_HEIGHT;
            if row == 0 {
                self.view = ExplorerView::Root;
                self.entries_dirty = true;
                self.selected_entry = None;
                self.redraw(fb);
                return true;
            }
            if row == 1 {
                self.view = ExplorerView::Bin;
                self.entries_dirty = true;
                self.selected_entry = None;
                self.redraw(fb);
                return true;
            }
        }

        if hit(x, y, layout.back_btn.0, layout.back_btn.1, layout.back_btn.2, layout.back_btn.3) {
            if self.view == ExplorerView::Root {
                if self.depth > 0 {
                    self.depth -= 1;
                    self.current_cluster = self.cluster_stack[self.depth];
                    self.entries_dirty = true;
                    self.selected_entry = None;
                    self.redraw(fb);
                }
            } else {
                self.view = ExplorerView::Root;
                self.entries_dirty = true;
                self.selected_entry = None;
                self.redraw(fb);
            }
            return true;
        }

        if hit(
            x,
            y,
            layout.action_btn.0,
            layout.action_btn.1,
            layout.action_btn.2,
            layout.action_btn.3,
        ) {
            if self.view == ExplorerView::Root {
                if let Some(idx) = self.selected_entry {
                    if idx < self.entry_count {
                        let e = self.entries[idx];
                        if e.kind != ENTRY_BACK {
                            if self.recycle_move(e) {
                                self.entries_dirty = true;
                                self.selected_entry = None;
                                self.redraw(fb);
                            }
                        }
                    }
                }
            } else if let Some(idx) = self.selected_entry {
                if idx < self.entry_count {
                    let e = self.entries[idx];
                    if e.recycle_idx < MAX_RECYCLE {
                        self.recycle[e.recycle_idx].state = REC_NONE;
                        self.entries_dirty = true;
                        self.selected_entry = None;
                        self.redraw(fb);
                    }
                }
            }
            return true;
        }

        if self.view == ExplorerView::Bin
            && hit(
                x,
                y,
                layout.purge_btn.0,
                layout.purge_btn.1,
                layout.purge_btn.2,
                layout.purge_btn.3,
            )
        {
            if let Some(idx) = self.selected_entry {
                if idx < self.entry_count {
                    let e = self.entries[idx];
                    if e.recycle_idx < MAX_RECYCLE {
                        self.recycle[e.recycle_idx].state = REC_PURGED;
                        self.entries_dirty = true;
                        self.selected_entry = None;
                        self.redraw(fb);
                    }
                }
            }
            return true;
        }

        let scroll_x = layout.list_x + layout.list_w + 4;
        let scroll_y = layout.list_start_y;
        let scroll_h = layout.list_h;
        if x >= scroll_x && x < scroll_x + SCROLL_W && y >= scroll_y && y < scroll_y + scroll_h {
            let max_scroll = self.entry_count.saturating_sub(layout.max_lines);
            if max_scroll > 0 {
                let track_h = scroll_h.saturating_sub(2 * SCROLL_W);
                if track_h > 0 {
                    let ratio = (y.saturating_sub(scroll_y + SCROLL_W)) as f32 / track_h as f32;
                    self.scroll_offset = (ratio * max_scroll as f32) as usize;
                    if self.scroll_offset > max_scroll {
                        self.scroll_offset = max_scroll;
                    }
                    self.selected_entry = None;
                    self.redraw(fb);
                }
            }
            return true;
        }

        if x >= layout.list_x
            && x < layout.list_x + layout.list_w
            && y >= layout.list_start_y
            && y < layout.list_start_y + layout.list_h
        {
            let row = (y - layout.list_start_y) / LINE_HEIGHT;
            if row < self.visible_count {
                let idx = self.scroll_offset + row;
                if idx < self.entry_count {
                    let was_selected = self.selected_entry == Some(idx);
                    let entry = self.entries[idx];
                    self.selected_entry = Some(idx);
                    self.redraw(fb);

                    if self.view == ExplorerView::Root {
                        if entry.kind == ENTRY_BACK {
                            if self.depth > 0 {
                                self.depth -= 1;
                                self.current_cluster = self.cluster_stack[self.depth];
                                self.entries_dirty = true;
                                self.selected_entry = None;
                                self.redraw(fb);
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
                                self.selected_entry = None;
                                self.redraw(fb);
                            }
                        } else if entry.kind == ENTRY_FILE && was_selected && is_text_file(entry.name, entry.name_len) {
                            self.action = ExplorerAction::OpenTextFile {
                                name: entry.name,
                                name_len: entry.name_len,
                                cluster: entry.cluster,
                                size: entry.size,
                            };
                        }
                    }
                    return true;
                }
            }
        }

        false
    }

    pub fn redraw(&mut self, fb: &Framebuffer) {
        if !self.visible {
            return;
        }

        let (x, y, w, h) = self.rect(fb);
        let ui = system::ui_settings();
        let accent = ui.accent;
        let is_dark = ui.dark;

        let title: &[u8] = if self.view == ExplorerView::Bin {
            b"Recycle Bin"
        } else {
            b"File Explorer"
        };
        let chrome = window::draw_window(fb, x, y, w, h, title);

        fill_vertical_gradient(
            fb,
            chrome.content_x,
            chrome.content_y,
            chrome.content_w,
            chrome.content_h,
            if is_dark { 0x001E1E1E } else { 0x00FFFFFF },
            if is_dark { 0x00181818 } else { 0x00F7FAFF },
        );

        let layout = self.layout(
            fb,
            (
                chrome.content_x,
                chrome.content_y,
                chrome.content_w,
                chrome.content_h,
            ),
        );

        fill_vertical_gradient(
            fb,
            layout.content_x,
            layout.toolbar_y,
            layout.content_w,
            TOOLBAR_H,
            if is_dark { 0x00313131 } else { 0x00F8FBFF },
            if is_dark { 0x002A2A2A } else { 0x00EDF3FC },
        );
        display::fill_rect(
            fb,
            layout.content_x,
            layout.toolbar_y + TOOLBAR_H.saturating_sub(1),
            layout.content_w,
            1,
            if is_dark { 0x00444444 } else { 0x00D6E2F0 },
        );

        let mut writer = crate::TextWriter::new(*fb);
        let text_color = if is_dark { 0x00F3F5F8 } else { 0x00121B29 };
        let detail = if is_dark { 0x00B7C0CC } else { 0x004D5D72 };

        draw_button(
            fb,
            &mut writer,
            layout.back_btn,
            b"Back",
            is_dark,
            text_color,
        );

        let action_label: &[u8] = if self.view == ExplorerView::Bin {
            b"Restore"
        } else {
            b"Delete"
        };
        draw_button(
            fb,
            &mut writer,
            layout.action_btn,
            action_label,
            is_dark,
            text_color,
        );

        if self.view == ExplorerView::Bin {
            draw_button(
                fb,
                &mut writer,
                layout.purge_btn,
                b"Delete now",
                is_dark,
                text_color,
            );
        }

        writer.set_color(detail);
        writer.set_pos(layout.action_btn.0 + layout.action_btn.2 + PAD, layout.toolbar_y + 8);
        if self.view == ExplorerView::Bin {
            writer.write_bytes(b"Items in recycle bin");
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

        fill_vertical_gradient(
            fb,
            layout.content_x,
            layout.body_y,
            layout.sidebar_w,
            layout.body_h,
            if is_dark { 0x00252525 } else { 0x00F9FBFF },
            if is_dark { 0x00212121 } else { 0x00EEF4FC },
        );

        writer.set_color(detail);
        writer.set_pos(layout.content_x + PAD, layout.body_y + PAD);
        writer.write_bytes(b"Places");

        let item_y = layout.body_y + PAD + LINE_HEIGHT + 4;
        draw_sidebar_item(
            fb,
            &mut writer,
            layout.content_x + PAD,
            item_y,
            layout.sidebar_w.saturating_sub(PAD * 2),
            b"Files",
            self.view == ExplorerView::Root,
            accent,
            text_color,
        );
        draw_sidebar_item(
            fb,
            &mut writer,
            layout.content_x + PAD,
            item_y + LINE_HEIGHT,
            layout.sidebar_w.saturating_sub(PAD * 2),
            b"Recycle Bin",
            self.view == ExplorerView::Bin,
            accent,
            text_color,
        );

        writer.set_color(detail);
        writer.set_pos(layout.list_x, layout.list_header_y);
        writer.write_bytes(b"Name");
        writer.set_pos(layout.list_x + layout.list_w.saturating_sub(64), layout.list_header_y);
        writer.write_bytes(b"Size");

        if self.entries_dirty {
            self.rebuild_entries(layout.max_lines);
            self.entries_dirty = false;
            self.scroll_offset = 0;
            self.selected_entry = None;
        } else {
            let max_scroll = self.entry_count.saturating_sub(layout.max_lines);
            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }
            self.visible_count = (self.entry_count.saturating_sub(self.scroll_offset)).min(layout.max_lines);
        }

        for i in 0..self.visible_count {
            let entry_idx = self.scroll_offset + i;
            if entry_idx >= self.entry_count {
                break;
            }
            let entry = self.entries[entry_idx];
            let row_y = layout.list_start_y + i * LINE_HEIGHT;

            let row_bg = if Some(entry_idx) == self.selected_entry {
                if is_dark { 0x40284A7A } else { 0x40D7E9FF }
            } else if (i & 1) == 1 {
                if is_dark { 0x002E2E2E } else { 0x00F6FAFF }
            } else {
                0
            };
            if row_bg != 0 {
                display::fill_rect(fb, layout.list_x, row_y, layout.list_w, LINE_HEIGHT, row_bg);
            }

            writer.set_color(if entry.kind == ENTRY_DIR { accent } else { text_color });
            writer.set_pos(layout.list_x + 4, row_y + 3);

            if entry.kind == ENTRY_BACK {
                writer.write_bytes(b"..");
            } else if entry.kind == ENTRY_DIR {
                writer.write_bytes(b"[DIR] ");
                writer.write_bytes(&entry.name[..entry.name_len]);
                writer.write_bytes(b"/");
            } else {
                writer.write_bytes(b"      ");
                writer.write_bytes(&entry.name[..entry.name_len]);
            }

            if entry.kind != ENTRY_DIR && entry.kind != ENTRY_BACK {
                let mut buf = [0u8; 12];
                let len = write_u32(&mut buf, entry.size);
                if len > 0 {
                    writer.set_color(detail);
                    let sx = layout.list_x + layout.list_w.saturating_sub(len * 8 + 8);
                    writer.set_pos(sx, row_y + 3);
                    writer.write_bytes(&buf[..len]);
                }
            }
        }

        let max_scroll = self.entry_count.saturating_sub(layout.max_lines);
        if max_scroll > 0 {
            let scroll_x = layout.list_x + layout.list_w + 4;
            let scroll_y = layout.list_start_y;
            let scroll_h = layout.list_h;
            display::fill_rect(
                fb,
                scroll_x,
                scroll_y,
                SCROLL_W,
                scroll_h,
                if is_dark { 0x00323232 } else { 0x00E1EAF5 },
            );
            let thumb_h = ((layout.max_lines as f32 / self.entry_count as f32) * scroll_h as f32) as usize;
            let thumb_h = thumb_h.max(16).min(scroll_h.max(16));
            let thumb_y = scroll_y
                + ((self.scroll_offset as f32 / max_scroll as f32)
                    * (scroll_h.saturating_sub(thumb_h)) as f32) as usize;
            display::fill_rect(
                fb,
                scroll_x,
                thumb_y,
                SCROLL_W,
                thumb_h,
                if is_dark { 0x00777F8A } else { 0x0093A9C0 },
            );
        }

        if self.entry_count == 0 {
            writer.set_color(detail);
            writer.set_pos(layout.list_x, layout.list_start_y + 4);
            if self.view == ExplorerView::Bin {
                writer.write_bytes(b"Recycle Bin is empty.");
            } else {
                writer.write_bytes(b"(empty folder)");
            }
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

    pub fn set_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        self.win_x = x;
        self.win_y = y;
        self.win_w = w;
        self.win_h = h;
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

    fn recycle_move(&mut self, entry: FileEntry) -> bool {
        for i in 0..MAX_RECYCLE {
            let rec = self.recycle[i];
            if rec.state == REC_NONE {
                continue;
            }
            if rec.parent_cluster == entry.parent_cluster
                && rec.cluster == entry.cluster
                && rec.name_len == entry.name_len
                && names_equal(rec.name, entry.name, rec.name_len)
            {
                if rec.state == REC_PURGED {
                    return false;
                }
                self.recycle[i].state = REC_BIN;
                return true;
            }
        }

        for i in 0..MAX_RECYCLE {
            if self.recycle[i].state == REC_NONE {
                self.recycle[i] = RecycleEntry {
                    state: REC_BIN,
                    name: entry.name,
                    name_len: entry.name_len,
                    kind: entry.kind,
                    cluster: entry.cluster,
                    size: entry.size,
                    parent_cluster: entry.parent_cluster,
                };
                return true;
            }
        }
        false
    }

    fn is_hidden(&self, parent_cluster: u32, name: [u8; MAX_NAME], name_len: usize, cluster: u32) -> bool {
        for i in 0..MAX_RECYCLE {
            let rec = self.recycle[i];
            if rec.state == REC_NONE {
                continue;
            }
            if rec.parent_cluster == parent_cluster
                && rec.cluster == cluster
                && rec.name_len == name_len
                && names_equal(rec.name, name, name_len)
            {
                return true;
            }
        }
        false
    }

    fn rebuild_entries(&mut self, max_lines: usize) {
        self.entry_count = 0;
        self.visible_count = 0;

        if self.view == ExplorerView::Bin {
            for i in 0..MAX_RECYCLE {
                let rec = self.recycle[i];
                if rec.state != REC_BIN {
                    continue;
                }
                if self.entry_count >= MAX_ENTRIES {
                    break;
                }
                self.entries[self.entry_count] = FileEntry {
                    name: rec.name,
                    name_len: rec.name_len,
                    kind: rec.kind,
                    cluster: rec.cluster,
                    size: rec.size,
                    parent_cluster: rec.parent_cluster,
                    recycle_idx: i,
                };
                self.entry_count += 1;
            }
            self.visible_count = self.entry_count.min(max_lines);
            return;
        }

        if self.depth > 0 && self.entry_count < MAX_ENTRIES {
            self.entries[0] = FileEntry::back();
            self.entry_count = 1;
        }

        let fs_img = self.fs_img;
        if let Some(fs) = fs_img.and_then(Fat32::new) {
            let mut dir_buf = [DirEntry::EMPTY; MAX_ENTRIES];
            let count = fs.list_dir(self.current_cluster, &mut dir_buf);
            for i in 0..count {
                if self.entry_count >= MAX_ENTRIES {
                    break;
                }
                let d = dir_buf[i];
                if self.is_hidden(self.current_cluster, d.name, d.name_len, d.cluster) {
                    continue;
                }
                self.entries[self.entry_count] = FileEntry {
                    name: d.name,
                    name_len: d.name_len,
                    kind: if d.is_dir { ENTRY_DIR } else { ENTRY_FILE },
                    cluster: d.cluster,
                    size: d.size,
                    parent_cluster: self.current_cluster,
                    recycle_idx: usize::MAX,
                };
                self.entry_count += 1;
            }
        }

        self.visible_count = self.entry_count.min(max_lines);
    }

    fn layout(&self, _fb: &Framebuffer, content: (usize, usize, usize, usize)) -> Layout {
        let content_x = content.0;
        let content_y = content.1;
        let content_w = content.2;
        let content_h = content.3;
        let toolbar_y = content_y;
        let body_y = content_y + TOOLBAR_H;
        let body_h = content_h.saturating_sub(TOOLBAR_H);
        let sidebar_w = (content_w / 4).max(120).min(content_w.saturating_sub(200));
        let list_x = content_x + sidebar_w + PAD;
        let list_header_y = body_y + PAD;
        let list_start_y = list_header_y + LINE_HEIGHT;
        let list_w = content_w.saturating_sub(sidebar_w + PAD * 2 + SCROLL_W + 4);
        let list_h = body_h.saturating_sub(PAD * 2 + LINE_HEIGHT);
        let max_lines = if LINE_HEIGHT == 0 { 0 } else { list_h / LINE_HEIGHT };

        let btn_h = TOOLBAR_H.saturating_sub(6);
        let back_btn = (content_x + PAD, toolbar_y + 3, 64, btn_h);
        let action_btn = (back_btn.0 + back_btn.2 + 8, toolbar_y + 3, 78, btn_h);
        let purge_btn = (action_btn.0 + action_btn.2 + 8, toolbar_y + 3, 104, btn_h);

        Layout {
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
            list_h,
            max_lines,
            back_btn,
            action_btn,
            purge_btn,
        }
    }

    fn content_rect(&self, fb: &Framebuffer) -> (usize, usize, usize, usize) {
        let (x, y, w, h) = self.rect(fb);
        let content_x = x + 2;
        let content_y = y + window::HEADER_H + 2;
        let content_w = w.saturating_sub(4);
        let content_h = h.saturating_sub(window::HEADER_H + 4);
        (content_x, content_y, content_w, content_h)
    }
}

fn draw_button(
    fb: &Framebuffer,
    writer: &mut crate::TextWriter,
    rect: (usize, usize, usize, usize),
    label: &[u8],
    is_dark: bool,
    text_color: u32,
) {
    fill_vertical_gradient(
        fb,
        rect.0,
        rect.1,
        rect.2,
        rect.3,
        if is_dark { 0x00494848 } else { 0x00EAF0F8 },
        if is_dark { 0x003F3F3F } else { 0x00D8E2EE },
    );
    writer.set_color(text_color);
    let text_w = label.len() * 8;
    let tx = rect.0 + rect.2.saturating_sub(text_w) / 2;
    writer.set_pos(tx, rect.1 + 6);
    writer.write_bytes(label);
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
        let active_bg = blend_rgb(accent, 0x00FFFFFF, 34);
        display::fill_rect(
            fb,
            x.saturating_sub(4),
            y.saturating_sub(2),
            w + 8,
            LINE_HEIGHT,
            active_bg,
        );
        writer.set_color(0x00FFFFFF);
    } else {
        writer.set_color(text);
    }
    writer.set_pos(x, y);
    writer.write_bytes(label);
}

fn is_text_file(name: [u8; MAX_NAME], len: usize) -> bool {
    if len < 4 {
        return false;
    }
    let mut dot = usize::MAX;
    for i in 0..len {
        if name[i] == b'.' {
            dot = i;
        }
    }
    if dot == usize::MAX || dot + 1 >= len {
        return false;
    }
    let ext = &name[dot + 1..len];
    eq_ascii_ci(ext, b"TXT")
        || eq_ascii_ci(ext, b"LOG")
        || eq_ascii_ci(ext, b"INI")
        || eq_ascii_ci(ext, b"CFG")
        || eq_ascii_ci(ext, b"CSV")
}

fn eq_ascii_ci(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for i in 0..a.len() {
        let ac = ascii_lower(a[i]);
        let bc = ascii_lower(b[i]);
        if ac != bc {
            return false;
        }
    }
    true
}

fn ascii_lower(b: u8) -> u8 {
    if (b'A'..=b'Z').contains(&b) {
        b + 32
    } else {
        b
    }
}

fn names_equal(a: [u8; MAX_NAME], b: [u8; MAX_NAME], len: usize) -> bool {
    for i in 0..len {
        if a[i] != b[i] {
            return false;
        }
    }
    true
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
    let w = (fb.width * 3 / 4).min(800).max(400);
    let h = (fb.height * 3 / 4).min(600).max(300);
    let x = (fb.width.saturating_sub(w)) / 2;
    let y = (fb.height.saturating_sub(h)) / 2;
    (x, y, w, h)
}

fn hit(px: usize, py: usize, x: usize, y: usize, w: usize, h: usize) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}

fn fill_vertical_gradient(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    top: u32,
    bottom: u32,
) {
    if w == 0 || h == 0 {
        return;
    }
    if h == 1 {
        display::fill_rect(fb, x, y, w, 1, top);
        return;
    }
    let den = (h - 1) as u32;
    for row in 0..h {
        let c = lerp_rgb(top, bottom, row as u32, den);
        display::fill_rect(fb, x, y + row, w, 1, c);
    }
}

fn lerp_rgb(a: u32, b: u32, num: u32, den: u32) -> u32 {
    if den == 0 {
        return a;
    }
    let ar = ((a >> 16) & 0xFF) as u32;
    let ag = ((a >> 8) & 0xFF) as u32;
    let ab = (a & 0xFF) as u32;
    let br = ((b >> 16) & 0xFF) as u32;
    let bg = ((b >> 8) & 0xFF) as u32;
    let bb = (b & 0xFF) as u32;
    let r = (ar * (den - num) + br * num) / den;
    let g = (ag * (den - num) + bg * num) / den;
    let b = (ab * (den - num) + bb * num) / den;
    (r << 16) | (g << 8) | b
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
