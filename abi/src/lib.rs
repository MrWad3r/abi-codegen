extern crate proc_macro;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use case::CaseExt;
use everscale_types::abi::{
    AbiHeaderType, AbiType, Contract, Function, NamedAbiType, PlainAbiType,
};
use proc_macro::TokenStream;
use quote::{format_ident, quote};

use self::models::FunctionDescriptionTokens;

mod models;
mod trait_impl_gen;

#[proc_macro]
pub fn abi(input: TokenStream) -> TokenStream {
    let mut generated_structs: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut generated_functions: Vec<proc_macro2::TokenStream> = Vec::new();

    let current_dir = std::env::current_dir().unwrap();
    println!("{}", input.to_string());
    let array = input
        .to_string()
        .split(',')
        .map(|x| x.trim().to_string())
        .collect::<Vec<String>>();

    let file_path = PathBuf::from(array[1].to_string().replace("\"", ""));
    let path = current_dir.join(file_path);

    let content = fs::read_to_string(path).unwrap();
    let contract = serde_json::from_str::<Contract>(&content).unwrap();

    let mut struct_gen = StructGen::new();

    contract.functions.iter().for_each(|(name, function)| {
        let name = name.to_string();
        let FunctionDescriptionTokens {
            body,
            input,
            output,
            inner_models,
        } = struct_gen.process_function(name, function);

        generated_functions.push(body);

        generated_structs.push(input);
        generated_structs.push(output);
        generated_structs.extend_from_slice(&inner_models.as_slice());
    });

    let trait_gen = trait_impl_gen::TraitImplGen::new();

    let mut trait_implementations: Vec<proc_macro2::TokenStream> = Vec::new();

    for (name, properties) in struct_gen.generated_structs.iter() {
        let struct_traits = trait_gen.implement_traits(&name, properties.as_slice());
        trait_implementations.push(struct_traits)
    }

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

    let contract_name = format_ident!("{}", array[0].replace("\"", ""));
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

        pub mod #contract_name {
            use anyhow::Result;
            use nekoton_abi::{BuildTokenValue, FunctionBuilder, EventBuilder, TokenValueExt};
            use everscale_types::abi::{NamedAbiType, AbiType, WithAbiType, IntoAbi, IntoPlainAbi,
                FromAbiIter, FromAbi, AbiValue, NamedAbiValue
            };
            use num_bigint::{BigInt, BigUint};



            #(#generated_structs)*

            #(#trait_implementations)*

            pub mod functions {
                use super::*;

                const HEADERS: [#header_type; #header_count] = #slice_token;
                const ABI_VERSION: #abi_type = <#abi_type>::new(#major, #minor);

                #(#generated_functions)*
            }
        }
    };

    quote.into()
}

struct StructGen {
    generated_structs: std::collections::HashMap<String, Vec<NamedAbiType>>,

    unique_tokes: std::collections::HashMap<AbiType, StructProperty>,

    //used only for one function
    temporary_internal_structs_idents: Vec<proc_macro2::TokenStream>,
}

impl StructGen {
    fn new() -> Self {
        Self {
            unique_tokes: std::collections::HashMap::new(),
            temporary_internal_structs_idents: Vec::new(),

            generated_structs: std::collections::HashMap::new(),
        }
    }

    fn process_function(&mut self, name: String, function: &Function) -> FunctionDescriptionTokens {
        let input_token = self.make_function_input_struct(function);
        let output_token = self.make_function_output_struct(function);

        let mut inner_modes = Vec::new();

        let func = self.generate_func_body(&name);

        for i in self.temporary_internal_structs_idents.iter() {
            inner_modes.push(i.clone());
        }

        self.temporary_internal_structs_idents.clear();

        FunctionDescriptionTokens {
            body: func,
            input: input_token,
            output: output_token,
            inner_models: inner_modes,
        }
    }

