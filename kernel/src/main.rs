#![no_std]
#![no_main]

mod cursor;
mod console;
mod clipboard;
mod desktop;
mod explorer;
mod display;
mod fat32;
mod window;
mod input;
mod rtc;
mod start_menu;
mod status_bar;
mod system;
#[path = "../../drivers/mod.rs"]
mod drivers;

use core::arch::asm;
use core::panic::PanicInfo;
use display::Framebuffer;

const MB2_MAGIC: u32 = 0x36d76289;

const MB2_HEADER_MAGIC: u32 = 0xE85250D6;
const MB2_ARCH: u32 = 0;
const MB2_HEADER_LEN: u32 = 48;
const MB2_CHECKSUM: u32 =
    0u32.wrapping_sub(MB2_HEADER_MAGIC.wrapping_add(MB2_ARCH).wrapping_add(MB2_HEADER_LEN));
const MB2_FB_WIDTH: u32 = 1024;
const MB2_FB_HEIGHT: u32 = 768;
const MB2_FB_BPP: u32 = 32;

#[repr(C, align(8))]
struct Mb2Header([u32; 12]);

#[link_section = ".multiboot2"]
#[used]
static MULTIBOOT2_HEADER: Mb2Header = Mb2Header([
    MB2_HEADER_MAGIC,
    MB2_ARCH,
    MB2_HEADER_LEN,
    MB2_CHECKSUM,
    5,  // framebuffer tag type
    20, // framebuffer tag size
    MB2_FB_WIDTH,
    MB2_FB_HEIGHT,
    MB2_FB_BPP,
    0, // padding for 8-byte alignment
    0, // end tag type
    8, // end tag size
]);

#[repr(C)]
struct Mb2Tag {
    typ: u32,
    size: u32,
}

#[repr(C, packed)]
struct Mb2FramebufferTag {
    typ: u32,
    size: u32,
    addr: u64,
    pitch: u32,
    width: u32,
    height: u32,
    bpp: u8,
    fb_type: u8,
    _reserved: u16,
}

#[repr(C, packed)]
struct Mb2FramebufferRgbTag {
    typ: u32,
    size: u32,
    addr: u64,
    pitch: u32,
    width: u32,
    height: u32,
    bpp: u8,
    fb_type: u8,
    _reserved: u16,
    red_pos: u8,
    red_size: u8,
    green_pos: u8,
    green_size: u8,
    blue_pos: u8,
    blue_size: u8,
}

#[repr(C, packed)]
struct Mb2ModuleTag {
    typ: u32,
    size: u32,
    mod_start: u32,
    mod_end: u32,
}

#[repr(C, packed)]
struct Mb2MmapTag {
    typ: u32,
    size: u32,
    entry_size: u32,
    entry_version: u32,
}

#[repr(C, packed)]
struct Mb2MmapEntry {
    addr: u64,
    len: u64,
    typ: u32,
    _reserved: u32,
}

