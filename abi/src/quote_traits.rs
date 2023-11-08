use crate::StructProperty;
use case::CaseExt;
use everscale_types::abi::{
    AbiHeaderType, AbiType, AbiValue, AbiVersion, Contract, FromAbi, Function, FunctionBuilder,
    IgnoreName, IntoAbi, IntoPlainAbi, NamedAbiType, NamedAbiValue, PlainAbiType, PlainAbiValue,
    WithAbiType, WithoutName,
};
use everscale_types::models::IntAddr;
use num_bigint::{BigInt, BigUint};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::num::NonZeroU8;
use syn::{parse_macro_input, DeriveInput, Expr, Result};

pub struct TraitImplGen {}

impl TraitImplGen {
    pub fn new() -> Self {
        Self {}
    }

    pub fn implement_traits(
        &self,
        name: &str,
        properties: &[NamedAbiType],
        unique_tokens: &std::collections::HashMap<AbiType, StructProperty>,
    ) -> proc_macro2::TokenStream {
        //let with_abi_type_impls = self.implement_with_abi_type(name, properties);
        //let into_abi_impls = self.implement_into_abi(name, properties);
        let from_abi_impls = self.implement_from_abi(name, properties, unique_tokens);

        quote! {
            //WithAbiType implementations
            //#with_abi_type_impls

            //IntoAbi implementations
            //#into_abi_impls

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
        unique_tokens: &std::collections::HashMap<AbiType, StructProperty>,
    ) -> proc_macro2::TokenStream {
        let struct_name_ident = format_ident!("{}", struct_name);
        let props: Vec<proc_macro2::TokenStream> = properites.iter().map(|x| {
            let ident = format_ident!("{}", x.name.to_snake());
            unique_tokens.get(&x.ty).map(|struct_property| {
                let type_name = struct_property.type_name_quote();

                quote! {
                    #ident: {
                        let value = iterator.next().ok_or(everscale_types::abi::error::AbiError::TypeMismatch { expected: std::boxed::Box::<str>::from("Some"), ty: "None"})?;
                        <#type_name>::from_abi(value)?
                    },
                }
            })

        }).filter_map(|x|x).collect();

        let props_vec = quote! {
            #(#props)*
        };

        let props = props.clone();

        quote! {
            impl FromAbi for #struct_name_ident {
                fn from_abi(value: AbiValue) -> Result<Self> {
                    match value {
                        AbiValue::Tuple(properties) =>  {
                            let iterator = properties.iter();
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
            let quote_abi_value = quote_abi_value(&name, &prop.ty);
            let quote = quote! {
                NamedAbiValue {
                    name: {
                        let name = #quote_name;
                        let arc: std::sync::Arc<str> = std::sync::Arc::from(name);
                        arc
                    },
                    value: #quote_abi_value,
                }
            };
            props.push(quote);
        }

        let len = props.len();

        let quote = quote! {
            impl IntoAbi for #struct_name_ident {
                fn as_abi(&self) -> AbiValue {
                    let struct_props: [NamedAbiValue; #len] = [#(#props),*];
                    AbiValue::Tuple(struct_props.to_vec())
                }

                fn into_abi(self) -> AbiValue
                where
                    Self: Sized,
                {
                    let struct_props: [ NamedAbiValue; #len] = [#(#props),*];
                    AbiValue::Tuple(struct_props.to_vec())
                }
            }
        };

        quote
    }
}

fn quote_abi_value(name: &str, value: &AbiType) -> proc_macro2::TokenStream {
    let name_ident = format_ident!("{}", name.to_snake());
    match value {
        AbiType::String => quote! {
            everscale_types::abi::AbiValue::String(self.#name_ident)
        },
        AbiType::Address => quote! {
            everscale_types::abi::AbiValue::Address(std::boxed::Box::new(self.#name_ident))
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
            everscale_types::abi::AbiValue::Uint(#size, BigUint::from(self.#name_ident))
        },
        AbiType::Int(size) => quote! {
            everscale_types::abi::AbiValue::Int(#size, BigInt::from(self.#name_ident))
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
            //let ty = quote_abi_type(ty);
            //quote!(everscale_types::abi::AbiValue::Array(std::sync::Arc::new(#ty), self.#name_ident.into_abi()))
            quote!(self.#name_ident.into_abi())
        }
        AbiType::FixedArray(ty, size) => {
            let ty = quote_abi_type(ty);
            quote!(everscale_types::abi::AbiValue::FixedArray(std::sync::Arc::new(#ty), self.#name_ident.into_abi()))
        }
        AbiType::Map(key, value) => {
            let key_type: proc_macro2::TokenStream = match key {
                PlainAbiType::Address => {
                    quote! {
                        everscale_types::abi::PlainAbiType::Address
                    }
                }
                PlainAbiType::Bool => {
                    quote! {
                        everscale_types::abi::PlainAbiType::Bool
                    }
                }
                PlainAbiType::Uint(val) => {
                    quote! {
                        everscale_types::abi::PlainAbiType::Uint(#val)
                    }
                }
                PlainAbiType::Int(val) => {
                    quote! {
                        everscale_types::abi::PlainAbiType::Int(#val)
                    }
                }
            };

            let value_type = quote_abi_type(&value);
            let arc_value_type = quote! {
                std::sync::Arc::new(#value_type)
            };
            //3.into_plain_abi()

            quote! {
                {
                    let mut map = std::collections::BTreeMap::<everscale_types::abi::PlainAbiValue, everscale_types::abi::AbiValue>::new();
                    for (key, value) in self.#name_ident.into_iter() {
                        map.insert(key.clone().into_plain_abi(), value.into_abi());
                    }
                    everscale_types::abi::AbiValue::Map(#key_type, #arc_value_type, map )
                }

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
                everscale_types::abi::AbiValue::Optional(std::sync::Arc::new(#ty), #val)
            }
        }
        AbiType::Ref(value) => {
            quote! {
                everscale_types::abi::AbiValue::Ref(std::boxed::Box::new(self.#name_ident.into_abi()))
            }
        }
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
            let len = tuple.len();

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
                        syn::parse_quote!(everscale_types::abi::AbiType::PlainAbiType::Address);
                    quote! {
                        #ty
                    }
                }
                PlainAbiType::Bool => {
                    let ty: syn::Type =
                        syn::parse_quote!(everscale_types::abi::AbiType::PlainAbiType::Bool);
                    quote! {
                        #ty
                    }
                }
                PlainAbiType::Uint(value) => {
                    quote! {
                        everscale_types::abi::AbiType::PlainAbiType::Uint(#value)
                    }
                }
                PlainAbiType::Int(value) => {
                    quote! {
                        everscale_types::abi::AbiType::PlainAbiType::Int(#value)
                    }
                }
            };

            let value_type = quote_abi_type(&value);
            syn::parse_quote!(everscale_types::abi::AbiType::PlainAbiType::Map(#key_type, #value_type))
        }
        AbiType::Optional(value) => {
            let ty = quote_abi_type(ty);
            quote! {
                everscale_types::abi::AbiType::Optional(std::sync::Arc<#ty>)
            }
        }
        AbiType::Ref(value) => {
            let ty = quote_abi_type(ty);
            quote! {
                everscale_types::abi::AbiType::Ref(std::sync::Arc<#ty>)
            }
        }
    };
    quote
}

fn make_abi_type(name: &str, abi_type: AbiType) -> proc_macro2::TokenStream {
    let abi_type = quote_abi_type(&abi_type);

    quote! {
        NamedAbiType::new(#name, #abi_type)
    }
}
