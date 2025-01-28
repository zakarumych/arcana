use proc_macro2::TokenStream;

proc_easy::easy_parse! {
    enum JobAttr {
        NameStr(syn::LitStr),
        NameIdent(syn::Ident),
    }
}

pub fn job(attr: proc_macro::TokenStream, item: syn::ItemStruct) -> syn::Result<TokenStream> {
    let mut name = item.ident.to_string();

    if !attr.is_empty() {
        let attr = syn::parse::<JobAttr>(attr)?;

        match attr {
            JobAttr::NameStr(lit) => {
                name = lit.value();
            }
            JobAttr::NameIdent(ident) => {
                name = ident.to_string();
            }
        }
    }

    let ident = &item.ident;
    Ok(quote::quote! {
        ::arcana::plugin_ctor_add!(plugin => {
            let id: ::arcana::work::JobId = ::arcana::local_name_hash_id!(#ident);

            let add = |hub: &mut ::arcana::plugin::PluginsHub| {
                let id: ::arcana::work::JobId = ::arcana::local_name_hash_id!(#ident);
                hub.add_job(id, < #ident >::new());
            };

            let info = ::arcana::plugin::JobInfo {
                id,
                name: ::arcana::Name::from_str(#name).unwrap(),
                desc: < #ident >::desc(),
                location: ::std::option::Option::Some(::arcana::plugin::Location {
                    file: std::string::String::from(::std::file!()),
                    line: ::std::line!(),
                    column: ::std::column!(),
                }),
            };

            plugin.add_job(info, add);
        });

        #item
    })
}
