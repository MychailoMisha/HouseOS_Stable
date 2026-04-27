// optimizer.rs - runtime redraw optimizer for HouseOS (improved, no_std)
// Зберігає старий API, виправляє логічні помилки

use crate::display::Framebuffer;

// Константи
const MAX_DIRTY_RECTS: usize = 24;
const MIN_DIRTY_AREA_THRESHOLD: usize = 8 * 1024;
const DIRTY_RATIO_DIVISOR: usize = 5;
const LOW_MEM_DIRTY_RATIO_DIVISOR: usize = 8;
const NEARBY_MERGE_GAP: usize = 3;
const LOW_MEMORY_SKIP_DIV: usize = 2;
const MAX_FULL_REDRAW_STREAK: usize = 360;
const LOW_MEM_RECOVERY_FRAMES: usize = 600;   // через скільки кадрів вийти з low-memory

#[derive(Clone, Copy, Debug)]
pub struct DirtyRect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl DirtyRect {
    pub const fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Self { x, y, w, h }
    }

    #[inline]
    fn area(&self) -> usize {
        self.w.saturating_mul(self.h)
    }

    #[inline]
    fn intersects_or_nearby(&self, other: &DirtyRect) -> bool {
        let self_right = self.x.saturating_add(self.w).saturating_add(NEARBY_MERGE_GAP);
        let other_right = other.x.saturating_add(other.w).saturating_add(NEARBY_MERGE_GAP);
        let self_bottom = self.y.saturating_add(self.h).saturating_add(NEARBY_MERGE_GAP);
        let other_bottom = other.y.saturating_add(other.h).saturating_add(NEARBY_MERGE_GAP);
        !(self_right <= other.x
            || other_right <= self.x
            || self_bottom <= other.y
            || other_bottom <= self.y)
    }

    #[inline]
    fn merge(&mut self, other: &DirtyRect) {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = self.x.saturating_add(self.w).max(other.x.saturating_add(other.w));
        let y2 = self.y.saturating_add(self.h).max(other.y.saturating_add(other.h));
        self.x = x1;
        self.y = y1;
        self.w = x2.saturating_sub(x1);
        self.h = y2.saturating_sub(y1);
    }

    /// Об'єднує два прямокутники, повертаючи новий (без зміни self)
    pub fn merged(&self, other: &DirtyRect) -> Self {
        let mut result = *self;
        result.merge(other);
        result
    }
}

pub struct Optimizer {
    dirty_rects: [Option<DirtyRect>; MAX_DIRTY_RECTS],
    dirty_count: usize,
    dirty_area: usize,
    full_redraw_needed: bool,
    frame_counter: usize,
    optimization_enabled: bool,
    low_memory_mode: bool,
    low_mem_skip_counter: usize,
    full_redraw_streak: usize,
    frames_since_last_full_redraw: usize,
    screen_w: usize,
    screen_h: usize,
    screen_area: usize,
}

impl Optimizer {
    pub const fn new() -> Self {
        Self {
            dirty_rects: [None; MAX_DIRTY_RECTS],
            dirty_count: 0,
            dirty_area: 0,
            full_redraw_needed: true,
            frame_counter: 0,
            optimization_enabled: true,
            low_memory_mode: false,
            low_mem_skip_counter: 0,
            full_redraw_streak: 0,
            frames_since_last_full_redraw: 0,
            screen_w: 0,
            screen_h: 0,
            screen_area: 1,
        }
    }

    pub fn init(&mut self, fb: &Framebuffer) {
        self.screen_w = fb.width;
        self.screen_h = fb.height;
        self.screen_area = fb.width.saturating_mul(fb.height).max(1);
        self.full_redraw_needed = true;
        self.frame_counter = 0;
        self.low_mem_skip_counter = 0;
        self.full_redraw_streak = 0;
        self.frames_since_last_full_redraw = 0;
        self.clear_dirty_rects();
    }

