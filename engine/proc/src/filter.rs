use proc_macro2::TokenStream;

use crate::plugin_registry_name;

pub fn filter(attr: proc_macro::TokenStream, item: syn::ItemFn) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            TokenStream::from(attr),
            "unexpected attribute",
        ));
    }

    let registry_name = plugin_registry_name();

    let ident = &item.sig.ident;
    Ok(quote::quote! {
        ::arcana::plugin::plugin_ctor_add!(#registry_name @ plugin => {
            let id: ::arcana::input::FilterId = ::arcana::id::local_name_hash_id!(#ident);

            let add = |hub: &mut ::arcana::plugin::PluginsHub| {
                let id: ::arcana::input::FilterId = ::arcana::id::local_name_hash_id!(#ident);
                hub.add_filter(id, #ident);
            };

            let info = ::arcana::plugin::FilterInfo {
                id,
                name: ::arcana::name!(#ident),
                location: ::std::option::Option::Some(::arcana::plugin::Location {
                    file: std::string::String::from(::std::file!()),
                    line: ::std::line!(),
                    column: ::std::column!(),
                }),
            };

            plugin.add_filter(info, add);
        });

        #item
    })
}
