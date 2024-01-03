use proc_macro2::TokenStream;

proc_easy::easy_parse! {
    pub enum StidValue {
        Int(syn::LitInt),
        Str(syn::LitStr),
    }
}

pub fn with_stid(stid: StidValue, input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;

    let num = match &stid {
        StidValue::Int(int) => int.base10_parse::<u64>()?,
        StidValue::Str(str) => {
            let s = str
                .value()
                .trim()
                .chars()
                .filter(|c| "0123456789abcdef".contains(*c))
                .collect::<String>();
            u64::from_str_radix(&s, 16).map_err(|err| {
                syn::Error::new(str.span(), format!("Failed to parse STID: {}", err))
            })?
        }
    };

    if num == 0 {
        return Err(syn::Error::new(
            match &stid {
                StidValue::Int(int) => int.span(),
                StidValue::Str(str) => str.span(),
            },
            "STID must not be 0",
        ));
    }

    let output = quote::quote! {
        #input

        impl ::arcana::stid::WithStid for #name {
            #[inline(always)]
            fn stid() -> Stid {
                Stid::new(unsafe { ::core::num::NonZeroU64::new_unchecked(#num) })
            }

            #[inline(always)]
            fn stid_dyn(&self) -> Stid {
                Self::stid()
            }
        }
    };

    Ok(output)
}
