use proc_macro::TokenStream;

#[cfg_attr(
    any(windows, all(unix, not(any(target_os = "macos", target_os = "ios")))),
    path = "vulkan/mod.rs"
)]
mod backend;

#[proc_macro_derive(Arguments, attributes(mev))]
pub fn arguments_derive(input: TokenStream) -> TokenStream {
    backend::arguments::derive(input)
}

#[proc_macro_derive(Constants, attributes(mev))]
pub fn constants_derive(input: TokenStream) -> TokenStream {
    backend::constants::derive(input)
}
