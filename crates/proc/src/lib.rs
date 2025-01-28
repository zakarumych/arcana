// extern crate proc_macro;

mod filter;
mod importer;
mod init;
mod job;
mod stable_hasher;
mod stid;
mod system;

use proc_macro::TokenStream;
use stid::HasStid;

// #[proc_macro_attribute]
// pub fn stid(attr: TokenStream, item: TokenStream) -> TokenStream {
//     let stid = if attr.is_empty() {
//         None
//     } else {
//         Some(syn::parse_macro_input!(attr as stid::StidValue))
//     };

//     let input = syn::parse_macro_input!(item as syn::DeriveInput);

//     match stid::with_stid(stid, &input) {
//         Ok(mut output) => {
//             output.extend(input.to_token_stream());
//             output.into()
//         }
//         Err(err) => err.to_compile_error().into(),
//     }
// }

#[proc_macro_derive(HasStid, attributes(stid))]
pub fn derive_with_stid(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    fn parse_stid_attr(attr: &syn::Attribute) -> syn::Result<stid::StidValue> {
        let name_value = attr.meta.require_name_value()?;
        match &name_value.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit),
                ..
            }) => Ok(stid::StidValue::Str(lit.clone())),
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(lit),
                ..
            }) => Ok(stid::StidValue::Int(lit.clone())),
            value => Err(syn::Error::new_spanned(
                value,
                "expected string or integer literal",
            )),
        }
    }

    let r = input.attrs.iter().find_map(|attr| {
        if attr.meta.path().is_ident("stid") {
            Some(parse_stid_attr(attr))
        } else {
            None
        }
    });

    let stid = match r {
        Some(Ok(stid)) => Some(stid),
        Some(Err(err)) => return err.to_compile_error().into(),
        None => None,
    };

    match stid::with_stid(stid, &input) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn with_stid(tokens: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokens as HasStid);

    match stid::with_stid_fn(input) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn stable_hash_tokens(tokens: TokenStream) -> TokenStream {
    let hash = stable_hasher::stable_hash(&tokens.to_string());
    quote::quote!(#hash).into()
}

/// Exports function as filter.
#[proc_macro_attribute]
pub fn filter(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    match filter::filter(attr, item) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Exports function as system.
#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    match system::system(attr, item) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Exports function as system.
#[proc_macro_attribute]
pub fn job(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemStruct);
    match job::job(attr, item) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Exports function as system.
#[proc_macro_attribute]
pub fn importer(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::Item);
    match importer::importer(attr, item) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Exports function as system.
#[proc_macro_attribute]
pub fn init(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    match init::init(attr, item) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

// /// Exports function as filter.
// #[proc_macro]
// pub fn plugin(_tokens: TokenStream) -> TokenStream {
//     match plugin::plugin() {
//         Ok(output) => output.into(),
//         Err(err) => err.to_compile_error().into(),
//     }
// }
