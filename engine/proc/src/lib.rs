// extern crate proc_macro;

mod codex;
mod component;
mod filter;
mod importer;
mod init;
mod job;
mod literal_hash;
mod plugin;
mod stable_hasher;
mod stid;
mod system;

use proc_macro::TokenStream;
use stid::HasStid;

/// Exports a component factory from a plugin.
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    match component::component(attr.into(), item, &plugin_registry_name()) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(Component, attributes(edict))]
pub fn derive_component(item: TokenStream) -> TokenStream {
    let path: syn::Path = syn::parse_quote!(arcana::ecs);
    let namespace = syn::parse_quote!(arcana);
    edict_proc_lib::derive_component(item.into(), &path, &namespace).into()
}

#[proc_macro_derive(Relation, attributes(edict))]
pub fn derive_relation(item: TokenStream) -> TokenStream {
    let path: syn::Path = syn::parse_quote!(arcana::ecs);
    let namespace = syn::parse_quote!(arcana);
    edict_proc_lib::derive_relation(item.into(), &path, &namespace).into()
}

#[proc_macro_derive(HasStid, attributes(stid))]
pub fn derive_has_stid(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    fn parse_stid_attr(attr: &syn::Attribute) -> syn::Result<syn::Expr> {
        match &attr.meta {
            syn::Meta::NameValue(name_value) => Ok(name_value.value.clone()),
            syn::Meta::List(_) => attr.parse_args(),
            meta => Err(syn::Error::new_spanned(
                meta,
                "expected #[stid = literal] or #[stid(const_expression)]",
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
        Some(Err(error)) => return error.to_compile_error().into(),
        None => None,
    };

    match stid::has_stid(stid, &input) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn has_stid(tokens: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokens as HasStid);

    match stid::has_stid_fn(input) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn stable_hash_tokens(tokens: TokenStream) -> TokenStream {
    let hash = stable_hasher::stable_hash(&tokens.to_string());
    quote::quote!(#hash).into()
}

#[proc_macro]
pub fn stable_hash_literal(tokens: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokens as syn::Expr);
    match literal_hash::stable_hash_literal(&input) {
        Ok(hash) => quote::quote!(#hash).into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Exports function as filter.
#[proc_macro_attribute]
pub fn filter(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    match filter::filter(attr, item) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Exports function as system.
#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    match system::system(attr, item) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Exports function as system.
#[proc_macro_attribute]
pub fn job(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemImpl);
    match job::job(attr, item) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Exports function as system.
#[proc_macro_attribute]
pub fn importer(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemImpl);
    match importer::importer(attr, item) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Exports function as system.
#[proc_macro_attribute]
pub fn init(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    match init::init(attr, item) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Exports function as system.
#[proc_macro_attribute]
pub fn codex(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    match codex::codex(attr, item) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Exports function as filter.
#[proc_macro]
pub fn declare_plugin(tokens: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(tokens as plugin::PluginItem);
    match plugin::plugin(item) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn plugin_registry_name() -> syn::Ident {
    let caller = std::env::var("CARGO_PKG_NAME").expect("'CARGO_PKG_NAME' must be set");
    quote::format_ident!("ARCANA_PLUGIN_REGISTRY_{}", caller.replace('-', "_"))
}
