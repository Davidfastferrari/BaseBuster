use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::primitives::{address, Address, U256};
use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::sol;
use alloy::sol_types::SolCall;
use alloy::sol_types::SolValue;
use alloy::transports::http::{Client, Http};
use anyhow::{anyhow, Result};
use revm::database_interface::WrapDatabaseAsync;
use revm::primitives::keccak256;
use revm::state::{AccountInfo, Bytecode};
use revm::wiring::default::TransactTo;
use revm::wiring::result::ExecutionResult;
use revm::wiring::EthereumWiring;
use std::time::Instant;
use revm::Evm;
use revm_database::{AlloyDB, CacheDB};
use node_db::{NodeDB, InsertionType};

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
        //let url = std::env::var("FULL").unwrap().parse().unwrap();
        //let provider = ProviderBuilder::new().on_http(url);

        // setup the database
        //let db = WrapDatabaseAsync::new(AlloyDB::new(provider, BlockId::latest())).unwrap();
        //let mut cache_db = CacheDB::new(db);
        let database_path = String::from("/home/dsfreakdude/nodes/base/data");
        let mut nodedb = NodeDB::new(database_path).unwrap();

        // give the account some weth
        let one_ether = U256::from(1_000_000_000_000_000_000u128);
        let hashed_acc_balance_slot = keccak256((account, U256::from(3)).abi_encode());
        nodedb
            .insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether, InsertionType::OnChain)
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
    ) -> Result<U256> {
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
                if let Ok(amount) = U256::abi_decode(value.data(), false) {
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
    pub fn optimize_input(
        &mut self,
        quote_path: Vec<FlashQuoter::SwapStep>,
    ) -> Result<U256> {
        let min_amount = U256::from(1_000_000_000_000_000u128); // 0.001 ETH
        let max_amount = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
        let mut best_amount = min_amount;
        let mut best_output = U256::ZERO;
        
        // Binary search with a fixed number of iterations
        for _ in 0..10 {
            let third = (max_amount - min_amount) / U256::from(3);
            let test_amounts = [
                min_amount + third,
                min_amount + (third * U256::from(2)),
            ];

            // Test both points
            for amount in test_amounts {
                if let Ok(output) = self.try_quote(quote_path.clone(), amount) {
                    if output > best_output {
                        best_amount = amount;
                        best_output = output;
                    }
                }
            }
        }

        Ok(best_amount)
    }

    // Helper function to try a single quote
    fn try_quote(&mut self, quote_path: Vec<FlashQuoter::SwapStep>, amount: U256) -> Result<U256> {
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
                U256::abi_decode(value.data(), false)
                    .map_err(|_| anyhow!("Failed to decode"))
            }
            ExecutionResult::Revert { output, .. } => Err(anyhow!("Simulation reverted {output}")),
            _ => Err(anyhow!("Failed to simulate")),
        }
    }
}
