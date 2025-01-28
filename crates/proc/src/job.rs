use proc_macro2::TokenStream;

pub fn job(attr: proc_macro::TokenStream, item: syn::ItemImpl) -> syn::Result<TokenStream> {
    let mut tokens = TokenStream::new();

    let job_trait_path = match item.trait_ {
        None => {
            return Err(syn::Error::new_spanned(
                item,
                "expected `Job` trait implementation",
            ));
        }
        Some((Some(_), _, _)) => {
            return Err(syn::Error::new_spanned(
                item,
                "expected non-negative `Job` trait implementation",
            ));
        }
        Some((_, ref path, _)) => path,
    };

    let type_path = match *item.self_ty {
        syn::Type::Path(ref type_path) => type_path,
        _ => {
            return Err(syn::Error::new_spanned(
                item.self_ty,
                "expected `Job` implementation for a type",
            ));
        }
    };

    let type_last_segment = type_path.path.segments.last().unwrap();

    let ident = &type_last_segment.ident;

    if !attr.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "`#[job]` does not accept any arguments",
        ));
    }

    tokens.extend(quote::quote! {
        ::arcana::plugin_ctor_add!(plugin => {
            fn job_is_job<T: #job_trait_path>() {
                ::arcana::for_macro::is_job::<T>();
            }
            job_is_job::<#type_path>();

            let id: ::arcana::work::JobId = ::arcana::local_name_hash_id!(#ident);

            let add = |hub: &mut ::arcana::plugin::PluginsHub| {
                let id: ::arcana::work::JobId = ::arcana::local_name_hash_id!(#ident);
                hub.add_job(id, < #type_path as ::arcana::work::Job >::new());
            };

            let info = ::arcana::plugin::JobInfo {
                id,
                name: < #type_path as ::arcana::work::Job >::name(),
                desc: < #type_path as ::arcana::work::Job >::desc(),
                location: ::std::option::Option::Some(::arcana::plugin::Location {
                    file: std::string::String::from(::std::file!()),
                    line: ::std::line!(),
                    column: ::std::column!(),
                }),
            };

            plugin.add_job(info, add);
        });

        #item
    });

    Ok(tokens)
}
