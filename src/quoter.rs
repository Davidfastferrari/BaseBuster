use alloy::primitives::{address, Address, U256};
use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::network::Ethereum;
use alloy::transports::http::{Http, Client};
use anyhow::{anyhow, Result};
use alloy::sol_types::SolCall;
use alloy::sol;
use alloy::sol_types::SolValue;
use alloy::eips::BlockId;
use revm::state::{AccountInfo, Bytecode};
use revm::wiring::default::TransactTo;
use revm::wiring::result::ExecutionResult;
use revm::wiring::EthereumWiring;
use revm::primitives::keccak256;
use revm::Evm;
use revm::database_interface::WrapDatabaseAsync;
use revm_database::{AlloyDB, CacheDB};

use crate::gen::FlashQuoter;

// Types to make our life easier
type AlloyCacheDB = CacheDB<WrapDatabaseAsync<AlloyDB<Http<Client>, Ethereum, RootProvider<Http<Client>>>>>;
type QuoterEvm = Evm<'static, EthereumWiring<AlloyCacheDB, ()>>;

// Quoter. This class is used to get a simulation quote before sending off a transaction.
// This will confirm that our offchain calculations are reasonable and make sure we can swap the tokens
pub struct Quoter {
    evm: QuoterEvm
}

impl Quoter {
    // Setup quoter with account information and proper approvals
    pub fn new() -> Self{
        // approval call for contract 
        sol!(
            #[derive(Debug)]
            contract Approval {
                function approve(address spender, uint256 amount) external returns (bool);
                function deposit(uint256 amount) external;
            }
        );

        let account = address!("18B06aaF27d44B756FCF16Ca20C1f183EB49111f");
        let weth = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let quoter: Address = address!("0000000000000000000000000000000000001000");

        // setup the provider
        let url = std::env::var("FULL").unwrap().parse().unwrap();
        let provider = ProviderBuilder::new().on_http(url);

        // setup the database
        let db = WrapDatabaseAsync::new(AlloyDB::new(provider, BlockId::latest())).unwrap();
        let mut cache_db = CacheDB::new(db);

        // give the account some weth
        let one_ether = U256::from(1_000_000_000_000_000_000u128);
        let hashed_acc_balance_slot = keccak256((account, U256::from(3)).abi_encode());
        cache_db
            .insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether)
            .unwrap();

        // insert the quoter bytecode
        let quoter_bytecode = FlashQuoter::DEPLOYED_BYTECODE.clone();
        let quoter_acc_info = AccountInfo {
            nonce: 0_u64,
            balance: U256::ZERO,
            code_hash: keccak256(&quoter_bytecode),
            code: Some(Bytecode::new_raw(quoter_bytecode)),
        };
        cache_db.insert_account_info(quoter, quoter_acc_info);

        // setup evm instance with approve call
        let approve_calldata = Approval::approveCall {
            spender: quoter,
            amount: U256::from(1e18),
        }
        .abi_encode();

        let mut evm = Evm::<EthereumWiring<AlloyCacheDB, ()>>::builder()
            .with_db(cache_db)
            .with_default_ext_ctx()
            .modify_tx_env(|tx| {
                tx.caller = address!("0000000000000000000000000000000000000001");
                tx.value = U256::ZERO;
                tx.data = approve_calldata.into();
            })
            .build();

        // persist the approve call
        evm.transact_commit().unwrap();

        // setup call address for quotes
        evm.tx_mut().transact_to = TransactTo::Call(quoter);

        // we now have a database with an account with 1 weth, our quoter bytecode, and the quoter approved to spend 1 weth
        Self {evm}
    }

    // get a quote for the path
    pub fn quote_path(&mut self, quote_path: Vec<FlashQuoter::SwapStep>, amount_in: U256) -> Result<U256> {
        // setup the calldata
        let quote_calldata = FlashQuoter::quoteArbitrageCall {
            steps: quote_path,
            amount: amount_in,
        }.abi_encode();
        self.evm.tx_mut().data = quote_calldata.into();

        // transact
        let ref_tx = self.evm.transact().unwrap();
        let result = ref_tx.result;

        match result {
            ExecutionResult::Success { output: value, .. } => {
                if let Ok(amount) = U256::abi_decode(&value.data(), false) {
                    Ok(amount)
                } else {
                    Err(anyhow!("Failed to decode"))
                }
            }
            _ => Err(anyhow!("Failed to simulate"))
        }
    }
}
