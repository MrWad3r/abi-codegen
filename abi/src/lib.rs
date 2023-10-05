extern crate proc_macro;

use std::fmt::format;
use std::fs;
use std::fs::{read_to_string, File};
use std::path::PathBuf;

use everscale_types::abi::{AbiType, Contract};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use serde_json::Value;
use syn::{parse_macro_input, DeriveInput, Expr, Result};

#[proc_macro]
pub fn abi(input: TokenStream) -> TokenStream {
    let current_dir = std::env::current_dir().unwrap();
    let file_path = PathBuf::from(input.to_string().replace("\"", ""));
    let path = current_dir.join(file_path);

    let content = fs::read_to_string(path).unwrap();
    let contract = serde_json::from_str::<Contract>(&content).unwrap();

    println!("{}", contract.abi_version.major);

    let functions: Vec<_> = contract
        .functions
        .iter()
        .map(|x| {
            let name = x.0.to_string();
            let name = format_ident!("{}_function", name);
            quote! {
                fn #name() -> String {
                    "".to_string()
                }
            }
        })
        .collect();

    let structs: Vec<_> = contract
        .events
        .iter()
        .map(|(name, event)| {
            let name = e.0.to_string();
            let name = format_ident!("{}EventInput", name);
            let field = event.inputs.iter().map(|f| {
                let ty = f.ty;
                quote! {
                    pub #f
                }
            }).collect();
            println!("{}", &name);
            quote! {
                #[derive(Debug, Clone)]
                struct #name {
                    pub
                }
            }
        })
        .collect();

    let quote = quote! {
        #(#structs)*
        #(#functions)*
    };

    quote.into()
}

fn abi_type_to_eversale_type(ty:AbiType) {
    match
}
