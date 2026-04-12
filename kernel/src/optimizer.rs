// optimizer.rs - Системний оптимізатор для HouseOS
// Запобігає зависанням, оптимізує рендеринг вікон та керування пам'яттю

use crate::display::Framebuffer;

const MAX_DIRTY_RECTS: usize = 16;
const DIRTY_RECT_THRESHOLD: usize = 512;
const GC_INTERVAL: usize = 1000;

#[derive(Clone, Copy)]
pub struct DirtyRect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl DirtyRect {
    pub fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Self { x, y, w, h }
    }
    
    pub fn intersects(&self, other: &DirtyRect) -> bool {
        !(self.x + self.w <= other.x 
          || other.x + other.w <= self.x 
          || self.y + self.h <= other.y 
          || other.y + other.h <= self.y)
    }
    
    pub fn merge(&mut self, other: &DirtyRect) {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.w).max(other.x + other.w);
        let y2 = (self.y + self.h).max(other.y + other.h);
        self.x = x1;
        self.y = y1;
        self.w = x2 - x1;
        self.h = y2 - y1;
    }
}

pub struct Optimizer {
    dirty_rects: [Option<DirtyRect>; MAX_DIRTY_RECTS],
    dirty_count: usize,
    full_redraw_needed: bool,
    frame_counter: usize,
    gc_counter: usize,
    optimization_enabled: bool,
    low_memory_mode: bool,
}

impl Optimizer {
    pub fn new() -> Self {
        Self {
            dirty_rects: [None; MAX_DIRTY_RECTS],
            dirty_count: 0,
            full_redraw_needed: true,
            frame_counter: 0,
            gc_counter: 0,
            optimization_enabled: true,
            low_memory_mode: false,
        }
    }
    
    pub fn init(&mut self, _fb: &Framebuffer) {
        self.full_redraw_needed = true;
        self.dirty_count = 0;
        self.frame_counter = 0;
        self.gc_counter = 0;
    }
    
    pub fn begin_frame(&mut self) -> bool {
        if !self.optimization_enabled {
            return true;
        }
        
        self.frame_counter = self.frame_counter.wrapping_add(1);
        
        if self.low_memory_mode && self.frame_counter % 2 == 0 {
            return false;
        }
        
        true
    }
    
    pub fn end_frame(&mut self) {
        if !self.optimization_enabled {
            return;
        }
        
        if !self.full_redraw_needed {
            self.clear_dirty_rects();
        }
        
        self.gc_counter = self.gc_counter.wrapping_add(1);
        if self.gc_counter >= GC_INTERVAL {
            self.force_gc();
            self.gc_counter = 0;
        }
    }
    
    pub fn add_dirty_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        if !self.optimization_enabled || w == 0 || h == 0 {
            return;
        }
        
        let new_rect = DirtyRect::new(x, y, w, h);
        
        if self.dirty_count >= MAX_DIRTY_RECTS {
            self.full_redraw_needed = true;
            self.clear_dirty_rects();
            return;
        }
        
        for i in 0..self.dirty_count {
            if let Some(rect) = &mut self.dirty_rects[i] {
                if rect.intersects(&new_rect) {
                    rect.merge(&new_rect);
                    return;
                }
            }
        }
        
        self.dirty_rects[self.dirty_count] = Some(new_rect);
        self.dirty_count += 1;
        
        let mut total_area = 0;
        for i in 0..self.dirty_count {
            if let Some(rect) = &self.dirty_rects[i] {
                total_area += rect.w * rect.h;
            }
        }
            
        if total_area > DIRTY_RECT_THRESHOLD {
            self.full_redraw_needed = true;
            self.clear_dirty_rects();
        }
    }
    
    pub fn should_redraw_full(&self) -> bool {
        self.full_redraw_needed || !self.optimization_enabled
    }
    
    pub fn mark_clean(&mut self) {
        self.full_redraw_needed = false;
    }
    
    fn clear_dirty_rects(&mut self) {
        for i in 0..self.dirty_count {
            self.dirty_rects[i] = None;
        }
        self.dirty_count = 0;
    }
    
    fn force_gc(&mut self) {
        self.clear_dirty_rects();
    }
    
    pub fn prevent_hang(&mut self) -> bool {
        if self.frame_counter > 10000 {
            self.low_memory_mode = true;
            self.full_redraw_needed = true;
            return true;
        }
        false
    }
    
    pub fn reset_hang_protection(&mut self) {
        self.low_memory_mode = false;
        self.frame_counter = 0;
    }
    
    pub fn toggle_optimization(&mut self) {
        self.optimization_enabled = !self.optimization_enabled;
        if !self.optimization_enabled {
            self.full_redraw_needed = true;
        }
    }
}

static mut OPTIMIZER: Option<Optimizer> = None;

pub fn init_optimizer(fb: &Framebuffer) {
    unsafe {
        let mut opt = Optimizer::new();
        opt.init(fb);
        OPTIMIZER = Some(opt);
    }
}

pub fn get_optimizer() -> Option<&'static mut Optimizer> {
    unsafe { OPTIMIZER.as_mut() }
}

#[macro_export]
macro_rules! dirty_rect {
    ($x:expr, $y:expr, $w:expr, $h:expr) => {
        if let Some(opt) = $crate::optimizer::get_optimizer() {
            opt.add_dirty_rect($x, $y, $w, $h);
        }
    };
}