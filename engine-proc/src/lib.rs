// extern crate proc_macro;

// use proc_macro::TokenStream;

// #[proc_macro_attribute]
// pub fn main_loop(_attr: TokenStream, item: TokenStream) -> TokenStream {
//     let syn::ItemFn {
//         attrs,
//         vis,
//         sig,
//         block,
//     } = syn::parse_macro_input!(item as syn::ItemFn);

//     let mut proxy_fn_sig = sig.clone();
//     proxy_fn_sig.asyncness = None;
//     proxy_fn_sig.inputs.clear();

//     if sig.inputs.len() != 1 {
//         return syn::Error::new_spanned(
//             sig.inputs,
//             "main_loop function must have exactly one argument",
//         )
//         .to_compile_error()
//         .into();
//     }

//     let arg = match &sig.inputs[0] {
//         syn::FnArg::Receiver(receiver) => {
//             return syn::Error::new_spanned(
//                 receiver,
//                 "main_loop function must have exactly one argument",
//             )
//             .to_compile_error()
//             .into();
//         }
//         syn::FnArg::Typed(pat_type) => pat_type,
//     };

//     let tokens = quote::quote_spanned! {
//         sig.fn_token.span =>
//         #(#attrs)*
//         #vis #proxy_fn_sig {
//             Loop::run(|#arg| async move {
//                 #block
//             });
//         }
//     };

//     tokens.into()
// }