    /// Слід викликати на початку кожного кадру.
    /// Повертає true, якщо кадр слід рендерити (з частковим або повним оновленням).
    pub fn begin_frame(&mut self) -> bool {
        if !self.optimization_enabled {
            return true;
        }
        if self.full_redraw_needed {
            return true;
        }

        self.frame_counter = self.frame_counter.wrapping_add(1);
        self.frames_since_last_full_redraw = self.frames_since_last_full_redraw.saturating_add(1);

        // Автоматичний вихід із low-memory режиму
        if self.low_memory_mode && self.frames_since_last_full_redraw > LOW_MEM_RECOVERY_FRAMES {
            self.low_memory_mode = false;
            self.low_mem_skip_counter = 0;
        }

        // Пропуск кадрів у low-memory, якщо немає брудних областей
        if self.low_memory_mode && self.dirty_count == 0 {
            self.low_mem_skip_counter = self.low_mem_skip_counter.wrapping_add(1);
            // Якщо full_redraw_needed, пропускати не можна
            if self.low_mem_skip_counter % LOW_MEMORY_SKIP_DIV != 0 && !self.full_redraw_needed {
                return false;
            }
        } else {
            self.low_mem_skip_counter = 0;
        }

        true
    }

    /// Завершує кадр; очищає брудні прямокутники, якщо не потрібне повне перемальовування.
    pub fn end_frame(&mut self) {
        if !self.optimization_enabled {
            return;
        }
        if !self.full_redraw_needed {
            self.clear_dirty_rects();
        }
    }