    fn generate_func_body(&self, name: &str) -> proc_macro2::TokenStream {
        let snake_function_name = name.to_snake();
        let function_name_ident = format_ident!("{}", snake_function_name);

        let func = quote! {
            pub fn #function_name_ident() -> &'static everscale_types::abi::Function {
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

    fn generate_model(
        &mut self,
        name: &str,
        values: Arc<[NamedAbiType]>,
    ) -> proc_macro2::TokenStream {
        let struct_name_ident = format_ident!("{}", name);
        let mut properties = Vec::<proc_macro2::TokenStream>::new();

        let mut inner_fields = Vec::new();

        let function_tuple = AbiType::Tuple(values.clone());

        for i in values.iter() {
            let struct_property = match self.unique_tokes.get(&i.ty) {
                Some(struct_property) => struct_property.clone(),
                None => self.make_struct_property_with_internal(i.name.to_string(), &i.ty),
            };

            self.unique_tokes
                .insert(i.ty.clone(), struct_property.clone());

            inner_fields.push(struct_property.clone());

            let rust_name = i.name.as_ref().to_snake();
            let rust_property_name_ident = format_ident!("{}", &rust_name);

            let ty_ident = struct_property.type_name_quote();

            let quote = quote! {
                pub #rust_property_name_ident: #ty_ident,
            };

            properties.push(quote.into());
        }

        if !inner_fields.is_empty() {
            self.unique_tokes.insert(
                function_tuple.clone(),
                StructProperty::Tuple {
                    name: name.to_string(),
                    //fields: inner_fields,
                },
            );
        }

        let func = quote! {
            #[derive(Clone, Debug)]
            pub struct #struct_name_ident {
                #(#properties)*
            }
        };

        func.into()
    }

    fn make_function_input_struct(&mut self, function: &Function) -> proc_macro2::TokenStream {
        let struct_name = format!("{}FunctionInput", function.name.as_ref().to_camel());
        let model = self.generate_model(&struct_name, function.inputs.clone());

        if !self.generated_structs.contains_key(&struct_name) {
            self.generated_structs
                .insert(struct_name.clone(), function.inputs.to_vec());
        }

        model.into()
    }

    fn make_function_output_struct(&mut self, function: &Function) -> proc_macro2::TokenStream {
        let struct_name = format!("{}FunctionOutput", function.name.as_ref().to_camel());
        let model = self.generate_model(&struct_name, function.outputs.clone());

        if !self.generated_structs.contains_key(&struct_name) {
            self.generated_structs
                .insert(struct_name.clone(), function.outputs.to_vec());
        }

        model.into()
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
        let name = initial_name.map(|x| x.to_string());
        if let Some(st_property) = self.unique_tokes.get(param) {
            if name
                .clone()
                .map(|name| st_property.name().eq(&name))
                .unwrap_or(false)
            {
                return st_property.clone();
            }
        }

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
                let name = name.unwrap_or_default();
                let camel_case_struct_name = name.to_camel();
                let struct_name_ident = format_ident!("{}", &camel_case_struct_name);

                let mut structs: Vec<StructProperty> = Vec::new();

                for i in a.iter() {
                    let property = self.make_struct_property(Some(i.name.to_string()), &i.ty);
                    structs.push(property);
                }

                let mut internal_properties: Vec<proc_macro2::TokenStream> = Vec::new();

                for p in &structs {
                    let p_name = p.name();
                    let rust_property_name_ident = format_ident!("{}", p_name.as_str().to_snake());
                    let internal_ident = p.type_name_quote();
                    let quote = quote! {
                        pub #rust_property_name_ident: #internal_ident,
                    };
                    internal_properties.push(quote);
                }

                let internal_struct = quote! {

                    #[derive(Clone, Debug)]
                    pub struct #struct_name_ident {
                        #(#internal_properties)*
                    }
                };

                println!("{:?}", internal_struct.to_string());

                self.temporary_internal_structs_idents.push(internal_struct);

                let property = StructProperty::Tuple {
                    name: name,
                    //fields: structs,
                };

                {
                    self.unique_tokes.insert(param.clone(), property.clone());

                    if !self.generated_structs.contains_key(&camel_case_struct_name) {
                        self.generated_structs
                            .insert(camel_case_struct_name, a.to_vec());
                    }
                }

                return property;
            }
            AbiType::Array(a) | AbiType::FixedArray(a, _) => {
                let internal_struct = self.make_struct_property(None, a);
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
                type_name: syn::parse_quote!(everscale_types::models::message::StdAddr),
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

#[derive(Clone)]
enum StructProperty {
    Simple {
        name: Option<String>,
        type_name: syn::Type,
    },
    Tuple {
        name: String,
        //_fields: Vec<StructProperty>,
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
    pub fn type_name_quote(&self) -> syn::Type {
        match self {
            StructProperty::Simple { type_name, .. } => type_name.clone(),
            StructProperty::Tuple { name, .. } => syn::parse_str(&name.to_camel()).unwrap(),
            StructProperty::Array { internal, .. } => {
                let ty = internal.type_name_quote();
                syn::parse_quote!(Vec<#ty>)
            }
            StructProperty::Option { internal, .. } => {
                let ty = internal.type_name_quote();
                syn::parse_quote!(Option<#ty>)
            }
            StructProperty::HashMap { key, value, .. } => {
                let key = key.type_name_quote();
                let value = value.type_name_quote();
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
