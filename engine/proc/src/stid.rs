use proc_easy::EasyMaybe;
use proc_macro2::TokenStream;

use std::hash::{Hash, Hasher};

use crate::stable_hasher::{stable_hash, stable_hasher};

proc_easy::easy_parse! {
    struct AssignStid {
        assign: syn::Token![=],
        stid: syn::Expr,
    }
}

proc_easy::easy_parse! {
    struct FromPath {
        at: syn::Token![@],
        path: syn::Path,
    }
}

proc_easy::easy_parse! {
    pub struct HasStid {
        ty: syn::Type,
        assign: proc_easy::EasyMaybe<AssignStid>,
        from_path: proc_easy::EasyMaybe<FromPath>,
    }
}

fn hash_derive_input(input: &syn::DeriveInput) -> u64 {
    let mut hasher = stable_hasher();

    for lt in input.generics.lifetimes() {
        lt.hash(&mut hasher);
    }

    for tp in input.generics.type_params() {
        tp.ident.hash(&mut hasher);
    }

    match input.data {
        syn::Data::Struct(ref data) => {
            for field in &data.fields {
                field.ident.hash(&mut hasher);
                field.ty.hash(&mut hasher);
            }
        }
        syn::Data::Enum(ref data) => {
            for variant in &data.variants {
                variant.ident.hash(&mut hasher);
                for field in &variant.fields {
                    field.ty.hash(&mut hasher);
                }
            }
        }
        syn::Data::Union(ref data) => {
            for field in &data.fields.named {
                field.ident.hash(&mut hasher);
                field.ty.hash(&mut hasher);
            }
        }
    }

    input.ident.hash(&mut hasher);
    hasher.finish()
}

fn base_id(
    mut stid: Option<&syn::Expr>,
    inferred_id: u64,
    from_path: &syn::Path,
) -> syn::Result<TokenStream> {
    while let Some(syn::Expr::Group(group)) = stid {
        stid = Some(&group.expr);
    }

    match stid {
        None => {
            let id = inferred_id | 0x8000_0000_0000_0000;
            Ok(quote::quote!(#id))
        }
        Some(syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(int),
            ..
        })) => {
            let id = int.base10_parse::<u64>()?;
            if id == 0 {
                return Err(syn::Error::new(int.span(), "STID must not be 0"));
            }
            Ok(quote::quote!(#id))
        }
        Some(
            expr @ syn::Expr::Lit(syn::ExprLit {
                lit:
                    syn::Lit::Str(_)
                    | syn::Lit::ByteStr(_)
                    | syn::Lit::Char(_)
                    | syn::Lit::Byte(_)
                    | syn::Lit::Bool(_),
                ..
            }),
        ) => {
            let id = crate::literal_hash::stable_hash_literal(expr)? | 0x8000_0000_0000_0000;
            Ok(quote::quote!(#id))
        }
        Some(expr) => Ok(quote::quote!({
            let value = const { #expr };
            let mut hasher = #from_path::for_macro::stable_hasher();
            ::core::hash::Hash::hash(&value, &mut hasher);
            ::core::hash::Hasher::finish(&hasher) | 0x8000_0000_0000_0000
        })),
    }
}

pub fn has_stid(stid: Option<syn::Expr>, input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let from_path = syn::parse_quote!(::arcana::id);
    let base_id = base_id(stid.as_ref(), hash_derive_input(input), &from_path)?;

    let mut generics = input.generics.clone();
    let combined_ids: TokenStream;

    if input.generics.type_params().next().is_some() {
        let where_clause = generics.make_where_clause();
        for tp in input.generics.type_params() {
            let ident = &tp.ident;
            where_clause.predicates.push(syn::parse_quote! {
                #ident: ::arcana::id::HasStid
            });
        }

        let ids = input.generics.type_params().map(|tp| {
            let ident = &tp.ident;
            quote::quote! {
                <#ident as ::arcana::id::HasStid>::stid().get()
            }
        });

        combined_ids = quote::quote!({
            let mut hasher = ::arcana::id::for_macro::stable_hasher();
            ::core::hash::Hash::hash(&#base_id, &mut hasher);
            #(::core::hash::Hash::hash(&#ids, &mut hasher);)*
            ::core::hash::Hasher::finish(&hasher) | 0x8000_0000_0000_0000
        });
    } else {
        combined_ids = base_id;
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let output = quote::quote! {
        impl #impl_generics ::arcana::id::HasStid for #name #ty_generics #where_clause {
            #[inline(always)]
            fn stid() -> ::arcana::id::Stid {
                let id = #combined_ids;
                ::arcana::id::Stid::new(::core::num::NonZeroU64::new(id).unwrap())
            }

            #[inline(always)]
            fn stid_dyn(&self) -> ::arcana::id::Stid {
                <Self as ::arcana::id::HasStid>::stid()
            }
        }
    };

    Ok(output)
}

pub fn has_stid_fn(input: HasStid) -> syn::Result<TokenStream> {
    let stid = match input.assign {
        EasyMaybe::Nothing => None,
        EasyMaybe::Just(AssignStid { assign: _, stid }) => Some(stid),
    };

    let ty = input.ty;
    let from_path = match input.from_path {
        EasyMaybe::Just(FromPath { path, .. }) => path,
        EasyMaybe::Nothing => syn::parse_quote!(::arcana::id),
    };

    let base_id = base_id(stid.as_ref(), stable_hash(&ty), &from_path)?;

    let output = quote::quote! {
        impl #from_path::HasStid for #ty {
            #[inline(always)]
            fn stid() -> #from_path::Stid {
                #from_path::Stid::new(::core::num::NonZeroU64::new(#base_id).unwrap())
            }

            #[inline(always)]
            fn stid_dyn(&self) -> #from_path::Stid {
                <Self as #from_path::HasStid>::stid()
            }
        }
    };

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_rejects_zero_and_out_of_range_integer_ids() {
        let input = syn::parse_quote!(
            struct Example;
        );
        for value in ["0", "0u64", "0x0", "18446744073709551616"] {
            let expr = syn::parse_str(value).expect("valid integer expression");
            assert!(has_stid(Some(expr), &input).is_err(), "{value}");
        }
    }

    #[test]
    fn function_macro_rejects_zero_and_out_of_range_integer_ids() {
        for value in ["0", "0u64", "0x0", "18446744073709551616"] {
            let input = syn::parse_str(&format!("Example = {value} @ arcana_id"))
                .expect("valid macro syntax");
            assert!(has_stid_fn(input).is_err(), "{value}");
        }
    }

    #[test]
    fn literals_expand_to_compile_time_hashing() {
        let input = syn::parse_quote!(
            struct Example;
        );
        let literal = syn::parse_quote!("my-plugin::Position");
        let id = crate::literal_hash::stable_hash_literal(&literal).expect("supported literal")
            | 0x8000_0000_0000_0000;
        let id_token = quote::quote!(#id).to_string();
        let derive = has_stid(Some(literal), &input)
            .expect("supported literal")
            .to_string();
        let function = has_stid_fn(syn::parse_quote!(Example = "my-plugin::Position" @ ids))
            .expect("supported literal")
            .to_string();
        for output in [derive, function] {
            assert!(output.contains(&id_token));
            assert!(!output.contains("stable_hash"));
        }
    }
}
