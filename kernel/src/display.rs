#[derive(Clone, Copy)]
pub struct Framebuffer {
    pub base: *mut u8,
    pub pitch: usize,
    pub width: usize,
    pub height: usize,
    pub bpp: u8,
    pub bytes_per_pixel: usize,
    pub red_pos: u8,
    pub red_size: u8,
    pub green_pos: u8,
    pub green_size: u8,
    pub blue_pos: u8,
    pub blue_size: u8,
}

const MAX_W: usize = 1024;
const MAX_H: usize = 768;
const MAX_PIXELS: usize = MAX_W * MAX_H;

static mut BACKBUFFER: [u32; MAX_PIXELS] = [0; MAX_PIXELS];
static mut BACK_W: usize = 0;
static mut BACK_H: usize = 0;
static mut BACK_ENABLED: bool = false;

pub fn enable_backbuffer(fb: &Framebuffer) -> bool {
    if fb.width == 0 || fb.height == 0 {
        unsafe {
            BACK_ENABLED = false;
            BACK_W = 0;
            BACK_H = 0;
        }
        return false;
    }
    if fb.width > MAX_W || fb.height > MAX_H {
        unsafe {
            BACK_ENABLED = false;
            BACK_W = 0;
            BACK_H = 0;
        }
        return false;
    }
    unsafe {
        BACK_W = fb.width;
        BACK_H = fb.height;
        BACK_ENABLED = true;
    }
    true
}

fn backbuffer_enabled() -> bool {
    unsafe { BACK_ENABLED }
}

fn back_put_pixel(x: usize, y: usize, rgb: u32) {
    let w = unsafe { BACK_W };
    let h = unsafe { BACK_H };
    if x >= w || y >= h || w == 0 || h == 0 {
        return;
    }
    unsafe {
        BACKBUFFER[y * w + x] = rgb;
    }
}

fn back_get_pixel(x: usize, y: usize) -> u32 {
    let w = unsafe { BACK_W };
    let h = unsafe { BACK_H };
    if x >= w || y >= h || w == 0 || h == 0 {
        return 0;
    }
    unsafe { BACKBUFFER[y * w + x] }
}

pub fn draw_bgra_image(fb: &Framebuffer, data: *const u8, size: usize) -> bool {
    if fb.base.is_null() || fb.width == 0 || fb.height == 0 || fb.pitch == 0 {
        return false;
    }
    if size < 8 {
        return false;
    }
    let w = read_u32_le(data) as usize;
    let h = read_u32_le(unsafe { data.add(4) }) as usize;
    if w == 0 || h == 0 {
        return false;
    }
    let pixels = unsafe { data.add(8) };
    let pixel_len = size - 8;
    let expected = w.saturating_mul(h).saturating_mul(4);
    if pixel_len < expected {
        return false;
    }
    blit_bgra_scaled(fb, pixels, w, h);
    true
}

pub fn put_pixel(fb: &Framebuffer, x: usize, y: usize, rgb: u32) {
    if backbuffer_enabled() {
        back_put_pixel(x, y, rgb);
        return;
    }
    put_pixel_fb(fb, x, y, rgb);
}

fn put_pixel_fb(fb: &Framebuffer, x: usize, y: usize, rgb: u32) {
    let bytes_pp = fb.bytes_per_pixel;
    if bytes_pp == 0 {
        return;
    }
    let offset = y * fb.pitch + x * bytes_pp;
    let ptr = unsafe { fb.base.add(offset) };
    if fb.bpp == 32 || fb.bpp == 24 {
        let b = (rgb & 0xFF) as u8;
        let g = ((rgb >> 8) & 0xFF) as u8;
        let r = ((rgb >> 16) & 0xFF) as u8;
        unsafe {
            ptr.write_volatile(b);
            ptr.add(1).write_volatile(g);
            ptr.add(2).write_volatile(r);
            if bytes_pp == 4 {
                ptr.add(3).write_volatile(0);
            }
        }
        return;
    }
    let packed = pack_color(fb, rgb);
    for i in 0..bytes_pp {
        unsafe {
            ptr.add(i)
                .write_volatile(((packed >> (i * 8)) & 0xFF) as u8);
        }
    }
}

