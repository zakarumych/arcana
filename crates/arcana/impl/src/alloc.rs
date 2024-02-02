//! Tracking global allocator.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub struct ArcanaAllocator;

unsafe impl GlobalAlloc for ArcanaAllocator {
    #[track_caller]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            GLOBAL_STATS.allocations.fetch_add(1, Ordering::Relaxed);
            GLOBAL_STATS
                .allocated_bytes
                .fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        GLOBAL_STATS.deallocations.fetch_add(1, Ordering::Relaxed);
        GLOBAL_STATS
            .deallocated_bytes
            .fetch_add(layout.size(), Ordering::Relaxed);

        unsafe {
            System.dealloc(ptr, layout);
        }
    }
}

impl ArcanaAllocator {
    pub fn global_stats() -> Stats {
        Stats {
            deallocated_bytes: GLOBAL_STATS.deallocated_bytes.load(Ordering::Relaxed),
            deallocations: GLOBAL_STATS.deallocations.load(Ordering::Relaxed),
            allocated_bytes: GLOBAL_STATS.allocated_bytes.load(Ordering::Relaxed),
            allocations: GLOBAL_STATS.allocations.load(Ordering::Relaxed),
        }
    }
}

pub struct Stats {
    pub allocations: usize,
    pub deallocations: usize,
    pub allocated_bytes: usize,
    pub deallocated_bytes: usize,
}

pub struct StatAccumulator {
    allocations: AtomicUsize,
    deallocations: AtomicUsize,
    allocated_bytes: AtomicUsize,
    deallocated_bytes: AtomicUsize,
}

impl StatAccumulator {
    pub const fn new() -> Self {
        StatAccumulator {
            allocations: AtomicUsize::new(0),
            deallocations: AtomicUsize::new(0),
            allocated_bytes: AtomicUsize::new(0),
            deallocated_bytes: AtomicUsize::new(0),
        }
    }
}

static GLOBAL_STATS: StatAccumulator = StatAccumulator::new();

thread_local! {
    static LOCAL_STATS: Cell<Option<&'static StatAccumulator>> = const { Cell::new(None) };
}

#[doc(hidden)]
pub fn set_local_stat_accumulator(
    accumulator: Option<&'static StatAccumulator>,
) -> Option<&'static StatAccumulator> {
    LOCAL_STATS.with(|local| local.replace(accumulator))
}

#[doc(hidden)]
pub struct StatAccumulatorGuard {
    prev: Option<&'static StatAccumulator>,
}

impl Drop for StatAccumulatorGuard {
    fn drop(&mut self) {
        set_local_stat_accumulator(self.prev);
    }
}

macro_rules! new_alloc_category {
    ($name:ident = $description:literal) => {
        pub struct $name;

        impl $name {
            pub fn description() -> &'static str {
                $description
            }

            pub fn stats() -> &'static $crate::alloc::StatAccumulator {
                static STATS: $crate::alloc::StatAccumulator =
                    $crate::alloc::StatAccumulator::new();
                &STATS
            }

            pub fn set_local_stat_accumulator() -> StatAccumulatorGuard {
                let prev = $crate::alloc::set_local_stat_accumulator(Self::stats());
                StatAccumulatorGuard { prev }
            }
        }
    };
}

macro_rules! with_alloc_category {
    ($name:ident) => {
        let _guard = $name::set_local_stat_accumulator();
    };
}
