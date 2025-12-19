mod ident;
mod intern;
mod name;

pub use self::{
    ident::{Ident, IdentError, assert_ascii_ident, validate_ident},
    name::{Name, NameError, assert_ascii_name, validate_name},
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

/// Formats a string with constant arguments only.
/// Keeps static reference to avoid re-formatting the same string each time code is executed.
/// Interns the result and returns a static string.
#[macro_export]
macro_rules! const_format {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        static CONST_FORMAT_STRING: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
        CONST_FORMAT_STRING.get_or_init(|| {
            intern_string(::std::format!(const { $fmt } $(, const { $arg })*))
        })
    }};
}
