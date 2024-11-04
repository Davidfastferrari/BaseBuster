use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::primitives::{address, Address, U256};
use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::sol;
use alloy::sol_types::SolCall;
use alloy::sol_types::SolValue;
use alloy::transports::http::{Client, Http};
use anyhow::{anyhow, Result};
use node_db::{InsertionType, NodeDB};
use revm::database_interface::WrapDatabaseAsync;
use revm::primitives::keccak256;
use revm::state::{AccountInfo, Bytecode};
use revm::wiring::default::TransactTo;
use revm::wiring::result::ExecutionResult;
use revm::wiring::EthereumWiring;
use revm::Evm;
use revm_database::{AlloyDB, CacheDB};
use std::time::Instant;

use crate::gen::FlashQuoter;

// Types to make our life easier
type AlloyCacheDB =
    CacheDB<WrapDatabaseAsync<AlloyDB<Http<Client>, Ethereum, RootProvider<Http<Client>>>>>;
type QuoterEvm = Evm<'static, EthereumWiring<NodeDB, ()>>;

// Quoter. This class is used to get a simulation quote before sending off a transaction.
// This will confirm that our offchain calculations are reasonable and make sure we can swap the tokens
pub struct Quoter {
    evm: QuoterEvm,
}

impl Quoter {
    // Setup quoter with account information and proper approvals
    pub async fn new() -> Self {
        // approval call for contract
        sol!(
            #[derive(Debug)]
            contract Approval {
                function approve(address spender, uint256 amount) external returns (bool);
                function deposit(uint256 amount) external;
            }
        );

        let account = address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");
        let weth = std::env::var("WETH").unwrap().parse().unwrap();
        let quoter: Address = address!("0000000000000000000000000000000000001000");

        // setup the provider

        // setup the database
        let database_path = std::env::var("DB_PATH").unwrap();
        let mut nodedb = NodeDB::new(database_path).unwrap();

        // give the account some weth
        let one_ether = U256::from(1_000_000_000_000_000_000u128);
        let hashed_acc_balance_slot = keccak256((account, U256::from(3)).abi_encode());
        nodedb
            .insert_account_storage(
                weth,
                hashed_acc_balance_slot.into(),
                one_ether,
                InsertionType::OnChain,
            )
            .unwrap();

        // insert the quoter bytecode
        let quoter_bytecode = FlashQuoter::DEPLOYED_BYTECODE.clone();
        let quoter_acc_info = AccountInfo {
            nonce: 0_u64,
            balance: U256::ZERO,
            code_hash: keccak256(&quoter_bytecode),
            code: Some(Bytecode::new_raw(quoter_bytecode)),
        };
        nodedb.insert_account_info(quoter, quoter_acc_info, InsertionType::Custom);

        let mut evm = Evm::<EthereumWiring<NodeDB, ()>>::builder()
            .with_db(nodedb)
            .with_default_ext_ctx()
            .modify_tx_env(|tx| {
                tx.caller = account;
            })
            .build();
        evm.cfg_mut().disable_nonce_check = true;

        // approve quoter to spend the eth
        let approve_calldata = Approval::approveCall {
            spender: quoter,
            amount: U256::from(1e18),
        }
        .abi_encode();
        evm.tx_mut().data = approve_calldata.into();
        evm.tx_mut().transact_to = TransactTo::Call(weth);
        evm.transact_commit().unwrap();

        // setup call address for quotes
        evm.tx_mut().transact_to = TransactTo::Call(quoter);

        // we now have a database with an account with 1 weth, our quoter bytecode, and the quoter approved to spend 1 weth
        Self { evm }
    }

    // get a quote for the path
    pub fn quote_path(
        &mut self,
        quote_path: Vec<FlashQuoter::SwapStep>,
        amount_in: U256,
        block_number: u64
    ) -> Result<Vec<U256>> {
        // setup the calldata
        let quote_calldata = FlashQuoter::quoteArbitrageCall {
            steps: quote_path,
            amount: amount_in,
        }
        .abi_encode();
        self.evm.tx_mut().data = quote_calldata.into();

        // transact
        let ref_tx = self.evm.transact().unwrap();
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

    // optimize the input amount
    /* 
    pub fn optimize_input(
        &mut self,
        quote_path: Vec<FlashQuoter::SwapStep>,
    ) -> Result<(U256, U256)> {
        let mut left = U256::from(1_000_000_000_000_000u128); // 0.001 ETH
        let mut right = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
        let mut best_amount = left;
        let mut best_output = U256::ZERO;

        // Binary search with golden-section search inspired approach
        while right - left > U256::from(1_000_000_000_000u128) {
            // 0.000001 ETH precision
            let mid1 = left + (right - left) / U256::from(3);
            let mid2 = right - (right - left) / U256::from(3);

            let output1 = self
                .try_quote(quote_path.clone(), mid1)
                .unwrap_or(U256::ZERO);
            let output2 = self
                .try_quote(quote_path.clone(), mid2)
                .unwrap_or(U256::ZERO);

            if output1 > output2 {
                right = mid2;
                if output1 > best_output {
                    best_output = output1;
                    best_amount = mid1;
                }
            } else {
                left = mid1;
                if output2 > best_output {
                    best_output = output2;
                    best_amount = mid2;
                }
            }
        }
        

        Ok((best_amount, best_output))
    }
    */

    // Helper function to try a single quote
    fn try_quote(&mut self, quote_path: Vec<FlashQuoter::SwapStep>, amount: U256) -> Result<Vec<U256>> {
        let quote_calldata = FlashQuoter::quoteArbitrageCall {
            steps: quote_path,
            amount,
        }
        .abi_encode();
        self.evm.tx_mut().data = quote_calldata.into();

        let ref_tx = self.evm.transact().unwrap();
        let result = ref_tx.result;

        match result {
            ExecutionResult::Success { output: value, .. } => {
                Vec::<U256>::abi_decode(value.data(), false).map_err(|_| anyhow!("Failed to decode"))
            }
            ExecutionResult::Revert { output, .. } => Err(anyhow!("Simulation reverted {output}")),
            _ => Err(anyhow!("Failed to simulate")),
        }
    }
}
