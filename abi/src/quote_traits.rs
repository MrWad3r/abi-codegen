use case::CaseExt;
use everscale_types::abi::{AbiType, NamedAbiType, PlainAbiType};

use quote::{format_ident, quote};

pub struct TraitImplGen {}

impl TraitImplGen {
    pub fn new() -> Self {
        Self {}
    }

    pub fn implement_traits(
        &self,
        name: &str,
        properties: &[NamedAbiType],
    ) -> proc_macro2::TokenStream {
        let with_abi_type_impls = self.implement_with_abi_type(name, properties);
        let into_abi_impls = self.implement_into_abi(name, properties);
        let from_abi_impls = self.implement_from_abi(name, properties);

        quote! {
            //WithAbiType implementations
            #with_abi_type_impls

            //IntoAbi implementations
            #into_abi_impls

            //FromAbi implementations
            #from_abi_impls
        }
    }

    pub fn implement_with_abi_type(
        &self,
        struct_name: &str,
        properites: &[NamedAbiType],
    ) -> proc_macro2::TokenStream {
        let name_ident = format_ident!("{}", struct_name);

        let props_quote: Vec<_> = properites
            .iter()
            .map(|x| {
                let name = x.name.as_ref();
                let quote_abi_type = quote_abi_type(&x.ty);

                quote! {
                    NamedAbiType::new(#name, #quote_abi_type)
                }
            })
            .collect();

        let properties = quote! {
            [ #(#props_quote),* ]
        };

        let properties_count = properites.len();
        let tuple_tokens = quote! {
            let properties: [NamedAbiType; #properties_count] = #properties;
        };
        quote! {
            impl WithAbiType for #name_ident {
                fn abi_type() -> AbiType {
                     #tuple_tokens
                     AbiType::Tuple(std::sync::Arc::new(properties))
                }
            }
        }
    }

    pub fn implement_from_abi(
        &self,
        struct_name: &str,
        properites: &[NamedAbiType],
    ) -> proc_macro2::TokenStream {
        let struct_name_ident = format_ident!("{}", struct_name);
        let props: Vec<proc_macro2::TokenStream> = properites
            .iter()
            .map(|x| {
                let ident = format_ident!("{}", x.name.to_snake());
                quote! {
                    #ident: everscale_types::abi::FromAbiIter::<#struct_name_ident>::next_value(&mut iterator)?,
                }
            })
            .collect();

        let props_vec = quote! {
            #(#props)*
        };

        quote! {
            impl FromAbi for #struct_name_ident {
                fn from_abi(value: AbiValue) -> Result<Self> {
                    match value {
                        AbiValue::Tuple(properties) =>  {
                            let mut iterator = properties.into_iter();
                            Ok(
                                #struct_name_ident {
                                    #props_vec
                                }
                            )

                        },
                        _ => Err(anyhow::Error::from(
                            everscale_types::abi::error::AbiError::TypeMismatch {
                                expected: std::boxed::Box::<str>::from("tuple"),
                                ty: value.display_type().to_string().into(),
                            },
                        )),
                    }
                }
            }
        }
    }

