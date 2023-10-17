extern crate proc_macro;

use std::fmt::format;
use std::fs;
use std::fs::{read_to_string, File};
use std::path::PathBuf;

use crate::StructProperty::Simple;
use case::CaseExt;
use everscale_types::abi::{
    AbiHeaderType, AbiType, AbiVersion, Contract, Function, FunctionBuilder, PlainAbiType,
};
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

    contract.functions.iter().for_each(|(name, function)| {
        let name = name.to_string();
        let function_token = process_function(name, function, &mut generated_structs);
        generated_functions.push(function_token);
    });

    // let structs: Vec<_> = contract
    //     .events
    //     .iter()
    //     .map(|(name, event)| {
    //         let name = name.to_string();
    //         let name = format_ident!("{}EventInput", name);
    //         let field: Vec<_> = event
    //             .inputs
    //             .iter()
    //             .map(|f| {
    //                 let property = make_struct_property(Some(f.name.to_string()), &f.ty);
    //                 let name_ident = format_ident!("{}", f.name.to_string());
    //                 let ty_ident = property.type_name();
    //                 quote! {
    //                     pub #name_ident: #ty_ident,
    //                 }
    //             })
    //             .collect();
    //     })
    //     .collect();
    let contract_name = format_ident!("{}", "test");
    let header_type: syn::Type = syn::parse_str("everscale_types::abi::AbiHeaderType").unwrap();
    let abi_type: syn::Type = syn::parse_str("everscale_types::abi::AbiVersion").unwrap();

    let mut header_idents = Vec::<proc_macro2::TokenStream>::new();
    for i in contract.headers.iter() {
        let ty = match i {
            AbiHeaderType::Expire => "everscale_types::abi::AbiHeaderType::Expire",
            AbiHeaderType::PublicKey => "everscale_types::abi::AbiHeaderType::PublicKey",
            AbiHeaderType::Time => "everscale_types::abi::AbiHeaderType::Time",
        };
        let ty: syn::Type = syn::parse_str(ty).expect("Failed to parse header type");
        let quote = quote! {
            #ty
        };
        header_idents.push(quote);
    }

    let slice_token = quote! {
        [ #(#header_idents),* ]
    };

    let header_count = contract.headers.len();
    let major = contract.abi_version.major;
    let minor = contract.abi_version.minor;

    let quote = quote! {

        mod #contract_name {
            use nekoton_abi::{UnpackAbi, UnpackAbiPlain, PackAbi, PackAbiPlain, KnownParamType, KnownParamTypePlain, UnpackerError, UnpackerResult, BuildTokenValue, FunctionBuilder, EventBuilder, TokenValueExt};
            use ton_abi::{Param, ParamType};
            use std::collections::HashMap;

            #(#generated_structs)*

            mod functions {
                use super::*;

                const HEADERS: [#header_type; #header_count] = #slice_token;
                const ABI_VERSION: #abi_type = <#abi_type>::new(#major, #minor);

                #(#generated_functions)*
            }
        }
    };

    quote.into()
}

fn process_function(
    name: String,
    function: &Function,
    structs: &mut Vec<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let mut struct_gen = StructGen::new();
    let (input_name, input_tokens) = struct_gen.make_function_input_struct(function);
    //let (output_name, output_tokens) = struct_gen.make_function_output_struct(function);

    let func = generate_func_body(input_name.as_str());

    for i in struct_gen.internal_structs {
        structs.push(i);
    }

    structs.push(input_tokens);
    //structs.push(output_tokens);
    func
}
fn generate_func_body(name: &str) -> proc_macro2::TokenStream {
    let function_name_ident = format_ident!("{}", name);

    let mut header_tokens: Vec<proc_macro2::TokenStream> = Vec::new();

    let func = quote! {
        fn #function_name_ident() -> &'static everscale_types::abi::Function {
            static ONCE: std::sync::OnceLock<everscale_types::abi::Function> = std::sync::OnceLock::new();
            ONCE.get_or_init(|| {
                everscale_types::abi::FunctionBuilder::new(ABI_VERSION, #name)
                .with_headers(HEADERS)
                .build()
            })
        }
    };
    func
}

struct StructGen {
    internal_structs: Vec<proc_macro2::TokenStream>,
}

impl StructGen {
    fn new() -> Self {
        Self {
            internal_structs: Vec::new(),
        }
    }

