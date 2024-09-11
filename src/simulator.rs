use alloy::primitives::U256;
use alloy::providers::ext::DebugApi;
use alloy::primitives::address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer;
use alloy::rpc::types::trace::geth::GethDebugTracingOptions;
use alloy::rpc::types::trace::geth::{
    CallFrame, GethDebugTracerConfig, GethDebugTracerType, GethDebugTracingCallOptions,
    GethDefaultTracingOptions, GethTrace,
};
use log::{debug, info, warn};
use serde_json::json;
use alloy::eips::BlockId;
use alloy::eips::BlockNumberOrTag;
use std::sync::mpsc::{Receiver, Sender};
use alloy::node_bindings::Anvil;

use revm::db::CacheDB;
//use crate::db::RethDB;
use alloy::sol_types::SolCall;
use pool_sync::PoolType;
use alloy::primitives::{Address};
use crate::FlashSwap;
use revm::primitives::Bytecode;
use revm::primitives::AccountInfo;
    use revm::primitives::TransactTo;
use revm::Evm;
use alloy::sol;
//use gweiyser::protocols::uniswap::v2::UniswapV2Pool;
//use gweiyser::protocols::uniswap::v3::UniswapV3Pool;
//use gweiyser::{Chain, Gweiyser};
use alloy::network::Ethereum;
use alloy::network::EthereumWallet;
use crate::db::RethDB;

use std::sync::Arc;
use alloy::{signers::local::PrivateKeySigner, sol_types::SolValue};
use revm::primitives::{keccak256, Bytes, ExecutionResult};

use crate::{events::*, AMOUNT};
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashQuoter,
    "src/abi/FlashQuoter.json"
);

sol!(
    #[derive(Debug)]
    contract Approval {
        function approve(address spender, uint256 amount) external returns (bool);
        function deposit(uint256 amount) external;
    }
);
// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub fn simulate_paths(
    tx_sender: Sender<Vec<FlashSwap::SwapStep>>,
    arb_receiver: Receiver<Event>,
) {
    // construct our db
    let mut db = CacheDB::new(RethDB::new());

    // account, weth, quoter addresses
    let account = address!("1E0294b6e4D72857B5eC467f5c2E52BDA37CA5b8");
    let weth = address!("4200000000000000000000000000000000000006");
    let quoter = address!("0000000000000000000000000000000000001000"); 

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

    let _ = evm.transact_commit().unwrap();

    // wait for a new arbitrage path
    while let Ok(Event::NewPath((arb_path, out))) = arb_receiver.recv() {
        // convert the path from searcher format into flash swap format
        let converted_path: Vec<FlashQuoter::SwapStep> = arb_path
            .clone()
            .iter()
            .map(|step| FlashQuoter::SwapStep {
                poolAddress: step.pool_address,
                tokenIn: step.token_in,
                tokenOut: step.token_out,
                protocol: step.as_u8(),
                fee: step.fee.try_into().unwrap(),
            })
            .collect();
        //println!("{:#?}", converted_path);


        // make our calldata
        let calldata = FlashQuoter::quoteArbitrageCall {
            steps: converted_path,
            amount: U256::from(2e16)
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
                if let Ok(amount) = U256::abi_decode(&value.data(), false) {
                    println!("output {:?}, expected {}", amount, out);
                    let converted_path: Vec<FlashSwap::SwapStep> = arb_path
                        .clone()
                        .iter()
                        .map(|step| FlashSwap::SwapStep {
                            poolAddress: step.pool_address,
                            tokenIn: step.token_in,
                            tokenOut: step.token_out,
                            protocol: step.as_u8(),
                            fee: step.fee.try_into().unwrap(),
                        })
                        .collect();
                    match tx_sender.send(converted_path) {
                        Ok(_) => info!("Successful path sent"),
                        Err(e) => warn!("Successful path send failed: {:?}", e),
                    }
                }
            }
            _ => {}//println!("{:#?}", result),
        }
    }
}