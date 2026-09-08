use arcana_hash::{Hash64, stable_hash};

macro_rules! assert_literal_hash {
    ($literal:literal) => {{
        const HASH: Hash64 = stable_hash!($literal);
        assert_eq!(HASH, stable_hash(&$literal));
    }};
}

#[test]
fn literal_hashes_match_runtime_values() {
    assert_literal_hash!("");
    assert_literal_hash!("my-plugin::Position");
    assert_literal_hash!("a long string that exercises hashing beyond a single block");
    assert_literal_hash!(b"binary\0\xff");
    assert_literal_hash!('\u{1f600}');
    assert_literal_hash!(b'\xff');
    assert_literal_hash!(true);
    assert_literal_hash!(false);
    assert_literal_hash!(42);
    assert_literal_hash!(255u8);
    assert_literal_hash!(65535u16);
    assert_literal_hash!(0xffff_ffffu32);
    assert_literal_hash!(0xffff_ffff_ffff_ffffu64);
    assert_literal_hash!(0x10000000000000000u128);
    assert_literal_hash!(127i8);
    assert_literal_hash!(32767i16);
    assert_literal_hash!(2147483647i32);
    assert_literal_hash!(9223372036854775807i64);
    assert_literal_hash!(0x10000000000000000i128);
}

#[test]
fn equivalent_literal_spellings_have_equal_hashes() {
    const ESCAPED: Hash64 = stable_hash!("my\x2dplugin::\u{50}osition",);
    const RAW: Hash64 = stable_hash!(r"my-plugin::Position");
    assert_eq!(ESCAPED, RAW);
    assert_eq!(stable_hash!(b"a\x62c"), stable_hash!(br"abc"));
    assert_eq!(stable_hash!(0xffu16), stable_hash!(255u16));
}
