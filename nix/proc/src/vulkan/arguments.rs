use proc_easy::{private::Spanned, EasyAttributes};
use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn;

pub fn derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    match derive_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

proc_easy::easy_flags! {
    Kind(kind) {
        // Constant(constant),
        Uniform(uniform),
        Sampled(sampled),
        Storage(storage),
    }
}

proc_easy::easy_flags! {
    Shader(shader) | Shaders(shaders) {
        Vertex(vertex),
        Fragment(fragment),
    }
}

proc_easy::easy_attributes! {
    @(nix)
    struct FieldAttributes {
        kind: Option<Kind>,
        shaders: Shaders,
    }
}

fn derive_impl(input: syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let vis = &input.vis;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            input.generics,
            "generic arguments are not supported by `#[derive(Arguments)]`",
        ));
    }

    let data = match input.data {
        syn::Data::Struct(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "only structs are supported by `#[derive(Arguments)]`",
            ))
        }
    };

    let field_attrs = data
        .fields
        .iter()
        .map(|field| FieldAttributes::parse(&field.attrs, field.span()))
        .collect::<Result<Vec<_>, _>>()?;

    let field_argument_impls = data
        .fields
        .iter()
        .zip(&field_attrs)
        .map(|(field, attrs)| {
            let ty = &field.ty;
            match attrs.kind {
                None => quote::quote!(<#ty as nix::proc_macro::ArgumentsField<nix::proc_macro::Automatic>>),
                // Some(Kind::Constant(_)) => {
                //     quote::quote!(<#ty as nix::proc_macro::ArgumentsField<nix::proc_macro::Constant>>)
                // }
                Some(Kind::Uniform(_)) => {
                    quote::quote!(<#ty as nix::proc_macro::ArgumentsField<nix::proc_macro::Uniform>>)
                }
                Some(Kind::Sampled(_)) => {
                    quote::quote!(<#ty as nix::proc_macro::ArgumentsField<nix::proc_macro::Sampled>>)
                }
                Some(Kind::Storage(_)) => {
                    quote::quote!(<#ty as nix::proc_macro::ArgumentsField<nix::proc_macro::Storage>>)
                }
            }
        })
        .collect::<Vec<_>>();

    let field_stages = data
        .fields
        .iter()
        .zip(&field_attrs)
        .map(|(field, attrs)| {
            if attrs.shaders.flags.is_empty() {
                quote_spanned!(field.span() => nix::ShaderStage::empty())
            } else {
                let mut tokens = quote!(0);

                for stage in attrs.shaders.flags.iter() {
                    match stage {
                        Shader::Vertex(vertex) => {
                            if !tokens.is_empty() {
                                tokens.extend(quote_spanned!(vertex.span() => | ));
                            }
                            tokens
                                .extend(quote_spanned!(vertex.span() => nix::ShaderStages::VERTEX.bits()))
                        }
                        Shader::Fragment(fragment) => {
                            if !tokens.is_empty() {
                                tokens.extend(quote_spanned!(fragment.span() => | ));
                            }
                            tokens.extend(
                                quote_spanned!(fragment.span() => nix::ShaderStages::FRAGMENT.bits()),
                            )
                        }
                    }
                }

                quote!(nix::ShaderStages::from_bits_truncate(#tokens))
            }
        })
        .collect::<Vec<_>>();

    let field_bindings = (0..data.fields.len() as u32).collect::<Vec<_>>();
    let fields_count = data.fields.len();

    let update_name = quote::format_ident!("NixGenerated{}Update", name);

    match &data.fields {
        syn::Fields::Unit => {
            return Err(syn::Error::new_spanned(
                data.fields,
                "unit structs are not supported by `#[derive(Arguments)]`",
            ));
        }
        syn::Fields::Unnamed(_) => todo!(),
        syn::Fields::Named(fields) => {
            let fields_name = fields
                .named
                .iter()
                .map(|field| field.ident.as_ref().unwrap())
                .collect::<Vec<_>>();
            Ok(quote! {
                #[doc(hidden)]
                #[derive(Clone, Copy)]
                #vis struct #update_name {
                    #(#fields_name: #field_argument_impls::Update,)*
                }

                impl #name {
                    const fn nix_generated_template_entries() -> [nix::proc_macro::DescriptorUpdateTemplateEntry; #fields_count] {
                        let update = nix::proc_macro::MaybeUninit::<#update_name>::uninit();
                        let ptr = update.as_ptr();
                        [
                            #(
                                nix::proc_macro::DescriptorUpdateTemplateEntry {
                                    dst_binding: #field_bindings,
                                    dst_array_element: 0,
                                    descriptor_count: {
                                        if #field_argument_impls::SIZE > u32::MAX as usize {
                                            panic!("Too many elements in the descriptor array");
                                        }
                                        #field_argument_impls::SIZE as u32
                                    },
                                    descriptor_type: nix::proc_macro::descriptor_type(#field_argument_impls::KIND),
                                    offset: unsafe { nix::proc_macro::addr_of!((*ptr).#fields_name).cast::<u8>().offset_from(ptr.cast::<u8>()) as usize },
                                    stride: #field_argument_impls::STRIDE,
                                },
                            )*
                        ]
                    }
                }

                impl nix::proc_macro::Arguments for #name {
                    const LAYOUT: nix::ArgumentGroupLayout<'static> = nix::ArgumentGroupLayout {
                        arguments: &[#(nix::ArgumentLayout {
                            kind: #field_argument_impls::KIND,
                            size: #field_argument_impls::SIZE,
                            stages: #field_stages,
                        },)*],
                    };

                    type Update = #update_name;

                    #[inline(always)]
                    fn template_entries() -> &'static [nix::proc_macro::DescriptorUpdateTemplateEntry] {
                        static ENTRIES: [nix::proc_macro::DescriptorUpdateTemplateEntry; #fields_count] = #name::nix_generated_template_entries();
                        &ENTRIES
                    }

                    #[inline(always)]
                    fn update(&self) -> #update_name {
                        #update_name {
                            #(#fields_name: #field_argument_impls::update(&self.#fields_name),)*
                        }
                    }

                    #[inline(always)]
                    fn add_refs(&self, refs: &mut nix::proc_macro::Refs) {
                        #(#field_argument_impls::add_refs(&self.#fields_name, refs);)*
                    }
                }
            })
        }
    }
}
