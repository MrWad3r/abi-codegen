use everscale_types::abi::{
    AbiHeaderType, AbiType, AbiValue, AbiVersion, Contract, FromAbi, Function, FunctionBuilder,
    IntoAbi, NamedAbiType, NamedAbiValue, PlainAbiType, WithAbiType,
};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::num::NonZeroU8;
use syn::{parse_macro_input, DeriveInput, Expr, Result};

pub fn implement_with_abi_type(
    struct_name: &str,
    properites: &[NamedAbiType],
) -> proc_macro2::TokenStream {
    let name_ident = format_ident!("{}", struct_name);
    let x = AbiType::Tuple(std::sync::Arc::new(properites));

    let props_quote: Vec<_> = properites
        .iter()
        .map(|x| {
            let name = format_ident!("{}", x.name.as_ref());
            let quote_abi_type = quote_abi_type(x.ty);

            quote! {
                NamedAbiType::new(#name, #quote_abi_type)
            }
        })
        .collect();

    quote! {
        impl WithAbiType for #name_ident {
            fn abi_type() -> AbiType {
                 AbiType::Tuple(std::sync::Arc::new(  &[ #(#props_quote),* ]))
            }
        }
    }
}

pub fn implement_from_abi(struct_name: &str, properites: &[String]) -> proc_macro2::TokenStream {
    let mut props_quote = Vec::new();
    let props: Vec<proc_macro2::TokenStream> = properites.iter().map(|x| {
        let ident = format_ident!("{}", x);
        quote! {
            pub #ident: iterator.next().ok_or(everscale_types::abi::error::AbiError::TypeMismatch {expected: Box::from(#x), ty: "None"})?
        }
    }).collect();

    quote! {
        impl FromAbi for #struct_name {
            fn from_abi(value: AbiValue) -> Result<Self> {
                match value {
                    AbiValue::Tuple(properties) =>  {
                        let iterator = properties.iter();
                        Ok(
                            #struct_name {
                                #(#props),*
                            }
                        )

                    },
                    _ => Err(anyhow::Error::from(
                        everscale_types::abi::error::AbiError::TypeMismatch {
                            expected: Box::from("tuple"),
                            ty: value.display_type().to_string().into(),
                        },
                    )),
                }
            }
        }
    }
}

pub fn implement_into_abi(
    struct_name: &str,
    properties: &[(String, AbiType)],
) -> proc_macro2::TokenStream {
    let mut props: Vec<proc_macro2::TokenStream> = Vec::new();
    NamedAbiValue {
        name: Arc::new(()),
        value: AbiValue::Tuple(),
    }

    for (name, ty) in properties {
        let name_ident = format_ident!("{}", name);
        quote! {
            NamedAbiValue::new(self.#name_ident
        }
    }

    let quote = quote! {
        impl IntoAbi for Test {
            fn as_abi(&self) -> AbiValue {
                let mut props: Vec<NamedAbiValue> = Vec::new();




                //AbiValue::Tuple()
            }

            fn into_abi(self) -> AbiValue
            where
                Self: Sized,
            {
            }
        }
    };

    quote
}

pub struct Test {
    x: u32,
    y: String,
}

impl IntoAbi for Test {
    fn as_abi(&self) -> AbiValue {
        let mut props: Vec<NamedAbiValue> = Vec::new();

        //AbiValue::Tuple()
    }

    fn into_abi(self) -> AbiValue
    where
        Self: Sized,
    {
    }
}

fn quote_abi_type(ty: AbiType) -> syn::Type {
    let quote: syn::Type = match ty {
        AbiType::String => syn::parse_quote!(everscale_types::abi::ty:AbiType::String),
        AbiType::Address => syn::parse_quote!(everscale_types::abi::ty:AbiType::Address),
        AbiType::Bool => syn::parse_quote!(everscale_types::abi::ty:AbiType::Bool),
        AbiType::Bytes => syn::parse_quote!(everscale_types::abi::ty:AbiType::Bytes),
        AbiType::FixedBytes(size) => {
            syn::parse_quote!(everscale_types::abi::ty:AbiType::FixedBytes(#size))
        }
        AbiType::Cell => syn::parse_quote!(everscale_types::abi::ty:AbiType::Cell),
        AbiType::Token => syn::parse_quote!(everscale_types::abi::ty:AbiType::Token),
        AbiType::Int(value) => syn::parse_quote!(everscale_types::abi::ty:AbiType::Int(#value)),
        AbiType::Uint(value) => syn::parse_quote!(everscale_types::abi::ty:AbiType::Uint(#value)),
        AbiType::VarInt(value) => {
            let val = value.get();
            syn::parse_quote!(everscale_types::abi::ty:AbiType::Int(core::num::nonzero::NonZeroU8(#val)))
        }
        AbiType::VarUint(value) => {
            let val = value.get();
            syn::parse_quote!(everscale_types::abi::ty:AbiType::Uint(core::num::nonzero::NonZeroU8(#val)))
        }
        AbiType::Tuple(tuple) => {
            let mut tuple_properties = Vec::new();
            for i in tuple.iter() {
                let name_abi_quote = make_abi_type(i.name.as_ref(), i.ty);
                tuple_properties.push(name_abi_quote);
            }
            syn::parse_quote!(everscale_types::abi::ty:AbiType::Tuple(std::sync::Arc<[ #(#tuple_properties),*]>))
        }
        AbiType::Array(ty) => {
            let ty = quote_abi_type(ty);
            syn::parse_quote!(everscale_types::abi::ty:AbiType::Array(std::sync::Arc<#ty>))
        }
        AbiType::FixedArray(ty, size) => {
            let ty = quote_abi_type(ty);
            syn::parse_quote!(everscale_types::abi::ty:AbiType::FixedArray(std::sync::Arc<#ty>, #size))
        }
        AbiType::Map(key, value) => {
            let key_type: syn::Type = match key {
                PlainAbiType::Address => {
                    syn::parse_quote!(everscale_types::abi::ty:AbiType::PlainAbiType::Address)
                }
                PlainAbiType::Bool => {
                    syn::parse_quote!(everscale_types::abi::ty:AbiType::PlainAbiType::Bool)
                }
                PlainAbiType::Uint(value) => {
                    syn::parse_quote!(everscale_types::abi::ty:AbiType::PlainAbiType::Uint(#value))
                }
                PlainAbiType::Int(value) => {
                    syn::parse_quote!(everscale_types::abi::ty:AbiType::PlainAbiType::Int(#value))
                }
            };

            let value_type = quote_abi_type(value);
            syn::parse_quote!(everscale_types::abi::ty:AbiType::PlainAbiType::Map(#key_type, #value_type))
        }
        AbiType::Optional(value) => {
            let ty = quote_abi_type(ty);
            syn::parse_quote!(everscale_types::abi::ty:AbiType::Optional(std::sync::Arc<#ty>))
        }
        AbiType::Ref(value) => {
            let ty = quote_abi_type(ty);
            syn::parse_quote!(everscale_types::abi::ty:AbiType::Ref(std::sync::Arc<#ty>))
        }
    };
    quote
}

fn make_abi_type(name: &str, abi_type: AbiType) -> proc_macro2::TokenStream {
    let name = format_ident!("{}", name.as_ref());
    let abi_type = quote_abi_type(abi_type);

    quote! {
        NamedAbiType::new(#name, #abi_type)
    }
}
