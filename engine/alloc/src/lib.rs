//! Allocation library for Arcana Engine.

mod arena;

/// Triggers panic.
/// Use when too large capacity is requested.
#[inline(never)]
#[cold]
fn capacity_overflow() -> ! {
    panic!("capacity overflow");
}

// Guards against allocating too large memory on systems with less than 64-bit address space.
#[inline(always)]
fn alloc_guard(alloc_size: usize) {
    // On 64-bit and wider systems, we assume that allocations larger than isize::MAX are impossible.
    if usize::BITS < 64 && alloc_size > isize::MAX as usize {
        capacity_overflow()
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

pub use self::arena::Arena;

pub struct TrackingGlobalAllocator {
    disabled: AtomicBool,
}

impl TrackingGlobalAllocator {
    pub const fn new() -> Self {
        TrackingGlobalAllocator {
            disabled: AtomicBool::new(false),
        }
    }
}

unsafe impl std::alloc::GlobalAlloc for TrackingGlobalAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        if layout.size() >= 1024 * 1024 {
            let was_disabled = self.disabled.fetch_or(true, Ordering::SeqCst);

            if !was_disabled {
                let backtrace = std::backtrace::Backtrace::force_capture();

                eprintln!("Allocation backtrace: {:?}", backtrace);

                self.disabled.store(false, Ordering::SeqCst);
            }
        }

        unsafe { std::alloc::System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        if layout.size() >= 1024 * 1024 {
            let was_disabled = self.disabled.fetch_or(true, Ordering::SeqCst);

            if !was_disabled {
                let backtrace = std::backtrace::Backtrace::force_capture();

                eprintln!("Deallocation backtrace: {:?}", backtrace);

                self.disabled.store(false, Ordering::SeqCst);
            }
        }

        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}
