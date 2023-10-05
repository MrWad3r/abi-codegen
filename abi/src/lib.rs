extern crate proc_macro;

use std::fmt::format;
use std::fs;
use std::fs::{read_to_string, File};
use std::path::PathBuf;

use everscale_types::abi::{AbiType, Contract, Function, FunctionBuilder};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use serde_json::Value;
use syn::{parse_macro_input, DeriveInput, Expr, Result};

#[proc_macro]
pub fn abi(input: TokenStream) -> TokenStream {
    let mut generated_structs: Vec<TokenStream> = Vec::new();
    let mut generated_functions: Vec<TokenStream> = Vec::new();

    let mut helpers: Vec<TokenStream> = Vec::new();
    let once_helper = quote! {
        #[macro_export]
        macro_rules! once {
            ($ty:path, || $expr:expr) => {{
                static ONCE: once_cell::race::OnceBox<$ty> = once_cell::race::OnceBox::new();
                ONCE.get_or_init(|| Box::new($expr))
            }};
        }
    };
    helpers.push(once_helper);

    let current_dir = std::env::current_dir().unwrap();
    let file_path = PathBuf::from(input.to_string().replace("\"", ""));
    let path = current_dir.join(file_path);

    let content = fs::read_to_string(path).unwrap();
    let contract = serde_json::from_str::<Contract>(&content).unwrap();

    println!("{}", contract.abi_version.major);

    let functions: Vec<_> = contract.functions.iter().for_each(|(name, function)| {
        let name = name.to_string();
        let name = format_ident!("{}_function", name);
        let function_token = process_function(name, function, &mut generated_structs);
        generated_functions.push(function_token);
    });

    let structs: Vec<_> = contract
        .events
        .iter()
        .map(|(name, event)| {
            let name = e.0.to_string();
            let name = format_ident!("{}EventInput", name);
            let field = event
                .inputs
                .iter()
                .map(|f| {
                    let ty = abi_type_to_eversale_type(f.ty);
                    let name = format_ident!("{}", f.name.to_string());
                    quote! {
                        pub #ty: #name
                    }
                })
                .collect();
            println!("{}", &name);
        })
        .collect();

    let quote = quote! {
        #(#generated_structs)*
        #(#generated_functions)*
    };

    quote.into();
}

fn process_function(
    name: String,
    function: &Function,
    &mut structs: Vec<TokenStream>,
) -> TokenStream {
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
    structs.push(func);
}

fn make_function_input_struct(function: &Function) -> (String, TokenStream) {
    let struct_name = format_ident!("{}FunctionInput", function.name);
    let properties: Vec<TokenStream> = Vec::new();
    for i in function.inputs.iter() {
        let type_name = make_struct_property(i.name.as_ref(), &i.ty);
    }
    quote! {
        pub struct #struct_name {
            #(#properties)*
        }
    }
}

fn make_struct_property(initial_name: &str, param: &AbiType) -> String {
    let mut name = None;

    let type_name = match param {
        AbiType::Uint(a) => match a {
            8 => "u8",
            16 => "u16",
            32 => "u32",
            64 => "u64",
            128 => "u128",
            160 => "[u8; 20]",
            256 => "ton_types::UInt256",
            _ => "num_bigint::BigUint",
        }
        .to_string(),
        AbiType::Int(a) => match a {
            8 => "i8",
            16 => "i16",
            32 => "i32",
            64 => "i64",
            128 => "i128",
            _ => "num_bigint::BigInt",
        }
        .to_string(),
        AbiType::VarUint(_) => "num_bigint::BigUint".to_string(),
        AbiType::VarInt(_) => "num_bigint::BigUint".to_string(),
        AbiType::Bool => "bool".to_string(),
        AbiType::Tuple(a) => {
            let mut structs: Vec<StructProperty> = Vec::new();
            for i in a {
                let name = i.name.clone();
                let property = generate_property(Some(name), &i.kind)?;
                structs.push(property);
            }
            return Ok(StructProperty::Tuple {
                abi_name: abi_name.unwrap_or_default(),
                internal_types: structs,
                params: a.iter().map(|x| x.kind.clone()).collect(),
            });
        }
        AbiType::Array(a) | AbiType::FixedArray(a, _) => {
            let internal_struct = generate_property(None, a.as_ref())?;
            return Ok(StructProperty::Array {
                abi_name: abi_name.unwrap_or_default(),
                internal_type: a.clone(),
                internal_struct_property: Box::new(internal_struct),
            });
        }
        AbiType::Cell => "everscale_types::cell::Cell".to_string(),
        AbiType::Map(ref a, ref b) => {
            let key = match a.as_ref() {
                &AbiType::Uint(_) | &AbiType::Int(_) | &AbiType::Address => {
                    generate_property(None, a.as_ref())?
                }
                _ => anyhow::bail!("Map key is not allowed type"),
            };

            let value = generate_property(None, b.as_ref())?;

            return Ok(StructProperty::HashMap {
                abi_name: abi_name.unwrap_or_default(),
                key: Box::new(key),
                value: Box::new(value),
                key_type: Box::new(a.as_ref().clone()),
                value_type: Box::new(b.as_ref().clone()),
            });
        }
        AbiType::Address => "ton_block::MsgAddressInt".to_string(),
        AbiType::Bytes => "Vec<u8>".to_string(),
        AbiType::FixedBytes(_) => "Vec<u8>".to_string(),
        AbiType::String => "String".to_string(),
        AbiType::Token => "everscale::Grams".to_string(),
        AbiType::Optional(a) => {
            let internal_struct = generate_property(None, a.as_ref())?;
            return Ok(StructProperty::Option {
                abi_name: abi_name.unwrap_or_default(),
                internal_type: a.clone(),
                internal_struct_property: Box::new(internal_struct),
            });
        }
        AbiType::Ref(a) => return abi_type_to_eversale_type(a.as_ref()),
    };
}
