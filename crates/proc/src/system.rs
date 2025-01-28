use proc_macro2::TokenStream;

pub fn system(attr: proc_macro::TokenStream, item: syn::ItemFn) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            TokenStream::from(attr),
            "unexpected attribute",
        ));
    }

    let ident = &item.sig.ident;
    Ok(quote::quote! {
        ::arcana::plugin_ctor_add!(plugin => {
            let id: ::arcana::plugin::SystemId = ::arcana::local_name_hash_id!(#ident);

            let add = |hub: &mut ::arcana::plugin::PluginsHub| {
                let id: ::arcana::plugin::SystemId = ::arcana::local_name_hash_id!(#ident);
                hub.add_system(id, #ident);
            };

            let info = ::arcana::plugin::SystemInfo {
                id,
                name: ::arcana::name!(#ident),
                location: ::std::option::Option::Some(::arcana::plugin::Location {
                    file: std::string::String::from(::std::file!()),
                    line: ::std::line!(),
                    column: ::std::column!(),
                }),
            };

            plugin.add_system(info, add);
        });

        #item
    })
}
