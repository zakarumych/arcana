use proc_macro2::TokenStream;

use crate::plugin_registry_name;

pub fn init(attr: proc_macro::TokenStream, item: syn::ItemFn) -> syn::Result<TokenStream> {
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
            plugin.add_init(#ident);
        });

        #item
    })
}