    fn make_function_output_struct(
        &mut self,
        function: &Function,
    ) -> (String, proc_macro2::TokenStream) {
        let struct_name = format_ident!("{}FunctionOutput", function.name.as_ref());
        let mut properties = Vec::<proc_macro2::TokenStream>::new();
        for i in function.outputs.iter() {
            let type_name = self.make_struct_property_with_internal(i.name.to_string(), &i.ty);
            let property_name_ident = format_ident!("{}", i.name.as_ref());
            let ty_ident = type_name.type_name();
            let quote = quote! {
                pub #property_name_ident: #ty_ident,
            };
            self.internal_structs.push(quote.into());
        }

        let func = quote! {
            pub struct #struct_name {
                #(#properties)*
            }
        };

        (struct_name.to_string(), func.into())
    }

    fn make_function_input_struct(
        &mut self,
        function: &Function,
    ) -> (String, proc_macro2::TokenStream) {
        let struct_name = format_ident!("{}FunctionInput", function.name.as_ref().to_camel());
        let mut properties = Vec::<proc_macro2::TokenStream>::new();
        for i in function.inputs.iter() {
            let type_name = self.make_struct_property_with_internal(i.name.to_string(), &i.ty);
            let property_name = i.name.as_ref().to_snake();
            let rust_property_name_ident = format_ident!("{}", property_name);
            let contract_name = i.name.as_ref();
            let derive = if property_name == contract_name {
                quote! {
                    #[abi]
                }
            } else {
                quote! {
                   #[abi(name = #property_name)]
                }
            };
            let ty_ident = type_name.type_name();
            let quote = quote! {
                #derive
                pub #rust_property_name_ident: #ty_ident,
            };
            properties.push(quote.into());
            properties.extend_from_slice(self.internal_structs.as_slice());
        }

        let func = quote! {
            #[derive(UnpackAbiPlain, PackAbi, KnownParamTypePlain)]
            pub struct #struct_name {
                #(#properties)*
            }
        };

        (struct_name.to_string(), func.into())
    }

    fn make_struct_property_with_internal(
        &mut self,
        initial_name: String,
        param: &AbiType,
    ) -> StructProperty {
        self.make_struct_property(Some(initial_name), param)
    }

    fn make_struct_property(
        &mut self,
        initial_name: Option<String>,
        param: &AbiType,
    ) -> StructProperty {
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
                    //let name = i.name.clone();
                    let property = self.make_struct_property(Some(i.name.to_string()), &i.ty);
                    let property_name_ident = format_ident!("{}", i.name.as_ref());
                    let ty_ident = property.type_name();

                    let quote = quote! {
                        pub #property_name_ident: #ty_ident,
                    };

                    structs.push(property);
                }

                let mut internal_properties: Vec<proc_macro2::TokenStream> = Vec::new();

                for p in &structs {
                    let name = p.name();
                    let rust_property_name_ident = format_ident!("{}", name.as_str().to_snake());
                    let derive = if rust_property_name_ident == name {
                        quote! {
                            #[abi]
                        }
                    } else {
                        quote! {
                           #[abi(name = #name)]
                        }
                    };
                    let internal_ident = p.type_name();
                    let quote = quote! {
                        #derive
                        pub #rust_property_name_ident: #internal_ident,
                    };
                    internal_properties.push(quote);
                }
                let name = name.unwrap_or_default();
                let struct_name_ident = format_ident!("{}", &name.to_camel());
                let internal_struct = quote! {

                    #[derive(UnpackAbi, PackAbi, KnownParamType)]
                    pub struct #struct_name_ident {
                        #(#internal_properties)*
                    }
                };
                self.internal_structs.push(internal_struct);

                return StructProperty::Tuple {
                    name: name,
                    fields: structs,
                };
            }
            AbiType::Array(a) | AbiType::FixedArray(a, _) => {
                let internal_struct = self.make_struct_property(None, a.as_ref());
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
                        self.make_struct_property(None, &a.clone().into())
                    }
                    _ => panic!("Map key is not allowed type"),
                };

                let value = self.make_struct_property(None, b.as_ref());

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
                let internal_struct = self.make_struct_property(None, a.as_ref());
                return StructProperty::Option {
                    name: name.unwrap_or_default(),
                    internal: Box::new(internal_struct),
                };
            }
            AbiType::Ref(a) => {
                let name = name.map(|x| x.clone());
                return self.make_struct_property(name, a.as_ref());
            }
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

    pub fn name(&self) -> String {
        match self {
            StructProperty::Simple { name, .. } => {
                let name = name.clone().map(|x| x.clone());
                name.unwrap_or_default()
            }
            StructProperty::Tuple { name, .. } => name.clone(),
            StructProperty::Array { name, .. } => name.clone(),
            StructProperty::Option { name, .. } => name.clone(),
            StructProperty::HashMap { name, .. } => name.clone(),
        }
    }
}
