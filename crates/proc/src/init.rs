use proc_macro2::TokenStream;

pub fn init(attr: proc_macro::TokenStream, item: syn::ItemFn) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            TokenStream::from(attr),
            "unexpected attribute",
        ));
    }

    let ident = &item.sig.ident;
    Ok(quote::quote! {
        ::arcana::plugin_ctor_add!(plugin => {
            plugin.add_init(#ident);
        });

        #item
    })
}
