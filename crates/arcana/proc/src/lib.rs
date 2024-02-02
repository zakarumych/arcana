// extern crate proc_macro;

mod stid;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn stid(attr: TokenStream, item: TokenStream) -> TokenStream {
    let stid = if attr.is_empty() {
        None
    } else {
        Some(syn::parse_macro_input!(attr as stid::StidValue))
    };

    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    match stid::with_stid(stid, input) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
