use everscale_types::abi::{
    AbiHeaderType, AbiType, AbiValue, AbiVersion, Contract, FromAbi, Function, FunctionBuilder,
    IntoAbi, NamedAbiType, NamedAbiValue, PlainAbiType, PlainAbiValue, WithAbiType,
};
use everscale_types::models::IntAddr;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use std::num::NonZeroU8;
use syn::{parse_macro_input, DeriveInput, Expr, Result};

// pub struct Test {
//     pub x: u32,
//     pub y: String,
// }
//
// impl IntoAbi for Test {
//     fn as_abi(&self) -> AbiValue {
//         let mut v = Vec::new();
//         let x = NamedAbiValue {
//             name: std::sync::Arc::new(*"x"),
//             value: AbiValue::Uint(32, self.x),
//         };
//
//         let y = NamedAbiValue {
//             name: std::sync::Arc::new(*"y"),
//             value: AbiValue::String(self.y),
//         };
//
//         v.push(x);
//         v.push(y);
//
//         AbiValue::Tuple(v)
//     }
//
//     /// Converts into a corresponding ABI value.
//     fn into_abi(self) -> AbiValue
//     where
//         Self: Sized,
//     {
//         let mut v = Vec::new();
//         let x = NamedAbiValue {
//             name: std::sync::Arc::new(*"x"),
//             value: AbiValue::Uint(32, self.x),
//         };
//
//         let y = NamedAbiValue {
//             name: std::sync::Arc::new(*"y"),
//             value: AbiValue::String(self.y),
//         };
//
//         v.push(x);
//         v.push(y);
//
//         AbiValue::Tuple(v)
//     }
// }

pub fn implement_with_abi_type(
    struct_name: &str,
    properites: &[NamedAbiType],
) -> proc_macro2::TokenStream {
    let name_ident = format_ident!("{}", struct_name);
    //let props = *properites.clone();
    //let x = AbiType::Tuple(std::sync::Arc::new(*properites.clone()));

    let props_quote: Vec<_> = properites
        .iter()
        .map(|x| {
            let name = format_ident!("{}", x.name.as_ref());
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
                 AbiType::Tuple(std::sync::Arc::new( properties))
            }
        }
    }
}

