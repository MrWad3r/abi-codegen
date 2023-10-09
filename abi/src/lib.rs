extern crate proc_macro;

use std::fmt::format;
use std::fs;
use std::fs::{read_to_string, File};
use std::path::PathBuf;

use crate::StructProperty::Simple;
use everscale_types::abi::{AbiType, Contract, Function, FunctionBuilder, PlainAbiType};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use serde_json::Value;
use std::any::type_name;
use std::os::unix::fs::symlink;
use std::process::id;
use syn::{parse_macro_input, DeriveInput, Expr, Result};

#[proc_macro]
pub fn abi(input: TokenStream) -> TokenStream {
    let mut generated_structs: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut generated_functions: Vec<proc_macro2::TokenStream> = Vec::new();

    let current_dir = std::env::current_dir().unwrap();
    let file_path = PathBuf::from(input.to_string().replace("\"", ""));
    let path = current_dir.join(file_path);

    let content = fs::read_to_string(path).unwrap();
    let contract = serde_json::from_str::<Contract>(&content).unwrap();

    println!("{}", contract.abi_version.major);

    contract.functions.iter().for_each(|(name, function)| {
        let name = name.to_string();
        let ident_name = format_ident!("{}_function", &name);
        let function_token = process_function(name, function, &mut generated_structs);
        generated_functions.push(function_token);
    });

    let structs: Vec<_> = contract
        .events
        .iter()
        .map(|(name, event)| {
            let name = name.to_string();
            let name = format_ident!("{}EventInput", name);
            let field: Vec<_> = event
                .inputs
                .iter()
                .map(|f| {
                    let property = make_struct_property(Some(f.name.to_string()), &f.ty);
                    let name_ident = format_ident!("{}", f.name.to_string());
                    let ty_ident = property.type_name();
                    quote! {
                        pub #name_ident: #ty_ident,
                    }
                })
                .collect();
        })
        .collect();

    let quote = quote! {
        #(#generated_structs)*
        #(#generated_functions)*
    };

    quote.into()
}

fn process_function(
    name: String,
    function: &Function,
    structs: &mut Vec<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let (input_name, input_tokens) = make_function_input_struct(function);
    let (output_name, output_tokens) = make_function_output_struct(function);
    let func = quote! {
        fn #name() -> &'static everscale_types::abi::Function {
            static ONCE: std::sync::OnceLock<everscale_types::abi::Function> = std::sync::OnceLock::new();
            ONCE.get_or_init(|| {
                FunctionBuilder::new(function.abi_version, function.name)
                .with_headers(function.headers)
                .build();
            });
        }
    };
    structs.push(input_tokens);
    structs.push(output_tokens);
    func
}

fn make_function_input_struct(function: &Function) -> (String, proc_macro2::TokenStream) {
    let struct_name = format_ident!("{}FunctionInput", function.name.as_ref());
    let mut properties: Vec<proc_macro2::TokenStream> = Vec::new();
    for i in function.inputs.iter() {
        let type_name = make_struct_property(Some(i.name.to_string()), &i.ty);
        let property_name_ident = format_ident!("{}", i.name.as_ref());
        let ty_ident = type_name.type_name();
        let quote = quote! {
             pub #property_name_ident: #ty_ident,
        };
        properties.push(quote);
    }
    let func = quote! {
        pub struct #struct_name {
            #(#properties)*
        }
    };

    (struct_name.to_string(), func.into())
}

fn make_function_output_struct(function: &Function) -> (String, proc_macro2::TokenStream) {
    let struct_name = format_ident!("{}FunctionOutput", function.name.as_ref());
    let mut properties: Vec<proc_macro2::TokenStream> = Vec::new();
    for i in function.outputs.iter() {
        let type_name = make_struct_property(Some(i.name.to_string()), &i.ty);
        let property_name_ident = format_ident!("{}", i.name.as_ref());
        let ty_ident = type_name.type_name();
        let quote = quote! {
            pub #property_name_ident: #ty_ident,
        };
        properties.push(quote.into());
    }
    let func = quote! {
        pub struct #struct_name {
            #(#properties)*
        }
    };

    (struct_name.to_string(), func.into())
}

