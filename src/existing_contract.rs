use anyhow::Result;
use everscale_types::abi::{AbiValue, Event, FromAbi, Function, IntoAbi, WithAbiType};
use everscale_types::models::Account;
use nekoton_abi::LastTransactionId;

pub struct ExistingContract {
    //#[serde(with = "serde_account_stuff")]
    pub account: everscale_types::models::Account,
    pub last_transaction_id: LastTransactionId,
}

impl ExistingContract {
    pub fn new(account: Account, last_transaction_id: LastTransactionId) -> Self {
        ExistingContract {
            account,
            last_transaction_id,
        }
    }

    pub fn run_local<T: FunctionDescr>(&self, input: T::Input) -> Result<T::Output> {
        let _function = T::function();

        let AbiValue::Tuple(_) = input.into_abi() else {
            anyhow::bail!("Expected input as tuple");
        };

        // TODO: call here smth
        Err(anyhow::Error::msg("not implemented"))
    }

    pub fn run_local_responsible<T: FunctionDescr>(&self, input: T::Input) -> Result<T::Output> {
        let _ = input.into_abi();
        // TODO: call here smth
        Err(anyhow::Error::msg("not implemented"))
    }
}

pub trait FunctionDescr {
    type Input: WithAbiType + IntoAbi + FromAbi;
    type Output: WithAbiType + IntoAbi + FromAbi;

    fn function() -> &'static Function;
}

pub trait EventDescr {
    type Input: WithAbiType + IntoAbi + FromAbi;

    fn event() -> &'static Event;
}