pub fn implement_from_abi(struct_name: &str, properites: &[String]) -> proc_macro2::TokenStream {
    //let mut props_quote = Vec::new();
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

    for (name, ty) in properties {
        let name = name.clone();
        let quote_abi_value = quote_abi_value(&name, ty);
        let quote = quote! {
            NamedAbiValue::new(std::sync::Arc::new(*#name), #quote_abi_value);
        };
        props.push(quote);
    }

    let len = props.len();

    let quote = quote! {
        impl IntoAbi for #struct_name {
            fn as_abi(&self) -> AbiValue {
                let mut props: Vec<NamedAbiValue> = Vec::new();
                let struct_props = [ AbiValue, #len] = [#(#props),*];
                AbiValue::Tuple(props.to_vec())
            }

            fn into_abi(self) -> AbiValue
            where
                Self: Sized,
            {
                let mut props: Vec<NamedAbiValue> = Vec::new();
                let struct_props = [ AbiValue, #len] = [#(#props),*];
                AbiValue::Tuple(props.to_vec())
            }
        }
    };

    quote
}

fn quote_abi_value(name: &str, value: &AbiType) -> proc_macro2::TokenStream {
    let name_ident = format_ident!("{}", name);
    match value {
        AbiType::String => quote! {
            everscale_types::abi::AbiValue::String(self.#name_ident)
        },
        AbiType::Address => quote! {
            everscale_types::abi::AbiValue::Address(std::sync::Box::new(self.#name_ident))
        },
        AbiType::Bool => quote! {
             everscale_types::abi::AbiValue::Bool(self.#name_ident)
        },
        AbiType::Bytes => quote! {
              everscale_types::abi::AbiValue::Bytes(self.#name_ident)
        },
        AbiType::FixedBytes(size) => quote! {
              everscale_types::abi::AbiValue::FixedBytes(#size, self.#name_ident)
        },
        AbiType::Cell => quote! {
            everscale_types::abi::AbiValue::Cell(self.#name_ident)
        },
        AbiType::Token => quote! {
            everscale_types::abi::AbiValue::Token(self.#name_ident)
        },
        AbiType::Uint(size) => quote! {
            everscale_types::abi::AbiValue::Uint(*#size, self.#name_ident)
        },
        AbiType::Int(size) => quote! {
            everscale_types::abi::AbiValue::Int(*#size, self.#name_ident)
        },
        AbiType::VarInt(value) => {
            let val = value.get();
            quote!( everscale_types::abi::AbiValue::VarInt(core::num::nonzero::NonZeroU8(#val), self.#name_ident))
        }
        AbiType::VarUint(value) => {
            let val = value.get();
            quote!( everscale_types::abi::AbiValue::VarUint(core::num::nonzero::NonZeroU8(#val), self.#name_ident))
        }
        AbiType::Tuple(properties) => {
            quote!(self.#name_ident.into_abi())
        }

        AbiType::Array(ty) => {
            let ty = quote_abi_type(ty);
            quote!(everscale_types::abi:AbiValue::Array(ty.cl))
        }
        AbiType::FixedArray(ty, size) => {
            let ty = quote_abi_type(ty);
            quote!(everscale_types::abi:AbiValue::FixedArray(ty.cl))
        }
        AbiType::Map(key, value) => {
            let key_type: syn::Type = match key {
                PlainAbiType::Address => {
                    syn::parse_quote!(everscale_types::abi::ty:AbiType::PlainAbiType::Address)
                }
                PlainAbiType::Bool => {
                    syn::parse_quote!(everscale_types::abi::ty:AbiType::PlainAbiType::Bool)
                }
                PlainAbiType::Uint(val) => {
                    syn::parse_quote!(everscale_types::abi::ty:AbiType::PlainAbiType::Uint(#val))
                }
                PlainAbiType::Int(val) => {
                    syn::parse_quote!(everscale_types::abi::ty:AbiType::PlainAbiType::Int(#val))
                }
            };

            let value_type = quote_abi_type(&value);
            let arc_value_type: syn::Type = syn::parse_quote!(std::sync::Arc<#value_type>);

            quote! {
                let mut map = BTreeMap::<everscale_types::abi::PlainAbiValue, everscale_types::abi::AbiValue>::new();
                for (key, value) in self.#name_ident.into_iter() {
                    map.insert(key, value);
                }
                everscale_types::abi::AbiValue::Map(#key_type, #arc_value_type, map );
            }
        }
        AbiType::Optional(value) => {
            let ty = quote_abi_type(&value);
            let val = quote! {
                self.#name_ident.map(|x| {
                    Box::new(x.into_abi())
                })
            };

            quote! {
                everscale_types::abi::AbiValue::Optional(std::sync::Arc<#ty>, #val)
            }
        }
        AbiType::Ref(_) => {
            quote! {
                everscale_types::abi::AbiValue::Ref(std::sync::Box::new(self.#name_ident))
            }
        }
    }
}

fn quote_abi_type(ty: &AbiType) -> syn::Type {
    let quote: syn::Type = match ty.clone() {
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
                let name_abi_quote = make_abi_type(i.name.as_ref(), i.ty.clone());
                tuple_properties.push(name_abi_quote);
            }
            syn::parse_quote!(everscale_types::abi::ty:AbiType::Tuple(std::sync::Arc<[ #(#tuple_properties),*]>))
        }
        AbiType::Array(ty) => {
            let ty = quote_abi_type(&ty);
            syn::parse_quote!(everscale_types::abi::ty:AbiType::Array(std::sync::Arc<#ty>))
        }
        AbiType::FixedArray(ty, size) => {
            let ty = quote_abi_type(&ty);
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

            let value_type = quote_abi_type(&value);
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
    let name = format_ident!("{}", name);
    let abi_type = quote_abi_type(&abi_type);

    quote! {
        NamedAbiType::new(#name, #abi_type)
    }
}
