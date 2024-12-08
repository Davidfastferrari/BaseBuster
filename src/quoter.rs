use alloy::network::Ethereum;
use alloy::primitives::{address, U256};
use alloy::providers::RootProvider;
use alloy::sol_types::SolCall;
use alloy::sol_types::SolValue;
use alloy::transports::http::{Client, Http};
use anyhow::{anyhow, Result};
use revm::primitives::{ExecutionResult, TransactTo};
use revm::Evm;
use std::sync::Arc;

use crate::gen::FlashQuoter;
use crate::market_state::MarketState;
use crate::AMOUNT;

// Quoter. This is used to get a simulation quote before sending off a transaction.
// This will confirm that our offchain calculations are reasonable and make sure we can swap the tokens
pub struct Quoter;
impl Quoter {
    // get a quote for the path
    pub fn quote_path(
        quote_params: FlashQuoter::SwapParams,
        market_state: Arc<MarketState<Http<Client>, Ethereum, RootProvider<Http<Client>>>>,
    ) -> Result<Vec<U256>> {
        let mut guard = market_state.db.write().unwrap();
        // need to pass this as mut somehow
        let mut evm = Evm::builder().with_db(&mut *guard).build();
        evm.tx_mut().caller = address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");
        evm.tx_mut().transact_to =
            TransactTo::Call(address!("0000000000000000000000000000000000001000"));
        // get read access to the db
        // setup the calldata
        
        let quote_calldata = FlashQuoter::quoteArbitrageCall {
            params: quote_params,
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

    /// Optimizes the input amount using binary search to find the maximum profitable input
    /// Returns the optimal input amount and its corresponding output amounts
    pub fn _optimize_input(
        quote_path: FlashQuoter::SwapParams,
        market_state: Arc<MarketState<Http<Client>, Ethereum, RootProvider<Http<Client>>>>,
    ) -> Result<(U256, U256)> {
        let mut sim_path = quote_path.clone();
        let mut left = *AMOUNT;
        let mut right = U256::from(1e18);
        let mut best_input = U256::ZERO;
        let mut best_output = U256::ZERO;

        let mut iterations = 0;
        while iterations < 12 {
            let mid = if iterations == 0 {
                *AMOUNT
            } else {
                (left + right) / U256::from(2)
            };


            sim_path.amountIn = mid;
            let current_amounts =
                match Self::quote_path(sim_path.clone(), Arc::clone(&market_state)) {
                    Ok(amounts) => amounts,
                    Err(_) => {
                        right = mid;
                        iterations += 1;
                        continue;
                    }
                };

            let current_output = *current_amounts.last().unwrap_or(&U256::ZERO);
            if best_output == U256::ZERO || current_output > best_output {
                best_output = current_output;
                best_input = mid;
                left = mid;
            } else {
                right = mid;
            }

            iterations += 1;
        }

        if best_input == U256::ZERO {
            Err(anyhow!("Could not find optimal input"))
        } else {
            Ok((best_input, best_output))
        }
    }
}