pub fn get_pixel(fb: &Framebuffer, x: usize, y: usize) -> u32 {
    if backbuffer_enabled() {
        return back_get_pixel(x, y);
    }
    let bytes_pp = fb.bytes_per_pixel;
    if bytes_pp == 0 {
        return 0;
    }
    let offset = y * fb.pitch + x * bytes_pp;
    let ptr = unsafe { fb.base.add(offset) };
    if fb.bpp == 32 || fb.bpp == 24 {
        let b = unsafe { ptr.read_volatile() } as u32;
        let g = unsafe { ptr.add(1).read_volatile() } as u32;
        let r = unsafe { ptr.add(2).read_volatile() } as u32;
        return (r << 16) | (g << 8) | b;
    }
    let mut packed: u32 = 0;
    for i in 0..bytes_pp {
        packed |= (unsafe { ptr.add(i).read_volatile() } as u32) << (i * 8);
    }
    unpack_color(fb, packed)
}

pub fn fill(fb: &Framebuffer, rgb: u32) {
    if backbuffer_enabled() {
        let w = unsafe { BACK_W };
        let h = unsafe { BACK_H };
        if w == 0 || h == 0 {
            return;
        }
        let total = w * h;
        for i in 0..total {
            unsafe {
                BACKBUFFER[i] = rgb;
            }
        }
        return;
    }
    let width = fb.width;
    let height = fb.height;
    for y in 0..height {
        for x in 0..width {
            put_pixel_fb(fb, x, y, rgb);
        }
    }
}

pub fn fill_rect(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize, rgb: u32) {
    if w == 0 || h == 0 {
        return;
    }
    if backbuffer_enabled() {
        let max_x = unsafe { BACK_W };
        let max_y = unsafe { BACK_H };
        let end_x = (x + w).min(max_x);
        let end_y = (y + h).min(max_y);
        for py in y..end_y {
            let row = py * max_x;
            for px in x..end_x {
                unsafe {
                    BACKBUFFER[row + px] = rgb;
                }
            }
        }
        return;
    }
    let max_x = fb.width;
    let max_y = fb.height;
    let end_x = (x + w).min(max_x);
    let end_y = (y + h).min(max_y);
    for py in y..end_y {
        for px in x..end_x {
            put_pixel_fb(fb, px, py, rgb);
        }
    }
}

pub fn draw_sprite(
    fb: &Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    pixels: &[u32],
    transparent: u32,
) {
    if w == 0 || h == 0 || pixels.len() < w * h {
        return;
    }
    let max_x = fb.width;
    let max_y = fb.height;
    for row in 0..h {
        let py = y + row;
        if py >= max_y {
            break;
        }
        let base = row * w;
        for col in 0..w {
            let px = x + col;
            if px >= max_x {
                break;
            }
            let rgb = pixels[base + col];
            if rgb != transparent {
                put_pixel(fb, px, py, rgb);
            }
        }
    }
}

pub fn present(fb: &Framebuffer) {
    if !backbuffer_enabled() {
        return;
    }
    let w = unsafe { BACK_W }.min(fb.width);
    let h = unsafe { BACK_H }.min(fb.height);
    if w == 0 || h == 0 || fb.base.is_null() {
        return;
    }
    let bytes_pp = fb.bytes_per_pixel;
    if bytes_pp == 0 {
        return;
    }
    if fb.bpp == 32 || fb.bpp == 24 {
        for y in 0..h {
            let row_base = y * unsafe { BACK_W };
            let dst_row = unsafe { fb.base.add(y * fb.pitch) };
            for x in 0..w {
                let rgb = unsafe { BACKBUFFER[row_base + x] };
                let b = (rgb & 0xFF) as u8;
                let g = ((rgb >> 8) & 0xFF) as u8;
                let r = ((rgb >> 16) & 0xFF) as u8;
                let ptr = unsafe { dst_row.add(x * bytes_pp) };
                unsafe {
                    ptr.write_volatile(b);
                    ptr.add(1).write_volatile(g);
                    ptr.add(2).write_volatile(r);
                    if bytes_pp == 4 {
                        ptr.add(3).write_volatile(0);
                    }
                }
            }
        }
        return;
    }
    for y in 0..h {
        let row_base = y * unsafe { BACK_W };
        for x in 0..w {
            let rgb = unsafe { BACKBUFFER[row_base + x] };
            put_pixel_fb(fb, x, y, rgb);
        }
    }
}