#[derive(Clone, Copy)]
pub struct ModuleRange {
    pub start: usize,
    pub end: usize,
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        asm!("cli");
    }
    let (magic, info_ptr) = unsafe {
        let mut eax: u32;
        let mut ebx: u32;
        asm!(
            "",
            out("eax") eax,
            out("ebx") ebx,
            options(nomem, nostack, preserves_flags)
        );
        (eax, ebx)
    };

    if magic != MB2_MAGIC {
        loop {}
    }

    let mut fb: Option<Framebuffer> = None;
    let mut initrd: Option<ModuleRange> = None;
    let mut fs_img: Option<ModuleRange> = None;
    let mut bgraw: Option<ModuleRange> = None;
    let mut mem_total: u64 = 0;
    let mut mem_avail: u64 = 0;

    let info_start = info_ptr as usize;
    let total_size = unsafe { *(info_start as *const u32) } as usize;
    let mut tag_ptr = info_start + 8;
    let info_end = info_start + total_size;

    while tag_ptr + core::mem::size_of::<Mb2Tag>() <= info_end {
        let tag = unsafe { &*(tag_ptr as *const Mb2Tag) };
        if tag.typ == 0 {
            break;
        }
        match tag.typ {
            8 => {
                let fb_tag = unsafe { &*(tag_ptr as *const Mb2FramebufferTag) };
                let addr = fb_tag.addr as u32 as usize;
                let mut red_pos = 0u8;
                let mut red_size = 0u8;
                let mut green_pos = 0u8;
                let mut green_size = 0u8;
                let mut blue_pos = 0u8;
                let mut blue_size = 0u8;

                if fb_tag.fb_type == 1
                    && (fb_tag.size as usize) >= core::mem::size_of::<Mb2FramebufferRgbTag>()
                {
                    let rgb = unsafe { &*(tag_ptr as *const Mb2FramebufferRgbTag) };
                    red_pos = rgb.red_pos;
                    red_size = rgb.red_size;
                    green_pos = rgb.green_pos;
                    green_size = rgb.green_size;
                    blue_pos = rgb.blue_pos;
                    blue_size = rgb.blue_size;
                } else {
                    match fb_tag.bpp {
                        32 | 24 => {
                            red_pos = 16;
                            red_size = 8;
                            green_pos = 8;
                            green_size = 8;
                            blue_pos = 0;
                            blue_size = 8;
                        }
                        16 | 15 => {
                            red_pos = 11;
                            red_size = 5;
                            green_pos = 5;
                            green_size = 6;
                            blue_pos = 0;
                            blue_size = 5;
                        }
                        _ => {}
                    }
                }

                let bytes_per_pixel = ((fb_tag.bpp as usize) + 7) / 8;
                fb = Some(Framebuffer {
                    base: addr as *mut u8,
                    pitch: fb_tag.pitch as usize,
                    width: fb_tag.width as usize,
                    height: fb_tag.height as usize,
                    bpp: fb_tag.bpp,
                    bytes_per_pixel,
                    red_pos,
                    red_size,
                    green_pos,
                    green_size,
                    blue_pos,
                    blue_size,
                });
            }
            3 => {
                let m = unsafe { &*(tag_ptr as *const Mb2ModuleTag) };
                let start = m.mod_start as usize;
                let end = m.mod_end as usize;
                let name = module_string(tag_ptr, tag.size as usize);
                let is_fat = module_is_fat32(start, end);
                let is_bg = name_starts_with(name, b"background")
                    || name_ends_with(name, b"background.raw")
                    || module_is_raw_bg(start, end);
                let is_initrd = name_starts_with(name, b"initrd")
                    || name_ends_with(name, b".tar")
                    || module_is_tar(start, end);
                if is_fat && fs_img.is_none() {
                    fs_img = Some(ModuleRange { start, end });
                }
                if is_bg {
                    bgraw = Some(ModuleRange { start, end });
                }
                if is_initrd && initrd.is_none() && !is_fat {
                    initrd = Some(ModuleRange { start, end });
                } else if initrd.is_none() && !is_bg && !is_fat {
                    initrd = Some(ModuleRange { start, end });
                }
            }
            6 => {
                let mmap = unsafe { &*(tag_ptr as *const Mb2MmapTag) };
                if mmap.entry_size == 0 {
                    break;
                }
                let mut entry_ptr = tag_ptr + core::mem::size_of::<Mb2MmapTag>();
                let end = tag_ptr + tag.size as usize;
                while entry_ptr + (mmap.entry_size as usize) <= end {
                    let entry = unsafe { &*(entry_ptr as *const Mb2MmapEntry) };
                    mem_total = mem_total.saturating_add(entry.len);
                    if entry.typ == 1 {
                        mem_avail = mem_avail.saturating_add(entry.len);
                    }
                    entry_ptr += mmap.entry_size as usize;
                }
            }
            _ => {}
        }
        let next = (tag_ptr + tag.size as usize + 7) & !7;
        tag_ptr = next;
    }

    kernel_main(fb, initrd, fs_img, bgraw, mem_total, mem_avail)
}

