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

pub use self::arena::Arena;
