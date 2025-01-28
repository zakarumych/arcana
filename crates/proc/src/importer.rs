use proc_macro2::TokenStream;

pub fn importer(attr: proc_macro::TokenStream, item: syn::Item) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            TokenStream::from(attr),
            "unexpected attribute",
        ));
    }

    let name = match &item {
        syn::Item::Fn(item) => &item.sig.ident,
        syn::Item::Struct(item) => &item.ident,
        syn::Item::Union(item) => &item.ident,
        _ => {
            return Err(syn::Error::new_spanned(item, "expected function or a type"));
        }
    };

    let id = match &item {
        syn::Item::Fn(item) => {
            let ident = &item.sig.ident;
            quote::quote! { ::arcana::local_name_hash_id!(#ident) }
        }
        syn::Item::Struct(item) => {
            let ident = &item.ident;
            quote::quote! { ::arcana::local_name_hash_id!(#ident) }
        }
        syn::Item::Union(item) => {
            let ident = &item.ident;
            quote::quote! { ::arcana::local_name_hash_id!(#ident) }
        }
        _ => {
            return Err(syn::Error::new_spanned(item, "expected function or a type"));
        }
    };

    let new = match &item {
        syn::Item::Fn(item) => {
            let ident = &item.sig.ident;
            quote::quote! { #ident }
        }
        syn::Item::Struct(item) => {
            let ident = &item.ident;
            quote::quote! { #ident::new() }
        }
        syn::Item::Union(item) => {
            let ident = &item.ident;
            quote::quote! { #ident::new() }
        }
        _ => {
            return Err(syn::Error::new_spanned(item, "expected function or a type"));
        }
    };

    Ok(quote::quote! {
        ::arcana::plugin_ctor_add!(plugin => {
            let id: ::arcana::assets::import::ImporterId = #id;

            let add = |hub: &mut ::arcana::plugin::PluginsHub| {
                let id: ::arcana::assets::import::ImporterId = #id;
                hub.add_importer(id, #new);
            };

            let info = ::arcana::plugin::ImporterInfo {
                id,
                name: ::arcana::name!(#name),
                location: ::std::option::Option::Some(::arcana::plugin::Location {
                    file: std::string::String::from(::std::file!()),
                    line: ::std::line!(),
                    column: ::std::column!(),
                }),
            };

            plugin.add_importer(info, add);
        });

        #item
    })
}