fn kernel_main(
    fb: Option<Framebuffer>,
    initrd: Option<ModuleRange>,
    fs_img: Option<ModuleRange>,
    bgraw: Option<ModuleRange>,
    mem_total: u64,
    mem_avail: u64,
) -> ! {
    let fb = match fb {
        Some(v) => v,
        None => {
            vga_write(b"No framebuffer tag. Check Limine config.\n");
            loop {}
        }
    };

    if fb.bpp != 32 && fb.bpp != 24 && fb.bpp != 16 && fb.bpp != 15 {
        vga_write(b"Unsupported framebuffer bpp.\n");
        loop {}
    }

    let _ = display::enable_backbuffer(&fb);

    let mut drew_bg = false;

    if let Some(bg) = bgraw {
        drew_bg =
            display::draw_bgra_image(&fb, bg.start as *const u8, bg.end - bg.start);
    } else if let Some(initrd_mod) = initrd {
        drew_bg = draw_background(initrd_mod.start as *const u8, initrd_mod.end as *const u8, &fb);
    }

    if !drew_bg {
        display::fill(&fb, 0x00F8F8F8);
    }

    desktop::capture(&fb);
    display::present(&fb);

    let mem_total_kib = mem_total / 1024;
    let mem_avail_kib = mem_avail / 1024;
    system::set_system_info(system::SystemInfo {
        mem_total_kib,
        mem_avail_kib,
        fb_w: fb.width,
        fb_h: fb.height,
        fb_bpp: fb.bpp,
    });

    let _usb = drivers::usb::init();

    let cursor_raw = if let Some(initrd_mod) = initrd {
        find_tar_file(
            initrd_mod.start as *const u8,
            initrd_mod.end as *const u8,
            b"cursor.raw",
        )
        .map(|(data, size)| cursor::CursorRaw { data, size })
    } else {
        None
    };

    input::run(&fb, cursor_raw, fs_img);
}

fn list_tar(start: *const u8, end: *const u8, writer: &mut TextWriter) {
    let mut ptr = start;
    while (ptr as usize) + 512 <= end as usize {
        let header = unsafe { core::slice::from_raw_parts(ptr, 512) };
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let name_len = header[0..100]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(100);
        let name = &header[0..name_len];
        let size = parse_octal(&header[124..136]);
        writer.write_bytes(name);
        writer.write_line("");

        let size_padded = (size + 511) & !511;
        unsafe {
            ptr = ptr.add(512 + size_padded);
        }
    }
}

fn module_string(tag_ptr: usize, tag_size: usize) -> &'static [u8] {
    let base = tag_ptr + core::mem::size_of::<Mb2ModuleTag>();
    if tag_size <= core::mem::size_of::<Mb2ModuleTag>() {
        return &[];
    }
    let max = tag_ptr + tag_size;
    let mut len = 0usize;
    let mut p = base;
    while p < max {
        let b = unsafe { *(p as *const u8) };
        if b == 0 {
            break;
        }
        len += 1;
        p += 1;
    }
    unsafe { core::slice::from_raw_parts(base as *const u8, len) }
}

fn name_starts_with(name: &[u8], prefix: &[u8]) -> bool {
    if name.len() < prefix.len() {
        return false;
    }
    for i in 0..prefix.len() {
        let a = name[i];
        let b = prefix[i];
        if a == b {
            continue;
        }
        let al = if a >= b'A' && a <= b'Z' { a + 32 } else { a };
        let bl = if b >= b'A' && b <= b'Z' { b + 32 } else { b };
        if al != bl {
            return false;
        }
    }
    true
}

fn name_ends_with(name: &[u8], suffix: &[u8]) -> bool {
    if name.len() < suffix.len() {
        return false;
    }
    let start = name.len() - suffix.len();
    for i in 0..suffix.len() {
        let a = name[start + i];
        let b = suffix[i];
        if a == b {
            continue;
        }
        let al = if a >= b'A' && a <= b'Z' { a + 32 } else { a };
        let bl = if b >= b'A' && b <= b'Z' { b + 32 } else { b };
        if al != bl {
            return false;
        }
    }
    true
}

fn module_is_raw_bg(start: usize, end: usize) -> bool {
    let size = end.saturating_sub(start);
    if size < 8 {
        return false;
    }
    let w = read_u32_le(start as *const u8) as usize;
    let h = read_u32_le((start + 4) as *const u8) as usize;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return false;
    }
    let expected = 8usize
        .saturating_add(w.saturating_mul(h).saturating_mul(4));
    expected <= size
}

fn module_is_tar(start: usize, end: usize) -> bool {
    let size = end.saturating_sub(start);
    if size < 512 {
        return false;
    }
    let header = unsafe { core::slice::from_raw_parts(start as *const u8, 512) };
    header[257..262] == *b"ustar"
}

