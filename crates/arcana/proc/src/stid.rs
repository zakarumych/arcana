use ahash::AHasher;
use proc_macro2::{Span, TokenStream};

use std::hash::{Hash, Hasher};

proc_easy::easy_parse! {
    pub enum StidValue {
        Int(syn::LitInt),
        Str(syn::LitStr),
    }
}

fn hash_id(input: &syn::DeriveInput) -> u64 {
    let mut hasher = AHasher::new_with_keys(
        0x2360_ED05_1FC6_5DA4_4385_DF64_9FCC_F645,
        0x5851_F42D_4C95_7F2D_1405_7B7E_F767_814F,
    );

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

pub fn with_stid(stid: Option<StidValue>, input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;

    let base_id = match &stid {
        None => hash_id(&input),
        Some(StidValue::Int(int)) => int.base10_parse::<u64>()?,
        Some(StidValue::Str(str)) => {
            let s = str
                .value()
                .trim()
                .chars()
                .filter(|c| "0123456789abcdef".contains(*c))
                .collect::<String>();
            u64::from_str_radix(&s, 16).map_err(|err| {
                syn::Error::new(str.span(), format!("Failed to parse STID: {}", err))
            })?
        }
    };

    if base_id == 0 {
        return Err(syn::Error::new(
            match &stid {
                None => Span::call_site(),
                Some(StidValue::Int(int)) => int.span(),
                Some(StidValue::Str(str)) => str.span(),
            },
            "STID must not be 0",
        ));
    }

    let mut generics = input.generics.clone();
    let combined_ids: TokenStream;

    if input.generics.type_params().next().is_some() {
        let where_clause = generics.make_where_clause();
        for tp in input.generics.type_params() {
            let ident = &tp.ident;
            where_clause.predicates.push(syn::parse_quote! {
                #ident: ::arcana::stid::WithStid
            });
        }

        let ids = input.generics.type_params().map(|tp| {
            let ident = &tp.ident;
            quote::quote! {
                <#ident as ::arcana::stid::WithStid>::stid().get()
            }
        });

        combined_ids = quote::quote!({
            let mut hasher = ::arcana::stable_hasher();
            ::core::hash::Hash::hash(&#base_id, &mut hasher);
            #(::core::hash::Hash::hash(&#ids, &mut hasher);)*
            hasher.finish() | 1
        });
    } else {
        combined_ids = quote::quote!(#base_id);
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let output = quote::quote! {
        #input

        impl #impl_generics ::arcana::stid::WithStid for #name #ty_generics #where_clause {
            #[inline(always)]
            fn stid() -> Stid {
                let id = #combined_ids;
                Stid::new(unsafe { ::core::num::NonZeroU64::new_unchecked(id) })
            }

            #[inline(always)]
            fn stid_dyn(&self) -> Stid {
                Self::stid()
            }
        }
    };

    Ok(output)
}
