use proc_macro2::TokenStream;

pub fn importer(attr: proc_macro::TokenStream, item: syn::ItemImpl) -> syn::Result<TokenStream> {
    let mut tokens = TokenStream::new();

    let importer_trait_path = match item.trait_ {
        None => {
            return Err(syn::Error::new_spanned(
                item,
                "expected `Importer` trait implementation",
            ));
        }
        Some((Some(_), _, _)) => {
            return Err(syn::Error::new_spanned(
                item,
                "expected non-negative `Importer` trait implementation",
            ));
        }
        Some((_, ref path, _)) => path,
    };

    let type_path = match *item.self_ty {
        syn::Type::Path(ref type_path) => type_path,
        _ => {
            return Err(syn::Error::new_spanned(
                item.self_ty,
                "expected `Importer` implementation for a type",
            ));
        }
    };

    let type_last_segment = type_path.path.segments.last().unwrap();

    let ident = &type_last_segment.ident;

    if !attr.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "`#[importer]` does not accept any arguments",
        ));
    }

    tokens.extend(quote::quote! {
        ::arcana::plugin_ctor_add!(plugin => {
            #[allow(dead_code)]
            fn importer_is_importer<T: #importer_trait_path>() {
                ::arcana::for_macro::is_importer::<T>();
            }
            importer_is_importer::<#type_path>();

            let id: ::arcana::assets::import::ImporterId = ::arcana::local_name_hash_id!(#ident);

            let add = |hub: &mut ::arcana::plugin::PluginsHub| {
                let id: ::arcana::assets::import::ImporterId = ::arcana::local_name_hash_id!(#ident);
                hub.add_importer(id, < #type_path as ::arcana::assets::import::Importer >::new());
            };

            let info = ::arcana::plugin::ImporterInfo {
                id,
                name: < #type_path as ::arcana::assets::import::Importer >::name(),
                location: ::std::option::Option::Some(::arcana::plugin::Location {
                    file: std::string::String::from(::std::file!()),
                    line: ::std::line!(),
                    column: ::std::column!(),
                }),
            };

            plugin.add_importer(info, add);
        });

        #item
    });

    Ok(tokens)
}
