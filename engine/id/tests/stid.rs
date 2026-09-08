extern crate self as arcana;

pub use arcana_id as id;

use arcana_id::{HasStid, has_stid};

fn hashed<T: core::hash::Hash>(value: &T) -> u64 {
    let mut hasher = id::for_macro::stable_hasher();
    core::hash::Hash::hash(value, &mut hasher);
    core::hash::Hasher::finish(&hasher) | 0x8000_0000_0000_0000
}

#[derive(HasStid)]
#[stid = 0x1234]
struct Exact;

#[derive(HasStid)]
#[stid(0x1234)]
struct ExactList;

struct ExactFunction;
has_stid!(ExactFunction = 0x1234);

struct HighBit;
has_stid!(HighBit = 0x8000_0000_0000_0001 @ id);

macro_rules! assign_stid {
    ($name:ident, $value:expr) => {
        struct $name;
        has_stid!($name = $value);
    };
}

assign_stid!(ForwardedExact, 0x1234);

#[test]
fn integer_literals_assign_exact_ids() {
    assert_eq!(Exact::stid().get(), 0x1234);
    assert_eq!(ExactList::stid(), Exact::stid());
    assert_eq!(ExactFunction::stid(), Exact::stid());
    assert_eq!(ForwardedExact::stid(), Exact::stid());
    assert_eq!(HighBit::stid().get(), 0x8000_0000_0000_0001);
    assert_eq!(u8::stid().get(), 1);
    assert_eq!(u64::stid().get(), 4);
}

const KEY: (&str, u32) = ("my-plugin::Position", 1);

const fn key() -> (&'static str, u32) {
    KEY
}

#[derive(HasStid)]
#[stid(KEY)]
struct Named;

#[derive(HasStid)]
#[stid(key())]
struct Called;

#[derive(HasStid)]
#[stid(("my-plugin::Position", 1u32))]
struct Tuple;

struct Function;
has_stid!(Function = { let value = key(); value } @ crate::id);

#[test]
fn expressions_hash_evaluated_values() {
    assert_eq!(Named::stid().get(), hashed(&KEY));
    assert_eq!(Called::stid(), Named::stid());
    assert_eq!(Tuple::stid(), Named::stid());
    assert_eq!(Function::stid(), Named::stid());
    let value: &dyn HasStid = &Function;
    assert_eq!(value.stid_dyn(), Function::stid());
    assert_eq!(Named.stid_dyn(), Named::stid());
}

const NUMBER: u32 = 42;

#[derive(HasStid)]
#[stid(NUMBER)]
struct NumericConst;

struct Arithmetic;
has_stid!(Arithmetic = 40u32 + 2);

#[test]
fn numeric_expressions_are_hashed() {
    assert_eq!(NumericConst::stid().get(), hashed(&NUMBER));
    assert_eq!(Arithmetic::stid(), NumericConst::stid());
}

#[derive(HasStid)]
#[stid = "deadbeef"]
struct StringValue;

#[derive(HasStid)]
#[stid("deadbeef")]
struct StringList;

struct StringFunction;
has_stid!(StringFunction = "deadbeef" @ crate::id);

const STRING_KEY: &str = "deadbeef";

#[derive(HasStid)]
#[stid(STRING_KEY)]
struct NamedString;

#[derive(HasStid)]
#[stid(r"deadbeef")]
struct RawString;

#[test]
fn strings_are_hashed_even_when_they_look_hexadecimal() {
    assert_eq!(StringValue::stid().get(), hashed(&"deadbeef"));
    assert_eq!(StringList::stid(), StringValue::stid());
    assert_eq!(StringFunction::stid(), StringValue::stid());
    assert_eq!(NamedString::stid(), StringValue::stid());
    assert_eq!(RawString::stid(), StringValue::stid());
}

struct ByteString;
has_stid!(ByteString = b"my-plugin\0\xff");

#[derive(HasStid)]
#[stid('\u{1f600}')]
struct Character;

struct Byte;
has_stid!(Byte = b'\xff');

#[derive(HasStid)]
#[stid(true)]
struct Boolean;

#[derive(HasStid)]
#[stid(c"my-plugin")]
struct CString;

#[test]
fn noninteger_literals_hash_their_values() {
    assert_eq!(ByteString::stid().get(), hashed(&b"my-plugin\0\xff"));
    assert_eq!(Character::stid().get(), hashed(&'\u{1f600}'));
    assert_eq!(Byte::stid().get(), hashed(&b'\xff'));
    assert_eq!(Boolean::stid().get(), hashed(&true));
    assert_eq!(CString::stid().get(), hashed(&c"my-plugin"));
}

#[derive(core::hash::Hash)]
struct Key {
    namespace: &'static str,
    version: u16,
}

#[derive(HasStid)]
#[stid(Key { namespace: "my-plugin", version: 2 })]
struct Custom;

struct Array;
has_stid!(Array = [1u8, 2, 3] @ id);

#[test]
fn compound_values_use_their_hash_implementation() {
    assert_eq!(
        Custom::stid().get(),
        hashed(&Key {
            namespace: "my-plugin",
            version: 2,
        })
    );
    assert_eq!(Array::stid().get(), hashed(&[1u8, 2, 3]));
}

#[derive(HasStid)]
#[stid("my-plugin::Generic")]
struct Generic<T = u8>(T)
where
    T: Copy;

#[test]
fn generic_ids_combine_base_and_type_ids() {
    let mut hasher = id::for_macro::stable_hasher();
    core::hash::Hash::hash(&hashed(&"my-plugin::Generic"), &mut hasher);
    core::hash::Hash::hash(&u8::stid().get(), &mut hasher);
    let expected = core::hash::Hasher::finish(&hasher) | 0x8000_0000_0000_0000;
    assert_eq!(Generic::<u8>::stid().get(), expected);
    assert_ne!(Generic::<u8>::stid(), Generic::<u16>::stid());
    assert_eq!(Generic(1u8).stid_dyn(), Generic::<u8>::stid());
}

#[derive(HasStid)]
struct Inferred;

struct InferredFunction;
has_stid!(InferredFunction);

#[test]
fn inferred_ids_are_nonzero_and_match_dynamic_ids() {
    assert_ne!(Inferred::stid().get() & (1 << 63), 0);
    assert_ne!(InferredFunction::stid().get() & (1 << 63), 0);
    assert_eq!(Inferred.stid_dyn(), Inferred::stid());
    assert_eq!(InferredFunction.stid_dyn(), InferredFunction::stid());
}
