use proc_macro2::TokenStream;

use crate::plugin_registry_name;

proc_easy::easy_token!(path);
proc_easy::easy_token!(git);
proc_easy::easy_token!(branch);
proc_easy::easy_token!(rev);
proc_easy::easy_token!(dependencies);

proc_easy::easy_argument_value! {
    struct GitUrl {
        token: git,
        url: syn::LitStr,
    }
}

proc_easy::easy_argument_value! {
    struct GitBranch {
        token: branch,
        name: syn::LitStr,
    }
}

proc_easy::easy_argument_value! {
    struct GitRevision {
        token: rev,
        id: syn::LitStr,
    }
}

proc_easy::easy_terminated! {
    @(syn::Token![,])
    struct Git2 {
        url: GitUrl,
        branch: Option<GitBranch>,
        rev: Option<GitRevision>,
    }
}

proc_easy::easy_parse! {
    enum DepKind {
        Path(path),
        Path2(syn::Token![...]),
        Git(GitUrl),
        Git2(proc_easy::EasyBraced<Git2>),
    }
}

proc_easy::easy_parse! {
    struct PluginDependency {
        name: syn::LitStr,
        kind: proc_easy::EasyMaybe<DepKind>,
    }
}

proc_easy::easy_argument! {
    struct PluginDependencies {
        token: dependencies,
        colon: syn::Token![:],
        array: proc_easy::EasyBracketed<proc_easy::EasyTerminated<PluginDependency>>,
    }
}

proc_easy::easy_terminated! {
    @(syn::Token![,])
    pub struct PluginItem {
        dependencies: Option<PluginDependencies>,
    }
}

fn name_to_ident(name: &syn::LitStr) -> syn::Ident {
    let ident = name.value().replace('-', "_");
    syn::Ident::new(&ident, name.span())
}

pub fn plugin(item: PluginItem) -> syn::Result<TokenStream> {
    let dependencies = item.dependencies.map_or(vec![], |d| {
        d.array
            .0
            .iter()
            .map(|d| {
                let ident = name_to_ident(&d.name);
                let d = match &d.kind {
                    proc_easy::EasyMaybe::Nothing => {
                        quote::quote! { #ident::arcana_plugin::dependency() }
                    }
                    proc_easy::EasyMaybe::Just(DepKind::Path(_) | DepKind::Path2(_)) => {
                        quote::quote! { #ident::arcana_plugin::path_dependency() }
                    }
                    proc_easy::EasyMaybe::Just(DepKind::Git(git)) => {
                        let url = &git.url;
                        quote::quote! {
                            arcana::project::Dependency::Git {
                                git: String::from(#url),
                                branch: None,
                            }
                        }
                    }
                    proc_easy::EasyMaybe::Just(DepKind::Git2(git)) => {
                        let url = &git.url.url;
                        let branch = &git.branch;
                        let rev = &git.rev;

                        match (branch, rev) {
                            (None, None) => {
                                quote::quote! {
                                    arcana::project::Dependency::Git {
                                        git: String::from(#url),
                                        branch: None,
                                    }
                                }
                            }
                            (Some(branch), None) => {
                                let branch = &branch.name;
                                quote::quote! {
                                    arcana::project::Dependency::Git {
                                        git: String::from(#url),
                                        branch: Some(#branch),
                                    }
                                }
                            }
                            _ => todo!(),
                        }
                    }
                };

                quote::quote! {
                    plugin.add_dependency(::arcana::plugin::for_macro::ident!(#ident), #d);
                }
            })
            .collect()
    });

    let registry_name = plugin_registry_name();

    let tokens = quote::quote! {
        #[doc(hidden)]
        pub mod arcana_plugin {
            pub fn dependency() -> ::arcana::plugin::for_macro::Dependency {
                ::arcana::plugin::for_macro::Dependency::Crates(env!("CARGO_PKG_VERSION").to_owned())
            }

            pub fn path_dependency() -> ::arcana::plugin::for_macro::Dependency {
                ::arcana::plugin::for_macro::Dependency::from_path(env!("CARGO_MANIFEST_DIR")).unwrap()
            }

            pub fn get() -> ::arcana::plugin::ArcanaPlugin {
                // // Safety: This value is accessed mutably at cdylib load time.
                // // Afterwards it can only be accessed immutably here.
                // unsafe { #registry_name.plugin() }

                let mut plugin = ::arcana::plugin::ArcanaPlugin::new(Some(::arcana::plugin::for_macro::PathBuf::from(env!("CARGO_MANIFEST_DIR"))));
                for add in #registry_name {
                    add(&mut plugin);
                }

                #(#dependencies)*

                plugin
            }

            // pub static mut #registry_name: ::arcana::plugin::for_macro::Registry =
            //     ::arcana::plugin::for_macro::Registry::new();

            #[::arcana::plugin::for_macro::distributed_slice]
            #[linkme(crate = ::arcana::plugin::for_macro::linkme)]
            pub(crate) static #registry_name: [fn(&mut ::arcana::plugin::ArcanaPlugin)];
        }
    };

    Ok(tokens)
}
