//! This module provides a way to create layout object of the given type.
//! With this layout object it is possible to compare two types for binary compatibility.
//!
//! If two objects have types with compatible layout, one can be safely transmuted to another.
//!
//!
//! Layout is based on primitives, arrays, products and sums.
//! Primitives are basic types like `u8`, `i32`, `f64`, etc. All primitives are compatible only to themselves.
//! Arrays are compatible if they have compatible element and length.
//! Products are compatible if all elements of respective offsets are compatible.
//! Sums are compatible if all elements are compatible.
//!
//! Type layout compatibility is used to keep values with types from unloaded plugins
//! and replace their types with compatible type from newly loaded plugin.
//!
//! Note that in products zero-sized elements are ignored and don't affect layout.

/// Type layout.
pub struct Layout {}

pub enum PrimitiveLayout {
    Unit,
    Bool,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    Usize,
    Isize,
    F32,
    F64,
}

pub struct ArrayLayout {
    pub element: Box<Layout>,
    pub len: usize,
}

pub struct ProductElement {
    pub offset: usize,
    pub layout: Layout,
}

pub struct ProductLayout {
    pub elements: Vec<ProductElement>,
}
