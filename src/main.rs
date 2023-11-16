//extern crate abi;
use abi::abi;

abi!("qube", "abi.json");

fn main() {
    //let qube_contract: ExistingContract = qube::contract::QubeContract::new();
    //qube_contract.run_local::<qube::GetGaugeVotesFunction>(input)
    let func = qube::functions::get_gauge_votes();
    println!("{}",func.abi_version );



}
