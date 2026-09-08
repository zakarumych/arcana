use proc_macro2::TokenStream;

pub fn component(
    attr: TokenStream,
    item: syn::ItemFn,
    registry_name: &syn::Ident,
) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "unexpected attribute arguments",
        ));
    }
    let sig = &item.sig;
    if !sig.inputs.is_empty() || sig.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            sig,
            "component factories must take no arguments",
        ));
    }
    if matches!(sig.safety, syn::Safety::Unsafe(_)) || sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            sig,
            "component factories must be safe and synchronous",
        ));
    }
    if !sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &sig.generics,
            "component factories must not have generic parameters",
        ));
    }
    let ty = match &sig.output {
        syn::ReturnType::Type(_, ty)
            if !matches!(**ty, syn::Type::ImplTrait(_) | syn::Type::Infer(_)) =>
        {
            ty
        }
        output => {
            return Err(syn::Error::new_spanned(
                output,
                "component factories must explicitly return a concrete component type",
            ));
        }
    };
    let ident = &sig.ident;
    Ok(quote::quote! {
        ::arcana::plugin::plugin_ctor_add!(#registry_name @ plugin => {
            let desc = plugin.add_component::<#ty>(
                ::arcana::name!(#ident),
                || <#ty as ::arcana::model::Reflect>::to_value(&self::#ident()),
            );
            desc.location = ::core::option::Option::Some(::arcana::plugin::Location {
                file: ::std::string::String::from(::core::file!()),
                line: ::core::line!(),
                column: ::core::column!(),
            });
        });

        #item
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_concrete_factories() {
        let item = syn::parse_quote!(
            pub fn position() -> Position {
                Position
            }
        );
        let output = component(TokenStream::new(), item, &syn::parse_quote!(REGISTRY))
            .expect("valid component factory");
        syn::parse2::<syn::File>(output).expect("generated items must parse");
    }

    #[test]
    fn rejects_invalid_factory_signatures() {
        let cases = [
            ("fn make(value: i32) -> C {}", "take no arguments"),
            ("unsafe fn make() -> C {}", "safe and synchronous"),
            ("async fn make() -> C {}", "safe and synchronous"),
            ("fn make<T>() -> C {}", "generic parameters"),
            ("fn make<'a>() -> C {}", "generic parameters"),
            ("fn make<const N: usize>() -> C {}", "generic parameters"),
            ("fn make() {}", "concrete component type"),
            ("fn make() -> impl Component {}", "concrete component type"),
            ("fn make() -> _ {}", "concrete component type"),
        ];
        for (source, message) in cases {
            let item = syn::parse_str(source).expect("valid Rust function syntax");
            let error = component(TokenStream::new(), item, &syn::parse_quote!(REGISTRY))
                .expect_err("invalid factory signature");
            assert!(error.to_string().contains(message), "{source}: {error}");
        }
    }

    #[test]
    fn rejects_attribute_arguments() {
        let item = syn::parse_quote!(
            fn make() -> C {}
        );
        assert!(
            component(
                quote::quote!(name = "custom"),
                item,
                &syn::parse_quote!(REGISTRY)
            )
            .is_err()
        );
    }
}
