extern crate abi;
use abi::abi;

abi!("hello");

fn main() {
    let x = time();
    println!("{x:?}");
}
