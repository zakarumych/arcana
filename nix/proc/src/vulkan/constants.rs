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
    let name = &input.ident;
    let vis = &input.vis;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            input.generics,
            "generic arguments are not supported by `#[derive(Constants)]`",
        ));
    }

    let data = match input.data {
        syn::Data::Struct(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "only structs are supported by `#[derive(Constants)]`",
            ))
        }
    };

    let name_pod = quote::format_ident!("NixGenerated{}Pod", name);

    let field_types = data
        .fields
        .iter()
        .map(|field| &field.ty)
        .collect::<Vec<_>>();

    let field_pad_sizes = data
        .fields
        .iter()
        .enumerate()
        .map(|(idx, field)| {
            let end = data.fields
                .iter()
                .take(idx)
                .fold(quote! { 0 }, |acc, field| {
                    let ty = &field.ty;
                    quote_spanned! { ty.span() => #acc + nix::proc_macro::size_of::<<#ty as nix::proc_macro::Constants>::Pod>() }
                });

            let ty = &field.ty;
            quote::quote_spanned! {
                ty.span() => nix::proc_macro::pad_for::<#ty>(#end)
            }
        })
        .collect::<Vec<_>>();

    let tail = data.fields
        .iter()
        .fold(quote! { 0 }, |acc, field| {
            let ty = &field.ty;
            quote_spanned! { ty.span() => #acc + nix::proc_macro::size_of::<<#ty as nix::proc_macro::Constants>::Pod>() }
        });

    let tail_pad = quote::quote!(nix::proc_macro::pad_align(#tail, 16));

    match data.fields {
        syn::Fields::Named(_) => {
            let field_names = data
                .fields
                .iter()
                .map(|field| &field.ident)
                .collect::<Vec<_>>();

            let pad_names = data
                .fields
                .iter()
                .map(|field| quote::format_ident!("_pad_for{}", field.ident.as_ref().unwrap()))
                .collect::<Vec<_>>();

            let tokens = quote::quote! {
                #[repr(C, align(16))]
                #[doc(hidden)]
                #[derive(Clone, Copy, Debug)]
                #vis struct #name_pod {
                    #(
                        #pad_names: [u8; #field_pad_sizes],
                        #field_names: <#field_types as nix::proc_macro::Constants>::Pod,
                    )*
                    _nix_tail_pad: [u8; #tail_pad],
                }

                unsafe impl nix::proc_macro::Zeroable for #name_pod {}
                unsafe impl nix::proc_macro::Pod for #name_pod {}

                impl nix::proc_macro::Constants for #name {
                    type Pod = #name_pod;

                    fn as_pod(&self) -> #name_pod {
                        #name_pod {
                            #(
                                #pad_names: [0xDAu8; #field_pad_sizes],
                                #field_names: self.#field_names.as_pod(),
                            )*
                            _nix_tail_pad: [0xDAu8; #tail_pad],
                        }
                    }
                }
            };

            Ok(tokens)
        }
        _ => todo!(),
    }
}
