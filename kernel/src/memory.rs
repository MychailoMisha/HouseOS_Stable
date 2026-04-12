// memory.rs - Менеджмент пам'яті для HouseOS

use core::alloc::{GlobalAlloc, Layout};

static mut ALLOCATED_BYTES: usize = 0;
static mut ALLOCATION_COUNT: usize = 0;

pub struct HouseOSAllocator;

unsafe impl GlobalAlloc for HouseOSAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = alloc_pages(layout.size());
        if !ptr.is_null() {
            ALLOCATED_BYTES += layout.size();
            ALLOCATION_COUNT += 1;
        }
        ptr
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !ptr.is_null() {
            dealloc_pages(ptr, layout.size());
            ALLOCATED_BYTES -= layout.size();
            ALLOCATION_COUNT -= 1;
        }
    }
}

#[global_allocator]
static ALLOCATOR: HouseOSAllocator = HouseOSAllocator;

const PAGE_SIZE: usize = 4096;
static mut HEAP_START: usize = 0x2000000;
static mut HEAP_PTR: usize = 0x2000000;
static mut HEAP_END: usize = 0x4000000;

fn alloc_pages(size: usize) -> *mut u8 {
    let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
    
    unsafe {
        if HEAP_PTR + pages * PAGE_SIZE > HEAP_END {
            return core::ptr::null_mut();
        }
        
        let ptr = HEAP_PTR;
        HEAP_PTR += pages * PAGE_SIZE;
        
        for i in 0..pages * PAGE_SIZE {
            *(ptr as *mut u8).add(i) = 0;
        }
        
        ptr as *mut u8
    }
}

fn dealloc_pages(_ptr: *mut u8, _size: usize) {}

pub fn gc_collect() {
    unsafe {
        ALLOCATION_COUNT = ALLOCATION_COUNT.saturating_sub(1);
    }
}

pub fn get_memory_usage() -> (usize, usize) {
    unsafe {
        let total = HEAP_END - HEAP_START;
        (ALLOCATED_BYTES, total)
    }
}