    pub fn implement_into_abi(
        &self,
        struct_name: &str,
        properties: &[NamedAbiType],
    ) -> proc_macro2::TokenStream {
        let mut props: Vec<proc_macro2::TokenStream> = Vec::new();
        let struct_name_ident = format_ident!("{}", struct_name);

        for prop in properties {
            let name = prop.name.clone();
            let quote_name = name.as_ref();
            let quote_abi_value = quote_abi_value(&name);
            let quote = quote! {
                NamedAbiValue {
                    name: {
                        let arc: std::sync::Arc<str> = std::sync::Arc::from(#quote_name);
                        arc
                    },
                    value: #quote_abi_value,
                }
            };
            props.push(quote);
        }

        let quote = quote! {
            impl IntoAbi for #struct_name_ident {
                fn as_abi(&self) -> AbiValue {
                    AbiValue::Tuple(vec![#(#props),*])
                }

                fn into_abi(self) -> AbiValue
                where
                    Self: Sized,
                {
                     AbiValue::Tuple(vec![#(#props),*])
                }
            }
        };

        quote
    }
}

fn quote_abi_type(ty: &AbiType) -> proc_macro2::TokenStream {
    let quote: proc_macro2::TokenStream = match ty.clone() {
        AbiType::String => {
            let ty: syn::Type = syn::parse_quote!(everscale_types::abi::AbiType::String);
            quote! {
                #ty
            }
        }
        AbiType::Address => {
            let ty: syn::Type = syn::parse_quote!(everscale_types::abi::AbiType::Address);
            quote! {
                #ty
            }
        }
        AbiType::Bool => syn::parse_quote!(everscale_types::abi::AbiType::Bool),
        AbiType::Bytes => syn::parse_quote!(everscale_types::abi::AbiType::Bytes),
        AbiType::FixedBytes(size) => {
            syn::parse_quote!(everscale_types::abi::AbiType::FixedBytes(#size))
        }
        AbiType::Cell => syn::parse_quote!(everscale_types::abi::AbiType::Cell),
        AbiType::Token => syn::parse_quote!(everscale_types::abi::AbiType::Token),
        AbiType::Int(value) => quote! {
            everscale_types::abi::AbiType::Int(#value)
        },
        AbiType::Uint(value) => {
            quote! {
                everscale_types::abi::AbiType::Uint(#value)
            }
        }
        AbiType::VarInt(value) => {
            let val = value.get();
            quote! {
                everscale_types::abi::AbiType::Int(core::num::nonzero::NonZeroU8(#val))
            }
        }
        AbiType::VarUint(value) => {
            let val = value.get();
            quote! {
                everscale_types::abi:AbiType::Uint(core::num::nonzero::NonZeroU8(#val))
            }
        }
        AbiType::Tuple(tuple) => {
            let mut tuple_properties = Vec::new();

            for i in tuple.iter() {
                let name_abi_quote = make_abi_type(i.name.as_ref(), i.ty.clone());
                tuple_properties.push(name_abi_quote);
            }

            quote! {
                everscale_types::abi::AbiType::Tuple(std::sync::Arc::new([ #(#tuple_properties),*]))
            }
        }
        AbiType::Array(ty) => {
            let ty = quote_abi_type(&ty);
            quote! {
                everscale_types::abi::AbiType::Array(std::sync::Arc::new(#ty))
            }
        }
        AbiType::FixedArray(ty, size) => {
            let ty = quote_abi_type(&ty);
            quote! {
                everscale_types::abi:AbiType::FixedArray(std::sync::Arc<#ty>, #size)
            }
        }
        AbiType::Map(key, value) => {
            let key_type: proc_macro2::TokenStream = match key {
                PlainAbiType::Address => {
                    let ty: syn::Type =
                        syn::parse_quote!(everscale_types::abi::PlainAbiType::Address);
                    quote! {
                        #ty
                    }
                }
                PlainAbiType::Bool => {
                    let ty: syn::Type = syn::parse_quote!(everscale_types::abi::PlainAbiType::Bool);
                    quote! {
                        #ty
                    }
                }
                PlainAbiType::Uint(value) => {
                    quote! {
                        everscale_types::abi::PlainAbiType::Uint(#value)
                    }
                }
                PlainAbiType::Int(value) => {
                    quote! {
                        everscale_types::abi::PlainAbiType::Int(#value)
                    }
                }
            };

            let value_type = quote_abi_type(&value);
            syn::parse_quote!(everscale_types::abi::AbiType::Map(#key_type, std::sync::Arc::new(#value_type)))
        }
        AbiType::Optional(_) => {
            let ty = quote_abi_type(ty);
            quote! {
                everscale_types::abi::AbiType::Optional(std::sync::Arc<#ty>)
            }
        }
        AbiType::Ref(_) => {
            let ty = quote_abi_type(ty);
            quote! {
                everscale_types::abi::AbiType::Ref(std::sync::Arc<#ty>)
            }
        }
    };
    quote
}

fn quote_abi_value(name: &str) -> proc_macro2::TokenStream {
    let name_ident = format_ident!("{}", name.to_snake());
    quote! {
        everscale_types::abi::IntoAbi::as_abi(&self.#name_ident)
    }
}

fn make_abi_type(name: &str, abi_type: AbiType) -> proc_macro2::TokenStream {
    let abi_type = quote_abi_type(&abi_type);

    quote! {
        NamedAbiType::new(#name, #abi_type)
    }
}
