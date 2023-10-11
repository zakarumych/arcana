//! Tracking global allocator.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, Ordering},
};

pub struct ArcanaAllocator {}

impl ArcanaAllocator {
    pub const fn new() -> Self {
        ArcanaAllocator {}
    }
}

unsafe impl GlobalAlloc for ArcanaAllocator {
    #[track_caller]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}