fn make_struct_property(initial_name: Option<String>, param: &AbiType) -> StructProperty {
    let mut name = initial_name.map(|x| x.to_string());

    match param {
        AbiType::Uint(a) => {
            let ty = match a {
                8 => "u8",
                16 => "u16",
                32 => "u32",
                64 => "u64",
                128 => "u128",
                160 => "[u8; 20]",
                256 => "everscale_types::prelude::HashBytes",
                _ => "num_bigint::BigUint",
            };
            StructProperty::Simple {
                name: name,
                type_name: syn::parse_str(ty).unwrap(),
            }
        }
        AbiType::Int(a) => {
            let ty = match a {
                8 => "i8",
                16 => "i16",
                32 => "i32",
                64 => "i64",
                128 => "i128",
                _ => "num_bigint::BigInt",
            };
            StructProperty::Simple {
                name: name,
                type_name: syn::parse_str(ty).unwrap(),
            }
        }
        AbiType::VarUint(_) | AbiType::VarInt(_) => StructProperty::Simple {
            name: name,
            type_name: syn::parse_quote!(num_bigint::BigUint),
        },
        AbiType::Bool => StructProperty::Simple {
            name: name,
            type_name: syn::parse_quote!(bool),
        },
        AbiType::Tuple(a) => {
            let mut structs: Vec<StructProperty> = Vec::new();
            for i in a.iter() {
                let name = i.name.clone();
                let property = make_struct_property(Some(name.to_string()), &i.ty);
                structs.push(property);
            }
            return StructProperty::Tuple {
                name: name.unwrap_or_default(),
                fields: structs,
            };
        }
        AbiType::Array(a) | AbiType::FixedArray(a, _) => {
            let internal_struct = make_struct_property(None, a.as_ref());
            return StructProperty::Array {
                name: name.unwrap_or_default(),
                internal: Box::new(internal_struct),
            };
        }
        AbiType::Cell => StructProperty::Simple {
            name: name,
            type_name: syn::parse_quote!(everscale_types::prelude::Cell),
        },
        AbiType::Map(a, b) => {
            let key = match a {
                &PlainAbiType::Uint(_) | &PlainAbiType::Int(_) | &PlainAbiType::Address => {
                    make_struct_property(None, &a.clone().into())
                }
                _ => panic!("Map key is not allowed type"),
            };

            let value = make_struct_property(None, b.as_ref());

            return StructProperty::HashMap {
                name: name.unwrap_or_default(),
                key: Box::new(key),
                value: Box::new(value),
            };
        }
        AbiType::Address => StructProperty::Simple {
            name: name,
            type_name: syn::parse_quote!(everscale_types::models::IntAddr),
        },
        AbiType::Bytes | AbiType::FixedBytes(_) => StructProperty::Simple {
            name: name,
            type_name: syn::parse_quote!(Vec<u8>),
        },
        AbiType::String => StructProperty::Simple {
            name: name,
            type_name: syn::parse_quote!(String),
        },
        AbiType::Token => StructProperty::Simple {
            name: name,
            type_name: syn::parse_quote!(everscale_types::num::Tokens),
        },
        AbiType::Optional(a) => {
            let internal_struct = make_struct_property(None, a.as_ref());
            return StructProperty::Option {
                name: name.unwrap_or_default(),
                internal: Box::new(internal_struct),
            };
        }
        AbiType::Ref(a) => {
            let name = name.map(|x| x.clone());
            return make_struct_property(name, a.as_ref());
        }
    }
}

enum StructProperty {
    Simple {
        name: Option<String>,
        type_name: syn::Type,
    },
    Tuple {
        name: String,
        fields: Vec<StructProperty>,
    },
    Array {
        name: String,
        internal: Box<StructProperty>,
    },
    Option {
        name: String,
        internal: Box<StructProperty>,
    },
    HashMap {
        name: String,
        key: Box<StructProperty>,
        value: Box<StructProperty>,
    },
}

impl StructProperty {
    pub fn type_name(&self) -> syn::Type {
        match self {
            StructProperty::Simple { type_name, .. } => type_name.clone(),
            StructProperty::Tuple { name, .. } => syn::parse_str(name).unwrap(),
            StructProperty::Array { internal, .. } => {
                let ty = internal.type_name();
                syn::parse_quote!(Vec<#ty>)
            }
            StructProperty::Option { internal, .. } => {
                let ty = internal.type_name();
                syn::parse_quote!(Option<#ty>)
            }
            StructProperty::HashMap { key, value, .. } => {
                let key = key.type_name();
                let value = value.type_name();
                syn::parse_quote!(std::collections::HashMap<#key, #value>)
            }
        }
    }
}