fn module_is_fat32(start: usize, end: usize) -> bool {
    let size = end.saturating_sub(start);
    if size < 512 {
        return false;
    }
    let header = unsafe { core::slice::from_raw_parts(start as *const u8, 512) };
    if header[510] != 0x55 || header[511] != 0xAA {
        return false;
    }
    let bps = read_u16_le(header, 11);
    if bps != 512 {
        return false;
    }
    let spc = header[13];
    if spc == 0 {
        return false;
    }
    let fs_type = &header[82..90];
    if fs_type[0] != b'F' || fs_type[1] != b'A' || fs_type[2] != b'T' {
        return false;
    }
    true
}

fn read_u16_le(buf: &[u8], offset: usize) -> u16 {
    if offset + 2 > buf.len() {
        return 0;
    }
    (buf[offset] as u16) | ((buf[offset + 1] as u16) << 8)
}

fn read_u32_le(ptr: *const u8) -> u32 {
    unsafe {
        (ptr.read() as u32)
            | ((ptr.add(1).read() as u32) << 8)
            | ((ptr.add(2).read() as u32) << 16)
            | ((ptr.add(3).read() as u32) << 24)
    }
}

fn draw_background(start: *const u8, end: *const u8, fb: &Framebuffer) -> bool {
    if let Some((data, size)) = find_tar_file(start, end, b"background.raw") {
        return display::draw_bgra_image(fb, data, size);
    }
    false
}

fn find_tar_file(start: *const u8, end: *const u8, name: &[u8]) -> Option<(*const u8, usize)> {
    let mut ptr = start;
    while (ptr as usize) + 512 <= end as usize {
        let header = unsafe { core::slice::from_raw_parts(ptr, 512) };
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let name_len = header[0..100]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(100);
        let file_name = &header[0..name_len];
        let size = parse_octal(&header[124..136]);
        let file_ptr = unsafe { ptr.add(512) };
        if file_name == name
            || (file_name.len() == name.len() + 2
                && file_name[0] == b'.'
                && file_name[1] == b'/'
                && &file_name[2..] == name)
        {
            return Some((file_ptr, size));
        }
        let size_padded = (size + 511) & !511;
        unsafe {
            ptr = ptr.add(512 + size_padded);
        }
    }
    None
}

fn parse_octal(buf: &[u8]) -> usize {
    let mut val = 0usize;
    for &b in buf {
        if b < b'0' || b > b'7' {
            continue;
        }
        val = (val << 3) + (b - b'0') as usize;
    }
    val
}

pub struct TextWriter {
    fb: Framebuffer,
    x: usize,
    y: usize,
    fg: u32,
}

impl TextWriter {
    pub fn new(fb: Framebuffer) -> Self {
        Self {
            fb,
            x: 0,
            y: 0,
            fg: 0x00FFFFFF,
        }
    }

    pub fn set_color(&mut self, color: u32) {
        self.fg = color;
    }

    pub fn set_pos(&mut self, x: usize, y: usize) {
        self.x = x;
        self.y = y;
    }

    pub fn clear(&mut self, color: u32) {
        let width = self.fb.width;
        let height = self.fb.height;
        for y in 0..height {
            for x in 0..width {
                self.put_pixel(x, y, color);
            }
        }
    }

