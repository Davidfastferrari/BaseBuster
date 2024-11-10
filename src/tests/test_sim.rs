/* 
use revm::db::CacheDB;
//use crate::db::RethDB;
use crate::db::RethDB;
use crate::FlashSwap;
use alloy::primitives::{address, Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::sol;
use alloy::sol_types::SolCall;
use pool_sync::PoolType;
use revm::primitives::AccountInfo;
use revm::primitives::Bytecode;
use revm::primitives::TransactTo;
use revm::Evm;

use alloy::{signers::local::PrivateKeySigner, sol_types::SolValue};
use revm::primitives::{keccak256, Bytes};
use std::sync::Arc;

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

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract IAerodromeRouter {
        struct Route {
            address from;
            address to;
            bool stable;
            address factory;
        }
        function swapExactTokensForTokens(
            uint256 amountIn,
            uint256 amountOutMin,
            Route[] calldata routes,
            address to,
            uint256 deadline
        ) external returns (uint256[] memory amounts);
    }
);

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract IAerodromePool {
        function stable() external view returns (bool);
        function factory() external view returns (address);
    }
);

#[cfg(test)]
mod test_sim {

    use revm::primitives::ExecutionResult;

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_router() {
        let mut db = CacheDB::new(RethDB::new());

        // Define necessary addresses
        let account = address!("c9034c3E7F58003E6ae0C8438e7c8f4598d5ACAA");
        let weth = address!("4200000000000000000000000000000000000006");
        let usdc = address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
        let router_address = address!("cF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43");

        // Set up EVM
        let mut evm = Evm::builder()
            .with_db(db)
            .modify_tx_env(|tx| {
                tx.caller = account;
                tx.transact_to = TransactTo::Call(router_address);
                tx.value = U256::ZERO;
            })
            .build();

        // Prepare swap parameters
        let amount_in = U256::from(1e18); // 1 WETH

        // Create Route struct
        let route = IAerodromeRouter::Route {
            from: weth,
            to: usdc,
            stable: true, // Assume it's not stable, you might want to query this
            factory: address!("420DD381b31aEf6683db6B902084cB0FFECe40Da"),
        };

        // Encode swap function call
        let calldata = IAerodromeRouter::swapExactTokensForTokensCall {
            amountIn: amount_in,
            amountOutMin: U256::ZERO,
            routes: vec![route],
            to: account,
            deadline: U256::MAX,
        }
        .abi_encode();
        evm.tx_mut().data = calldata.into();

        // Execute the swap
        let result = evm.transact();
        println!("{:?}", result);

        /*
        match result.result {
            ExecutionResult::Success { output, .. } => {
                let amounts: Vec<U256> = Vec::<U256>::abi_decode(&output.data(), false).unwrap();
                println!("Swap successful. Amounts: {:?}", amounts);
            },
            _ => println!("Swap failed: {:?}", result)
        }
        */
    }

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
            })
            .build();

        let approve_calldata = Approval::approveCall {
            spender: quoter,
            amount: U256::from(1e18),
        }
        .abi_encode();

        evm.tx_mut().data = approve_calldata.into();

        let ref_tx = evm.transact_commit().unwrap();

        let start = std::time::Instant::now();
        let path = vec![FlashQuoter::SwapStep {
            poolAddress: address!("3548029694fbB241D45FB24Ba0cd9c9d4E745f16"),
            tokenIn: address!("4200000000000000000000000000000000000006"),
            tokenOut: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
            protocol: 15,
            fee: 0.try_into().unwrap(),
        }];

        let calldata = FlashQuoter::quoteArbitrageCall {
            steps: path,
            amount: U256::from(1e16),
        }
        .abi_encode();

        evm.tx_mut().data = calldata.into();
        evm.tx_mut().transact_to = TransactTo::Call(quoter);

        let ref_tx = evm.transact().unwrap();
        let result = ref_tx.result;
        match result {
            ExecutionResult::Success { output: value, .. } => {
                let a = match <U256>::abi_decode(&value.data(), false) {
                    Ok(a) => a,
                    Err(_) => U256::ZERO,
                };
                println!("Profit: {:#?}", a);
                let duration = start.elapsed();
                println!("Time taken: {:?}", duration);
            }
            _ => println!("{:#?}", result),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn contract_sim() {
        sol! {
            #[derive(Debug)]
            #[sol(rpc)]
            contract WETH {
                function approve(address spender, uint256 amount) external returns (bool);
                function increaseAllowance(address spender, uint256 addedValue) external returns (bool);
            }
        };

        let url = std::env::var("FULL").unwrap();
        let amount = U256::from(1e16);
        let provider = Arc::new(ProviderBuilder::new().on_http(url.parse().unwrap()));

        let account = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");
        let weth = address!("4200000000000000000000000000000000000006");
        let quoter = address!("0000000000000000000000000000000000001000"); // Replace with actual quoter address

        let quoter_contract = FlashQuoter::deploy(provider.clone()).await.unwrap();
        let weth = WETH::new(weth, provider.clone());

        weth.approve(quoter_contract.address().clone(), U256::from(1e18))
            .send()
            .await;
    }
}
*/
