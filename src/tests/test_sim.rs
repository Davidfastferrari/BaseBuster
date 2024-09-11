

use revm::db::CacheDB;
//use crate::db::RethDB;
use alloy::sol_types::SolCall;
use pool_sync::PoolType;
use alloy::primitives::{Address, address, U256};
use crate::FlashSwap;
use alloy::providers::{ProviderBuilder, Provider };
use revm::primitives::Bytecode;
use revm::primitives::AccountInfo;
    use revm::primitives::TransactTo;
use revm::Evm;
use alloy::sol;
use crate::db::RethDB;

use std::sync::Arc;
use alloy::{signers::local::PrivateKeySigner, sol_types::SolValue};
use revm::primitives::{keccak256, Bytes};


sol!(
    #[derive(Debug)]
    contract Approval {
        function approve(address spender, uint256 amount) external returns (bool);
        function deposit(uint256 amount) external;
    }
);

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashQuoter,
    "src/abi/FlashQuoter.json"
);



#[cfg(test)]
mod test_sim {

    use revm::primitives::ExecutionResult;

    use super::*;


    #[tokio::test(flavor = "multi_thread")]
    pub async fn full_quote() {
        let mut db = CacheDB::new(RethDB::new());

        let account = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");
        let weth = address!("4200000000000000000000000000000000000006");
        let quoter = address!("0000000000000000000000000000000000001000"); // Replace with actual quoter address

        // Give ourselves WETH
        let weth_balance_slot = U256::from(3);
        let one_ether = U256::from(1_000_000_000_000_000_000u128);
        let hashed_acc_balance_slot = keccak256((account, weth_balance_slot).abi_encode());
        db.insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether)
            .unwrap();
    
        let acc_info = AccountInfo {
            nonce: 0_u64,
            balance: one_ether,
            code_hash: keccak256(Bytes::new()),
            code: None,
        };
        db.insert_account_info(account, acc_info);
    
        // Insert quoter bytecode
        let quoter_bytecode = FlashQuoter::DEPLOYED_BYTECODE.clone();
        let quoter_acc_info = AccountInfo {
            nonce: 0_u64,
            balance: U256::ZERO,
            code_hash: keccak256(&quoter_bytecode),
            code: Some(Bytecode::new_raw(quoter_bytecode)),
        };
        db.insert_account_info(quoter, quoter_acc_info);

        let mut evm = Evm::builder()
            .with_db(db)
            .modify_tx_env(|tx| {
                tx.caller = account;
                tx.transact_to = TransactTo::Call(weth);
                tx.value = U256::ZERO;
            }).build();

        let approve_calldata = Approval::approveCall {
            spender: quoter,
            amount: U256::from(1e18)
        }.abi_encode();

        evm.tx_mut().data = approve_calldata.into();

        let ref_tx = evm.transact_commit().unwrap();

        let start = std::time::Instant::now();
        let path = vec![
            FlashQuoter::SwapStep {
                poolAddress: address!("68163919a8a996e9fed4466ee98c7da785d5fe34"),
                tokenIn: address!("4200000000000000000000000000000000000006"),
                tokenOut: address!("6921b130d297cc43754afba22e5eac0fbf8db75b"),
                protocol: 1, // BaseSwapV2
                fee: 0.try_into().unwrap(),
            },
            FlashQuoter::SwapStep {
                poolAddress: address!("f609cdba05f08e850676f7434db0d9468b3701bd"),
                tokenIn: address!("6921b130d297cc43754afba22e5eac0fbf8db75b"),
                tokenOut: address!("2075f6e2147d4ac26036c9b4084f8e28b324397d"),
                protocol: 7, // UniswapV3
                fee: 10000.try_into().unwrap(),
            },
            FlashQuoter::SwapStep {
                poolAddress: address!("f282e7c46be1a3758357a5961cf02e1f46a78b75"),
                tokenIn: address!("2075f6e2147d4ac26036c9b4084f8e28b324397d"),
                tokenOut: address!("4200000000000000000000000000000000000006"),
                protocol: 0, // UniswapV2
                fee: 0.try_into().unwrap(),
            },
        ];

        let calldata = FlashQuoter::quoteArbitrageCall {
            steps: path,
            amount: U256::from(1e16)
        }.abi_encode();

        evm.tx_mut().data = calldata.into();
        evm.tx_mut().transact_to = TransactTo::Call(quoter);

        let ref_tx = evm.transact().unwrap();
        let result = ref_tx.result;
        match result {
            ExecutionResult::Success {
                output: value,
                ..
            } => {
                let a = match <U256>::abi_decode(&value.data(), false) {
                    Ok(a) => a,
                    Err(_) => U256::ZERO
                };
                println!("Profit: {:#?}", a);
                let duration = start.elapsed();
                println!("Time taken: {:?}", duration);
            }
            _=> println!("{:#?}", result),
    
    
        }
    }




    #[tokio::test(flavor = "multi_thread")]
    pub async fn contract_sim() {
        let url = std::env::var("FULL").unwrap();
        let amount = U256::from(1e16);
        let provider = ProviderBuilder::new().on_http(url.parse().unwrap());

        // setup the contract
        let flash_quoter_address = address!("71dFd76e36371CaCeca1350C084BCfcb37da52d0");
        let flash_quoter = FlashSwap::new(flash_quoter_address, provider.clone());

        // our path
        let path = vec![
            FlashSwap::SwapStep {
                poolAddress: address!("88A43bbDF9D098eEC7bCEda4e2494615dfD9bB9C"),
                tokenIn: address!("4200000000000000000000000000000000000006"),
                tokenOut: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                protocol: 0,
                fee: 0.try_into().unwrap(),
            },
            FlashSwap::SwapStep {
                poolAddress: address!("d0b53D9277642d899DF5C87A3966A349A798F224"),
                tokenIn: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                tokenOut: address!("4200000000000000000000000000000000000006"),
                protocol: 7,
                fee: 500.try_into().unwrap(),
            },
        ];


        let res  = flash_quoter
            .executeArbitrage(path, amount)
            .call()
            .await;
        println!("{:?}", res);
    }

}


