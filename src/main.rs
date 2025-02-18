//extern crate abi;

use abi_codegen::contracts::qube::functions::*;

fn main() {
    let func = upgrade();

    println!("{}", func.input_id);
    println!("{}", func.output_id);
    println!("{}", func.abi_version);
    println!("{:?}", func.headers);
    println!("{}", func.name);
    println!("{:?}", func.inputs);
    println!("{:?}", func.outputs);
}
