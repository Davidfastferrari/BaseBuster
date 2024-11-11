use alloy::network::Ethereum;
use alloy::primitives::{address, U256};
use alloy::providers::RootProvider;
use alloy::sol_types::SolCall;
use alloy::sol_types::SolValue;
use alloy::transports::http::{Client, Http};
use anyhow::{anyhow, Result};
use revm::wiring::default::TransactTo;
use revm::wiring::result::ExecutionResult;
use revm::wiring::EthereumWiring;
use revm::Evm;
use std::sync::Arc;

use crate::gen::FlashQuoter;
use crate::market_state::MarketState;
use crate::state_db::BlockStateDB;

// type to make our life easier
type QuoteEvm<'a> = Evm<
    'a,
    EthereumWiring<&'a mut BlockStateDB<Http<Client>, Ethereum, RootProvider<Http<Client>>>, ()>,
>;

// Quoter. This is used to get a simulation quote before sending off a transaction.
// This will confirm that our offchain calculations are reasonable and make sure we can swap the tokens
pub struct Quoter;
impl Quoter {
    // get a quote for the path
    pub fn quote_path(
        quote_path: Vec<FlashQuoter::SwapStep>,
        amount_in: U256,
        market_state: Arc<MarketState<Http<Client>, Ethereum, RootProvider<Http<Client>>>>,
    ) -> Result<Vec<U256>> {
        let mut guard = market_state.db.write().unwrap();
        // need to pass this as mut somehow
        let mut evm: QuoteEvm = Evm::builder()
            .with_db(&mut *guard)
            .with_default_ext_ctx()
            .build();
        evm.cfg_mut().disable_nonce_check = true;
        evm.tx_mut().caller = address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");
        evm.tx_mut().transact_to =
            TransactTo::Call(address!("0000000000000000000000000000000000001000"));
        // get read access to the db
        // setup the calldata
        let quote_calldata = FlashQuoter::quoteArbitrageCall {
            steps: quote_path,
            amount: amount_in,
        }
        .abi_encode();
        evm.tx_mut().data = quote_calldata.into();

        // transact
        let ref_tx = evm.transact().unwrap();
        let result = ref_tx.result;

        match result {
            ExecutionResult::Success { output: value, .. } => {
                if let Ok(amount) = Vec::<U256>::abi_decode(value.data(), false) {
                    Ok(amount)
                } else {
                    Err(anyhow!("Failed to decode"))
                }
            }
            ExecutionResult::Revert { output, .. } => Err(anyhow!("Simulation reverted {output}")),
            _ => Err(anyhow!("Failed to simulate")),
        }
    }
}