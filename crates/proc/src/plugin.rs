use proc_macro2::TokenStream;

pub fn plugin() -> syn::Result<TokenStream> {
    // let plugin_name = std::env::var("CARGO_PKG_NAME").unwrap();
    // let registry = quote::format_ident!("{}_PLUGIN_REGISTRY", plugin_name.to_uppercase());

    let mut items: Vec<syn::Item> = Vec::new();

    items.push(syn::parse_quote!(
        pub fn dependency() -> ::arcana::project::Dependency {
            ::arcana::project::Dependency::Crates(env!("CARGO_PKG_VERSION").to_owned())
        }
    ));

    items.push(syn::parse_quote!(
        pub fn git_dependency(git: &str, branch: Option<&str>) -> ::arcana::project::Dependency {
            ::arcana::project::Dependency::Git {
                git: git.to_owned(),
                branch: branch.map(str::to_owned),
            }
        }
    ));

    items.push(syn::parse_quote!(
        pub fn path_dependency() -> ::arcana::project::Dependency {
            ::arcana::project::Dependency::from_path(env!("CARGO_MANIFEST_DIR")).unwrap()
        }
    ));

    items.push(syn::parse_quote!(
        pub fn __arcana_plugin() -> Box<dyn ::arcana::plugin::ArcanaPlugin> {
            let plugin = unsafe { crate::PLUGIN_REGISTRY.plugin() };
            Box::new(plugin)
        }
    ));

    items.push(syn::parse_quote!(
        static mut PLUGIN_REGISTRY: ::arcana::plugin::init::Registry =
            ::arcana::plugin::init::Registry::new();
    ));

    let tokens = quote::quote! {
        #(#items)*
    };

    Ok(tokens)
}