    pub fn write_line(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
        self.new_line();
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if b == b'\n' {
                self.new_line();
            } else {
                self.write_char(b);
            }
        }
    }

    pub fn write_char(&mut self, ch: u8) {
        let glyph = FONT8X8_BASIC[ch as usize];
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if (bits >> col) & 1 == 1 {
                    let px = self.x + col;
                    let py = self.y + row;
                    if px < self.fb.width && py < self.fb.height {
                        self.put_pixel(px, py, self.fg);
                    }
                }
            }
        }
        self.x += 8;
        if self.x + 8 >= self.fb.width {
            self.new_line();
        }
    }

    pub fn new_line(&mut self) {
        self.x = 0;
        self.y += 10;
    }

    pub fn put_pixel(&self, x: usize, y: usize, rgb: u32) {
        display::put_pixel(&self.fb, x, y, rgb);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0usize;
    while i < n {
        let av = unsafe { *a.add(i) };
        let bv = unsafe { *b.add(i) };
        if av != bv {
            return av as i32 - bv as i32;
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0usize;
    while i < n {
        unsafe {
            *dst.add(i) = *src.add(i);
        }
        i += 1;
    }
    dst
}

#[no_mangle]
pub extern "C" fn memset(dst: *mut u8, val: i32, n: usize) -> *mut u8 {
    let byte = val as u8;
    let mut i = 0usize;
    while i < n {
        unsafe {
            *dst.add(i) = byte;
        }
        i += 1;
    }
    dst
}

fn vga_write(msg: &[u8]) {
    let vga = 0xb8000 as *mut u8;
    let mut col = 0usize;
    let mut row = 0usize;
    for &b in msg {
        if b == b'\n' {
            col = 0;
            row += 1;
            continue;
        }
        if row >= 25 {
            break;
        }
        let idx = (row * 80 + col) * 2;
        unsafe {
            vga.add(idx).write_volatile(b);
            vga.add(idx + 1).write_volatile(0x0F);
        }
        col += 1;
        if col >= 80 {
            col = 0;
            row += 1;
        }
    }
}

static FONT8X8_BASIC: [[u8; 8]; 128] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [126, 129, 165, 129, 189, 153, 129, 126],
    [126, 255, 219, 255, 195, 231, 255, 126],
    [108, 254, 254, 254, 124, 56, 16, 0],
    [16, 56, 124, 254, 124, 56, 16, 0],
    [56, 124, 56, 254, 254, 124, 56, 124],
    [16, 16, 56, 124, 254, 124, 56, 124],
    [0, 0, 24, 60, 60, 24, 0, 0],
    [255, 255, 231, 195, 195, 231, 255, 255],
    [0, 60, 102, 66, 66, 102, 60, 0],
    [255, 195, 153, 189, 189, 153, 195, 255],
    [15, 7, 15, 125, 204, 204, 204, 120],
    [60, 102, 102, 102, 60, 24, 126, 24],
    [63, 51, 63, 48, 48, 112, 240, 224],
    [127, 99, 127, 99, 99, 103, 230, 192],
    [153, 90, 60, 231, 231, 60, 90, 153],
    [128, 224, 248, 254, 248, 224, 128, 0],
    [2, 14, 62, 254, 62, 14, 2, 0],
    [24, 60, 126, 24, 24, 126, 60, 24],
    [102, 102, 102, 102, 102, 0, 102, 0],
    [127, 219, 219, 123, 27, 27, 27, 0],
    [62, 99, 56, 108, 108, 56, 204, 120],
    [0, 0, 0, 0, 126, 126, 126, 0],
    [24, 60, 126, 24, 126, 60, 24, 255],
    [24, 60, 126, 24, 24, 24, 24, 0],
    [24, 24, 24, 24, 126, 60, 24, 0],
    [0, 24, 12, 254, 12, 24, 0, 0],
    [0, 48, 96, 254, 96, 48, 0, 0],
    [0, 0, 192, 192, 192, 254, 0, 0],
    [0, 36, 102, 255, 102, 36, 0, 0],
    [0, 24, 60, 126, 255, 255, 0, 0],
    [0, 255, 255, 126, 60, 24, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [24, 60, 60, 24, 24, 0, 24, 0],
    [54, 54, 36, 0, 0, 0, 0, 0],
    [54, 54, 127, 54, 127, 54, 54, 0],
    [24, 62, 3, 30, 48, 31, 24, 0],
    [0, 99, 51, 24, 12, 102, 99, 0],
    [28, 54, 28, 59, 102, 102, 59, 0],
    [6, 6, 3, 0, 0, 0, 0, 0],
    [24, 12, 6, 6, 6, 12, 24, 0],
    [6, 12, 24, 24, 24, 12, 6, 0],
    [0, 102, 60, 255, 60, 102, 0, 0],
    [0, 12, 12, 63, 12, 12, 0, 0],
    [0, 0, 0, 0, 0, 12, 12, 6],
    [0, 0, 0, 63, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 12, 12, 0],
    [96, 48, 24, 12, 6, 3, 1, 0],
    [62, 99, 115, 123, 111, 103, 62, 0],
    [12, 14, 12, 12, 12, 12, 63, 0],
    [30, 51, 48, 28, 6, 51, 63, 0],
    [30, 51, 48, 28, 48, 51, 30, 0],
    [56, 60, 54, 51, 127, 48, 120, 0],
    [63, 3, 31, 48, 48, 51, 30, 0],
    [28, 6, 3, 31, 51, 51, 30, 0],
    [63, 51, 48, 24, 12, 12, 12, 0],
    [30, 51, 51, 30, 51, 51, 30, 0],
    [30, 51, 51, 62, 48, 24, 14, 0],
    [0, 12, 12, 0, 0, 12, 12, 0],
    [0, 12, 12, 0, 0, 12, 12, 6],
    [24, 12, 6, 3, 6, 12, 24, 0],
    [0, 0, 63, 0, 0, 63, 0, 0],
    [6, 12, 24, 48, 24, 12, 6, 0],
    [30, 51, 48, 24, 12, 0, 12, 0],
    [62, 99, 123, 123, 123, 3, 30, 0],
    [12, 30, 51, 51, 63, 51, 51, 0],
    [63, 102, 102, 62, 102, 102, 63, 0],
    [60, 102, 3, 3, 3, 102, 60, 0],
    [31, 54, 102, 102, 102, 54, 31, 0],
    [127, 70, 38, 30, 38, 70, 127, 0],
    [127, 70, 38, 30, 38, 6, 15, 0],
    [60, 102, 3, 3, 115, 102, 124, 0],
    [51, 51, 51, 63, 51, 51, 51, 0],
    [30, 12, 12, 12, 12, 12, 30, 0],
    [120, 48, 48, 48, 51, 51, 30, 0],
    [103, 102, 54, 30, 54, 102, 103, 0],
    [15, 6, 6, 6, 70, 102, 127, 0],
    [99, 119, 127, 107, 99, 99, 99, 0],
    [99, 103, 111, 123, 115, 99, 99, 0],
    [28, 54, 99, 99, 99, 54, 28, 0],
    [63, 102, 102, 62, 6, 6, 15, 0],
    [30, 51, 51, 51, 59, 30, 56, 0],
    [63, 102, 102, 62, 54, 102, 103, 0],
    [30, 51, 7, 14, 56, 51, 30, 0],
    [63, 45, 12, 12, 12, 12, 30, 0],
    [51, 51, 51, 51, 51, 51, 63, 0],
    [51, 51, 51, 51, 51, 30, 12, 0],
    [99, 99, 99, 107, 127, 119, 99, 0],
    [99, 99, 54, 28, 28, 54, 99, 0],
    [51, 51, 51, 30, 12, 12, 30, 0],
    [127, 99, 49, 24, 76, 102, 127, 0],
    [30, 6, 6, 6, 6, 6, 30, 0],
    [3, 6, 12, 24, 48, 96, 64, 0],
    [30, 24, 24, 24, 24, 24, 30, 0],
    [8, 28, 54, 99, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 255],
    [12, 12, 24, 0, 0, 0, 0, 0],
    [0, 0, 30, 48, 62, 51, 110, 0],
    [7, 6, 6, 62, 102, 102, 59, 0],
    [0, 0, 30, 51, 3, 51, 30, 0],
    [56, 48, 48, 62, 51, 51, 110, 0],
    [0, 0, 30, 51, 63, 3, 30, 0],
    [28, 54, 6, 15, 6, 6, 15, 0],
    [0, 0, 110, 51, 51, 62, 48, 31],
    [7, 6, 54, 110, 102, 102, 103, 0],
    [12, 0, 14, 12, 12, 12, 30, 0],
    [24, 0, 28, 24, 24, 24, 27, 14],
    [7, 6, 102, 54, 30, 54, 103, 0],
    [14, 12, 12, 12, 12, 12, 30, 0],
    [0, 0, 51, 127, 127, 107, 99, 0],
    [0, 0, 31, 51, 51, 51, 51, 0],
    [0, 0, 30, 51, 51, 51, 30, 0],
    [0, 0, 59, 102, 102, 62, 6, 15],
    [0, 0, 110, 51, 51, 62, 48, 120],
    [0, 0, 59, 102, 6, 6, 15, 0],
    [0, 0, 62, 3, 30, 48, 31, 0],
    [8, 12, 62, 12, 12, 44, 24, 0],
    [0, 0, 51, 51, 51, 51, 110, 0],
    [0, 0, 51, 51, 51, 30, 12, 0],
    [0, 0, 99, 107, 127, 127, 54, 0],
    [0, 0, 99, 54, 28, 54, 99, 0],
    [0, 0, 51, 51, 51, 62, 48, 31],
    [0, 0, 63, 25, 12, 38, 63, 0],
    [56, 12, 12, 7, 12, 12, 56, 0],
    [12, 12, 12, 0, 12, 12, 12, 0],
    [7, 12, 12, 56, 12, 12, 7, 0],
    [110, 59, 0, 0, 0, 0, 0, 0],
    [0, 8, 28, 54, 99, 99, 127, 0],
];
