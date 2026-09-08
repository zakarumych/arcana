//! Type identifiers with explicit or inferred identity.
//!
//! `Stid` is a nonzero 64-bit value associated with a type through [`HasStid`].
//! The trait can be derived, implemented with [`has_stid!`], or implemented manually.
//!
//! A direct integer literal assigns its exact value and must not be zero. Reserve
//! values with the most significant bit clear to avoid collisions with hashed IDs.
//! Other expressions must be const-evaluable and their values must implement
//! [`core::hash::Hash`]. String, byte string, character, byte, and boolean literals
//! are hashed during macro expansion. Other values are evaluated in the consuming
//! crate, then hashed at runtime. Both use Arcana's stable hasher with the most
//! significant bit set. Numeric constants and arithmetic expressions are hashed,
//! not assigned verbatim.
//!
//! Without an explicit value, the derive hashes the type definition and the
//! function-like macro hashes its type syntax, setting the most significant bit.
//! Derived generic types combine the base identifier with the identifiers of
//! their type arguments. Lifetimes and const argument values are not combined.
//!
//! Explicit values decouple identity from a type's definition. Choose unique
//! values for distinct types. The hasher is stable; changing a value type's
//! `Hash` implementation may change its hashed ID.
//!

pub use arcana_proc::{HasStid, has_stid};
use gametime::TimeSpan;

extern crate self as arcana_id;

crate::make_uid! {
    /// Stable Type Identifier.
    ///
    /// Identifier is assigned via `HasStid` trait.
    /// The trait can be implemented manually, derived or implemented by `has_stid!` macro.
    ///
    /// Both macros accept an exact integer literal or hash a const expression's value.
    /// If no value is specified, they hash the type's definition or syntax.
    ///
    /// Generated identifier will always have MSB set to 1.
    /// This ensures that manually set identifiers with MSB set to 0 will never collide with generated ones.
    pub Stid;
}

/// Trait for types that have stable identifier.
/// Derive it, implement manually or use `has_stid!` macro.
///
/// The derive accepts `#[stid = literal]` or `#[stid(expression)]`:
///
/// ```
/// # extern crate self as arcana;
/// # pub extern crate arcana_id as id;
/// use arcana_id::HasStid;
///
/// #[derive(HasStid)]
/// #[stid = 42]
/// struct Exact;
///
/// const COMPONENT_KEY: (&str, u32) = ("my-plugin::Position", 1);
///
/// #[derive(HasStid)]
/// #[stid(COMPONENT_KEY)]
/// struct Position;
///
/// struct OtherPosition;
/// arcana_id::has_stid!(OtherPosition = ("my-plugin::Position", 1u32) @ arcana_id);
///
/// # fn main() {
/// assert_eq!(Exact::stid().get(), 42);
/// assert_eq!(Position::stid(), OtherPosition::stid());
/// # }
/// ```
///
/// String literals, including hexadecimal-looking strings, are hashed as strings.
/// A supplied path after `@` identifies the ID crate; the default is `::arcana::id`.
///
/// Exact IDs must not be zero:
///
/// ```compile_fail
/// struct Invalid;
/// arcana_id::has_stid!(Invalid = 0 @ arcana_id);
/// ```
///
/// Expressions must be const-evaluable:
///
/// ```compile_fail
/// fn runtime_key() -> u32 { 42 }
/// struct Invalid;
/// arcana_id::has_stid!(Invalid = runtime_key() @ arcana_id);
/// let _ = <Invalid as arcana_id::HasStid>::stid();
/// ```
///
/// Expression values must implement `Hash`:
///
/// ```compile_fail
/// # extern crate self as arcana;
/// # pub extern crate arcana_id as id;
/// #[derive(arcana_id::HasStid)]
/// #[stid(1.0f32)]
/// struct Invalid;
/// # fn main() {}
/// ```
pub trait HasStid: 'static {
    fn stid() -> Stid
    where
        Self: Sized;

    fn stid_dyn(&self) -> Stid;
}

impl Stid {
    /// Returns stable identifier of a type.
    pub fn of<T>() -> Self
    where
        T: HasStid,
    {
        T::stid()
    }

    /// Returns stable identifier of a value's type.
    /// This works for trait objects.
    pub fn of_val<T>(value: &T) -> Self
    where
        T: HasStid + ?Sized,
    {
        value.stid_dyn()
    }
}

has_stid!(u8 = 0x0000_0000_0000_0001 @ arcana_id);
has_stid!(u16 = 0x0000_0000_0000_0002 @ arcana_id);
has_stid!(u32 = 0x0000_0000_0000_0003 @ arcana_id);
has_stid!(u64 = 0x0000_0000_0000_0004 @ arcana_id);
has_stid!(i8 = 0x0000_0000_0000_0005 @ arcana_id);
has_stid!(i16 = 0x0000_0000_0000_0006 @ arcana_id);
has_stid!(i32 = 0x0000_0000_0000_0007 @ arcana_id);
has_stid!(i64 = 0x0000_0000_0000_0008 @ arcana_id);
has_stid!(f32 = 0x0000_0000_0000_0009 @ arcana_id);
has_stid!(f64 = 0x0000_0000_0000_000A @ arcana_id);

has_stid!(bool = 0x0000_0000_0000_000B @ arcana_id);
has_stid!(smol_str::SmolStr = 0x0000_0000_0000_000C @ arcana_id);

has_stid!(palette::LinLuma = 0x0000_0000_0000_000D @ arcana_id);
has_stid!(palette::LinLumaa = 0x0000_0000_0000_000E @ arcana_id);
has_stid!(palette::Srgb = 0x0000_0000_0000_000F @ arcana_id);
has_stid!(palette::Srgba = 0x0000_0000_0000_0010 @ arcana_id);
has_stid!(palette::Hsv = 0x0000_0000_0000_0011 @ arcana_id);
has_stid!(palette::Hsva = 0x0000_0000_0000_0012 @ arcana_id);

has_stid!(TimeSpan = 0x0000_0000_0000_00041 @ arcana_id);
has_stid!(chrono::TimeDelta = 0x0000_0000_0000_00041 @ arcana_id);
has_stid!(::edict::entity::EntityId = 0x0000_0000_0000_0042 @ arcana_id);
