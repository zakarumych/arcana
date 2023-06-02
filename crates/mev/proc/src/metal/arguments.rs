use proc_easy::private::Spanned;
use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn;

pub fn derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    match derive_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_impl(input: syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    todo!()
}
