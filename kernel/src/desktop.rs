use crate::display::{self, Framebuffer};

const MAX_W: usize = 1024;
const MAX_H: usize = 768;
const MAX_PIXELS: usize = MAX_W * MAX_H;

static mut DESKTOP_BACK: [u32; MAX_PIXELS] = [0; MAX_PIXELS];
static mut DESKTOP_W: usize = 0;
static mut DESKTOP_H: usize = 0;

pub fn capture(fb: &Framebuffer) {
    if fb.width == 0 || fb.height == 0 {
        return;
    }
    let w = fb.width.min(MAX_W);
    let h = fb.height.min(MAX_H);
    let mut idx = 0usize;
    for y in 0..h {
        for x in 0..w {
            if idx >= MAX_PIXELS {
                break;
            }
            unsafe {
                DESKTOP_BACK[idx] = display::get_pixel(fb, x, y);
            }
            idx += 1;
        }
    }
    unsafe {
        DESKTOP_W = w;
        DESKTOP_H = h;
    }
}

pub fn restore(fb: &Framebuffer) {
    let w = unsafe { DESKTOP_W }.min(fb.width);
    let h = unsafe { DESKTOP_H }.min(fb.height);
    if w == 0 || h == 0 {
        return;
    }
    let mut idx = 0usize;
    for y in 0..h {
        for x in 0..w {
            if idx >= MAX_PIXELS {
                break;
            }
            let rgb = unsafe { DESKTOP_BACK[idx] };
            display::put_pixel(fb, x, y, rgb);
            idx += 1;
        }
    }
}