pub fn present_rect(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize) {
    if !backbuffer_enabled() {
        return;
    }
    if w == 0 || h == 0 || fb.base.is_null() {
        return;
    }
    let max_w = unsafe { BACK_W }.min(fb.width);
    let max_h = unsafe { BACK_H }.min(fb.height);
    if max_w == 0 || max_h == 0 {
        return;
    }
    let end_x = (x + w).min(max_w);
    let end_y = (y + h).min(max_h);
    let bytes_pp = fb.bytes_per_pixel;
    if bytes_pp == 0 {
        return;
    }
    if fb.bpp == 32 || fb.bpp == 24 {
        for py in y..end_y {
            let row_base = py * unsafe { BACK_W };
            let dst_row = unsafe { fb.base.add(py * fb.pitch) };
            for px in x..end_x {
                let rgb = unsafe { BACKBUFFER[row_base + px] };
                let b = (rgb & 0xFF) as u8;
                let g = ((rgb >> 8) & 0xFF) as u8;
                let r = ((rgb >> 16) & 0xFF) as u8;
                let ptr = unsafe { dst_row.add(px * bytes_pp) };
                unsafe {
                    ptr.write_volatile(b);
                    ptr.add(1).write_volatile(g);
                    ptr.add(2).write_volatile(r);
                    if bytes_pp == 4 {
                        ptr.add(3).write_volatile(0);
                    }
                }
            }
        }
        return;
    }
    for py in y..end_y {
        let row_base = py * unsafe { BACK_W };
        for px in x..end_x {
            let rgb = unsafe { BACKBUFFER[row_base + px] };
            put_pixel_fb(fb, px, py, rgb);
        }
    }
}

fn read_u32_le(ptr: *const u8) -> u32 {
    unsafe {
        (ptr.read() as u32)
            | ((ptr.add(1).read() as u32) << 8)
            | ((ptr.add(2).read() as u32) << 16)
            | ((ptr.add(3).read() as u32) << 24)
    }
}

fn blit_bgra_scaled(fb: &Framebuffer, src: *const u8, src_w: usize, src_h: usize) {
    let dst_w = fb.width;
    let dst_h = fb.height;
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return;
    }

    let step_x = ((src_w as u32) << 16) / (dst_w as u32);
    let step_y = ((src_h as u32) << 16) / (dst_h as u32);
    let mut sy_fp: u32 = 0;

    for y in 0..dst_h {
        let sy = (sy_fp >> 16) as usize;
        let row = unsafe { src.add(sy * src_w * 4) };
        let mut sx_fp: u32 = 0;
        for x in 0..dst_w {
            let sx = (sx_fp >> 16) as usize;
            let px = unsafe { row.add(sx * 4) };
            let b = unsafe { px.read() } as u32;
            let g = unsafe { px.add(1).read() } as u32;
            let r = unsafe { px.add(2).read() } as u32;
            let rgb = (r << 16) | (g << 8) | b;
            put_pixel(fb, x, y, rgb);
            sx_fp = sx_fp.wrapping_add(step_x);
        }
        sy_fp = sy_fp.wrapping_add(step_y);
    }
}

fn pack_color(fb: &Framebuffer, rgb: u32) -> u32 {
    let r = ((rgb >> 16) & 0xFF) as u32;
    let g = ((rgb >> 8) & 0xFF) as u32;
    let b = (rgb & 0xFF) as u32;

    let rp = if fb.red_size == 0 {
        0
    } else {
        r >> (8 - fb.red_size as u32)
    };
    let gp = if fb.green_size == 0 {
        0
    } else {
        g >> (8 - fb.green_size as u32)
    };
    let bp = if fb.blue_size == 0 {
        0
    } else {
        b >> (8 - fb.blue_size as u32)
    };

    (rp << fb.red_pos)
        | (gp << fb.green_pos)
        | (bp << fb.blue_pos)
}

fn unpack_color(fb: &Framebuffer, packed: u32) -> u32 {
    let r = expand_bits(
        if fb.red_size == 0 {
            0
        } else {
            (packed >> fb.red_pos) & ((1u32 << fb.red_size) - 1)
        },
        fb.red_size,
    );
    let g = expand_bits(
        if fb.green_size == 0 {
            0
        } else {
            (packed >> fb.green_pos) & ((1u32 << fb.green_size) - 1)
        },
        fb.green_size,
    );
    let b = expand_bits(
        if fb.blue_size == 0 {
            0
        } else {
            (packed >> fb.blue_pos) & ((1u32 << fb.blue_size) - 1)
        },
        fb.blue_size,
    );
    (r << 16) | (g << 8) | b
}

fn expand_bits(val: u32, size: u8) -> u32 {
    if size == 0 {
        return 0;
    }
    let max = (1u32 << size) - 1;
    (val * 255 + (max / 2)) / max
}
