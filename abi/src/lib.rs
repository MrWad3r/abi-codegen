extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Expr, Result};

#[proc_macro]
pub fn abi(input: TokenStream) -> TokenStream {
    println!("{:#?}", input);

    let func = "pub fn time() -> u32 { 42 }";
    let x = func.parse::<TokenStream>().unwrap();
    x
}
