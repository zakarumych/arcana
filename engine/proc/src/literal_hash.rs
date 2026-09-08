use std::hash::{BuildHasher, Hash, Hasher};

use foldhash::fast::FixedState;

fn hash<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut hasher = FixedState::with_seed(17).build_hasher();
    value.hash(&mut hasher);
    hasher.finish()
}

pub fn stable_hash_literal(mut input: &syn::Expr) -> syn::Result<u64> {
    while let syn::Expr::Group(group) = input {
        input = &group.expr;
    }

    match input {
        syn::Expr::Lit(expr) => match &expr.lit {
            syn::Lit::Str(lit) => Ok(hash(&lit.value())),
            syn::Lit::ByteStr(lit) => Ok(hash(&lit.value())),
            syn::Lit::Char(lit) => Ok(hash(&lit.value())),
            syn::Lit::Byte(lit) => Ok(hash(&lit.value())),
            syn::Lit::Bool(lit) => Ok(hash(&lit.value)),
            syn::Lit::Int(lit) => match lit.suffix() {
                "u8" => Ok(hash(&lit.base10_parse::<u8>()?)),
                "u16" => Ok(hash(&lit.base10_parse::<u16>()?)),
                "u32" => Ok(hash(&lit.base10_parse::<u32>()?)),
                "u64" => Ok(hash(&lit.base10_parse::<u64>()?)),
                "u128" => Ok(hash(&lit.base10_parse::<u128>()?)),
                "i8" => Ok(hash(&lit.base10_parse::<i8>()?)),
                "i16" => Ok(hash(&lit.base10_parse::<i16>()?)),
                "" | "i32" => Ok(hash(&lit.base10_parse::<i32>()?)),
                "i64" => Ok(hash(&lit.base10_parse::<i64>()?)),
                "i128" => Ok(hash(&lit.base10_parse::<i128>()?)),
                _ => Err(syn::Error::new_spanned(
                    lit,
                    "expected a fixed-width integer suffix, or no suffix for i32",
                )),
            },
            lit => Err(syn::Error::new_spanned(
                lit,
                "expected a string, byte string, character, byte, boolean, or integer literal",
            )),
        },
        expr => Err(syn::Error::new_spanned(expr, "expected a literal")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarded_literals_are_unwrapped() {
        let literal = quote::quote!("my-plugin::Position");
        let group = proc_macro2::Group::new(proc_macro2::Delimiter::None, literal);
        let input = syn::parse2(quote::quote!(#group)).expect("valid forwarded expression");
        assert_eq!(
            stable_hash_literal(&input).expect("supported literal"),
            hash("my-plugin::Position")
        );
    }

    #[test]
    fn rejects_unsupported_literals_and_expressions() {
        for input in ["1.0", "42usize", "256u8", "2147483648", "-1", "1 + 2"] {
            let input = syn::parse_str(input).expect("valid expression");
            assert!(stable_hash_literal(&input).is_err());
        }
    }
}