    /// Додає прямокутник до списку змінених областей.
    pub fn add_dirty_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        if !self.optimization_enabled || w == 0 || h == 0 || self.full_redraw_needed {
            return;
        }
        let rect = match self.clamp_to_screen(x, y, w, h) {
            Some(r) => r,
            None => return,
        };
        if self.dirty_count >= MAX_DIRTY_RECTS {
            self.request_full_redraw();
            return;
        }
        // Шукаємо існуючий прямокутник, що перетинається або поруч
        for i in 0..self.dirty_count {
            if let Some(existing) = &mut self.dirty_rects[i] {
                if existing.intersects_or_nearby(&rect) {
                    existing.merge(&rect);
                    self.coalesce_dirty_rects();
                    self.recalc_dirty_area();
                    self.check_dirty_budget();
                    return;
                }
            }
        }
        // Інакше додаємо новий
        self.dirty_rects[self.dirty_count] = Some(rect);
        self.dirty_count += 1;
        self.coalesce_dirty_rects();
        self.recalc_dirty_area();
        self.check_dirty_budget();
    }

    /// Чи потрібне повне перемальовування всього екрану.
    pub fn should_redraw_full(&self) -> bool {
        self.full_redraw_needed || !self.optimization_enabled
    }

    /// Позначає, що повне перемальовування виконано, і брудні області більше не актуальні.
    pub fn mark_clean(&mut self) {
        self.full_redraw_needed = false;
        self.full_redraw_streak = 0;
        self.frames_since_last_full_redraw = 0;
        self.clear_dirty_rects();
    }

    /// Отримати зріз поточних брудних прямокутників (для часткового оновлення).
    pub fn dirty_rects(&self) -> &[DirtyRect] {
        // Трансмутуємо Option<DirtyRect> у DirtyRect, бо всі Some в межах dirty_count
        unsafe {
            core::slice::from_raw_parts(
                self.dirty_rects.as_ptr() as *const DirtyRect,
                self.dirty_count,
            )
        }
    }

    /// Об'єднаний прямокутник, що охоплює всі брудні області.
    /// Може бути використаний для швидкого (але надлишкового) копіювання.
    pub fn dirty_bounding_box(&self) -> Option<DirtyRect> {
        if self.dirty_count == 0 {
            return None;
        }
        let drects = self.dirty_rects();
        let mut bbox = drects[0];
        for rect in &drects[1..] {
            bbox = bbox.merged(rect);
        }
        Some(bbox)
    }

    /// Захист від зависань: якщо повне перемальовування триває надто довго,
    /// примусово вмикає режим низької пам'яті.
    pub fn prevent_hang(&mut self) -> bool {
        if !self.optimization_enabled {
            return false;
        }
        if self.full_redraw_needed {
            self.full_redraw_streak = self.full_redraw_streak.saturating_add(1);
        } else if self.full_redraw_streak > 0 {
            self.full_redraw_streak -= 1;
        }
        if self.full_redraw_streak > MAX_FULL_REDRAW_STREAK {
            self.low_memory_mode = true;
            self.request_full_redraw();
            self.full_redraw_streak = 0;
            return true;
        }
        false
    }

    pub fn reset_hang_protection(&mut self) {
        self.low_memory_mode = false;
        self.low_mem_skip_counter = 0;
        self.full_redraw_streak = 0;
    }

    pub fn toggle_optimization(&mut self) {
        self.optimization_enabled = !self.optimization_enabled;
        if !self.optimization_enabled {
            self.request_full_redraw();
        } else {
            self.full_redraw_needed = true;
        }
    }

    // --- Внутрішні методи ---
    fn request_full_redraw(&mut self) {
        self.full_redraw_needed = true;
        self.clear_dirty_rects();
    }

    fn clamp_to_screen(&self, x: usize, y: usize, w: usize, h: usize) -> Option<DirtyRect> {
        if self.screen_w == 0 || self.screen_h == 0 {
            return Some(DirtyRect::new(x, y, w, h));
        }
        if x >= self.screen_w || y >= self.screen_h {
            return None;
        }
        let end_x = x.saturating_add(w).min(self.screen_w);
        let end_y = y.saturating_add(h).min(self.screen_h);
        if end_x <= x || end_y <= y {
            return None;
        }
        Some(DirtyRect::new(x, y, end_x - x, end_y - y))
    }

    fn dirty_area_threshold(&self) -> usize {
        let divisor = if self.low_memory_mode {
            LOW_MEM_DIRTY_RATIO_DIVISOR
        } else {
            DIRTY_RATIO_DIVISOR
        };
        let by_ratio = self.screen_area / divisor.max(1);
        by_ratio.min(self.screen_area).max(MIN_DIRTY_AREA_THRESHOLD)
    }

    fn check_dirty_budget(&mut self) {
        if self.dirty_count >= MAX_DIRTY_RECTS {
            self.request_full_redraw();
            return;
        }
        if self.dirty_area > self.dirty_area_threshold() {
            self.request_full_redraw();
        }
    }

    // Покращена коалесценція: сортування на місці та об'єднання без додаткового масиву.
    fn coalesce_dirty_rects(&mut self) {
        if self.dirty_count <= 1 {
            return;
        }

        // Сортування бульбашкою на місці (n <= 24)
        for i in 0..self.dirty_count {
            for j in i + 1..self.dirty_count {
                let a = self.dirty_rects[i].unwrap();
                let b = self.dirty_rects[j].unwrap();
                if a.x > b.x || (a.x == b.x && a.y > b.y) {
                    self.dirty_rects.swap(i, j);
                }
            }
        }

        // Об'єднання перекривних/близьких прямокутників з видаленням поглинутих
        let mut write_idx = 0;
        for read_idx in 1..self.dirty_count {
            let current = self.dirty_rects[write_idx].unwrap();
            let next = self.dirty_rects[read_idx].unwrap();
            if current.intersects_or_nearby(&next) {
                // Об'єднуємо next у current
                let mut merged = current;
                merged.merge(&next);
                self.dirty_rects[write_idx] = Some(merged);
            } else {
                write_idx += 1;
                self.dirty_rects[write_idx] = self.dirty_rects[read_idx];
            }
        }
        self.dirty_count = write_idx + 1;
        // Очищаємо зайві слоти (не обов'язково, але для чистоти)
        for i in self.dirty_count..MAX_DIRTY_RECTS {
            self.dirty_rects[i] = None;
        }
    }

    fn recalc_dirty_area(&mut self) {
        let mut area: usize = 0;
        for i in 0..self.dirty_count {
            if let Some(rect) = self.dirty_rects[i] {
                area = area.saturating_add(rect.area());
            }
        }
        self.dirty_area = area;
    }

    fn clear_dirty_rects(&mut self) {
        for i in 0..MAX_DIRTY_RECTS {
            self.dirty_rects[i] = None;
        }
        self.dirty_count = 0;
        self.dirty_area = 0;
    }
}

// Глобальний синглтон
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
            opt.add_dirty_rect($x as usize, $y as usize, $w as usize, $h as usize);
        }
    };
}