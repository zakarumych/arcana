extern crate arcana;

pub use arcana::*;

#[global_allocator]
pub static ALLOC: arcana::alloc::ArcanaAllocator = arcana::alloc::ArcanaAllocator::new();
