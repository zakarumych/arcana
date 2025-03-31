mod ident;
mod intern;
mod name;

pub use self::{
    ident::{validate_ident, Ident, IdentError},
    name::{validate_name, Name, NameError},
};

/// Intern an arbitrary string.
///
/// Returns interned version of the string.
pub fn intern(s: &str) -> &'static str {
    intern::INTERNER.intern(s)
}

/// Intern an arbitrary string.
///
/// Returns interned version of the string.
///
/// This version uses statically allocated string instead of allocating storage.
///
/// While caller don't get more than provided - a static string again,
/// but other callers that intern borrowed string may find it interned already and skip allocation and insertion.
pub fn intern_static(s: &'static str) -> &'static str {
    intern::INTERNER.intern_static(s)
}

/// Intern an arbitrary string.
///
/// Returns interned version of the string.
///
/// This version uses provided [`String`] to store value instead of allocating storage.
/// If you don't already have an expendable [`String`], use [`intern`] instead.
pub fn intern_string(s: String) -> &'static str {
    intern::INTERNER.intern_string(s)
}
