use alloy::primitives::utils::parse_units;
use alloy::primitives::Address;
<<<<<<< HEAD
use alloy::sol_types::SolCall;
use alloy::primitives::address;
use alloy::providers::{Provider, ProviderBuilder};
=======
>>>>>>> 86750add12efb7ea2e4a20a99cd4e7b0550e3e74
use alloy_sol_types::SolEvent;
use log::info;
use num_bigint::BigUint;
use alloy::providers::{Provider, ProviderBuilder};
use serde_json::json;
use alloy::primitives::utils::parse_units;
use num_traits::{FromPrimitive, One, ToPrimitive, Zero};
use serde_json::json;
use std::str::FromStr;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::events::*;

use alloy::primitives::{U128, U256};
use alloy::providers::ext::{DebugApi, TraceApi};
use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer;
use revm::primitives::{ExecutionResult, Output, TxKind};
use alloy::rpc::types::trace::geth::GethDebugTracingOptions;
use alloy::rpc::types::trace::geth::{
    CallConfig, CallFrame, GethDebugTracerConfig, GethDebugTracerType, GethDebugTracingCallOptions,
    GethDefaultTracingOptions, GethTrace,
};
use alloy::rpc::types::trace::parity::TraceType;
use pool_sync::Pool;
use alloy::providers::ext::{TraceApi, DebugApi};
use alloy::rpc::types::trace::parity::TraceType;
use alloy::rpc::types::trace::geth::{CallConfig, CallFrame, GethDebugTracerConfig, GethDebugTracerType, GethDebugTracingCallOptions, GethDefaultTracingOptions, GethTrace};
use alloy::rpc::types::trace::geth::GethDebugTracingOptions;
use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer;

use alloy::sol;

use crate::FlashSwap;


pub async fn optimize_paths(
    opt_sender: Sender<Event>, 
    mut arb_receiver: Receiver<Event>,
    flash_addr: Address,
) {

    let provider = ProviderBuilder::new()
        .on_http("http://localhost:8545".parse().unwrap());

    let contract = FlashSwap::new(flash_addr, provider.clone());

    let options = GethDebugTracingCallOptions {
        tracing_options: GethDebugTracingOptions {
            config: GethDefaultTracingOptions {
                disable_memory: Some(true),
                disable_stack: Some(true),
                disable_storage: Some(true),
                disable_return_data: Some(true),
                ..Default::default()
            },
            tracer: Some(GethDebugTracerType::BuiltInTracer(CallTracer)),
            tracer_config: GethDebugTracerConfig(json!({
                "withLog": true,
            })),
            timeout: None,
            ..Default::default()
        },
        state_overrides: None,
        block_overrides: None,
    };


<<<<<<< HEAD
use alloy::sol;


use revm::primitives::{Bytecode, TransactTo, AccountInfo};
use revm::Evm;
use revm::db::{AlloyDB, CacheDB};
use alloy::transports::http::{Http, Client};
use crate::FlashSwap;
use alloy::network::Ethereum;
use std::sync::Arc;
use alloy::providers::RootProvider;



pub type AlloyDb = CacheDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>;

pub async fn optimize_paths(
    opt_sender: Sender<Event>,
    mut arb_receiver: Receiver<Event>,
    flash_addr: Address,
) {
    let provider = ProviderBuilder::new().on_http("http://localhost:8545".parse().unwrap());

    let contract = Arc::new(FlashSwap::new(flash_addr, provider.clone()));

    let options = GethDebugTracingCallOptions {
        tracing_options: GethDebugTracingOptions {
            config: GethDefaultTracingOptions {
                /*
                disable_memory: Some(true),
                disable_stack: Some(true),
                disable_storage: Some(true),
                debug: Some(false),
                */
                ..Default::default()
            },
            tracer: Some(GethDebugTracerType::BuiltInTracer(CallTracer)),
            timeout: None,
            ..Default::default()
        },
        state_overrides: None,
        block_overrides: None,
    };



    while let Ok(Event::NewPath(arb_path)) = arb_receiver.recv().await {
        let mut db = CacheDB::new(AlloyDB::new(provider.clone(), Default::default()).unwrap());

        /*
        let bytecode = FlashSwap::DEPLOYED_BYTECODE.clone();
        let revm_bytecode = Bytecode::new_raw(bytecode.clone());
        let code_hash = revm_bytecode.hash_slow();
        let account_info = AccountInfo {
            balance: U256::from(0),
            nonce: 0,
            code_hash,
            code: Some(revm_bytecode)
        };

        db.insert_account_info(flash_addr, account_info);
        */





        //info!("Received arb path: {:?}", arb_path);

        let converted_path: Vec<FlashSwap::SwapStep> = arb_path
            .iter()
            .map(|step| FlashSwap::SwapStep {
                poolAddress: step.pool_address,
                tokenIn: step.token_in,
                tokenOut: step.token_out,
                protocol: step.as_u8(),
            })
            .collect();

        let encoded = FlashSwap::executeArbitrageCall {
            steps: converted_path,
            amount: U256::from(1e17)
        }.abi_encode();

        let mut evm = Evm::builder()
            .with_db(db)
            .modify_tx_env(|tx| {
                tx.caller = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
                tx.transact_to = TxKind::Call(flash_addr);
                tx.data = encoded.into();
                tx.value = U256::from(0);
            })
            .build();

        let result = evm.transact().unwrap();
        info!("Result: {:#?}", result);
        /*
        match result {
            ExecutionResult::Success{ output: Output::Call(value), .. } => info!("{:?}", value),
            _ => info!("not successful")
        }
        */





        /*


        let tx = contract
            .executeArbitrage(converted_path, U256::from(2e17))
            .into_transaction_request();
        let output = provider
            .debug_trace_call(tx, alloy::eips::BlockNumberOrTag::Latest, options.clone())
            .await
            .unwrap();
        if let GethTrace::CallTracer(call_trace) = output {
            if call_trace.error.is_none() {
                println!("Success!");
                let output = extract_profit(&call_trace).unwrap();
                info!(
                    "Profit {:?}",
                    parse_units(output.to_string().as_str(), "ether")
                );
            } else {
                info!("Reverted with reason: {:?}", call_trace.revert_reason);
            }

        }
        */
    }
}

fn extract_profit(frame: &CallFrame) -> Option<U256> {
=======
    while let Ok(Event::NewPath(arb_path)) = arb_receiver.recv().await {
        //info!("Received arb path: {:?}", arb_path);

        let converted_path: Vec<FlashSwap::SwapStep> = arb_path.iter().map(|step| FlashSwap::SwapStep {
            poolAddress: step.pool_address,
            tokenIn: step.token_in,
            tokenOut: step.token_out,
            protocol: step.as_u8()
        }).collect();

        let tx = contract.executeArbitrage(converted_path, U256::from(2e17)).into_transaction_request();
        let output = provider.debug_trace_call(tx, alloy::eips::BlockNumberOrTag::Latest, options.clone()).await.unwrap();
        match output {
            GethTrace::CallTracer(call_trace) => {
                if call_trace.error.is_none() {
                    println!("Success!");
                    let output = extract_profit(&call_trace).unwrap();
                    println!("Profit {:?}", parse_units(output.to_string().as_str(), "ether"));

                } else {
                    println!("Reverted with error: {:?}", call_trace.error)
                }

            }
            _ => {}
        }
        //println!("{:#?}", output);

    }
}


fn extract_profit(frame: &CallFrame) -> Option<U256> {

>>>>>>> 86750add12efb7ea2e4a20a99cd4e7b0550e3e74
    let mut profit = None;

    for log in &frame.logs {
        let topics = log.topics.as_ref().unwrap();
        if topics.contains(&FlashSwap::Profit::SIGNATURE_HASH) {
            //let profit = FlashSwap::Profit::de(&log.data, false).unwrap();
<<<<<<< HEAD
            let profit =
                FlashSwap::Profit::decode_raw_log(topics, &log.data.clone().unwrap(), false)
                    .unwrap();
            //println!("Profit: {:?}", profit);
=======
            let profit = FlashSwap::Profit::decode_raw_log(topics, &log.data.clone().unwrap(), false).unwrap();
            println!("Profit: {:?}", profit);
>>>>>>> 86750add12efb7ea2e4a20a99cd4e7b0550e3e74
        }
    }

    for call in &frame.calls {
        if let Some(child_profit) = extract_profit(call) {
            profit = Some(child_profit);
        }
    }
    profit
<<<<<<< HEAD
=======

>>>>>>> 86750add12efb7ea2e4a20a99cd4e7b0550e3e74
}

/*
fn optimize_amount_in(path: Vec<Address>, reserves: Vec<(U128, U128)>) -> (U256, U256) {
    let mut low = U256::from(0);
    let mut high = U256::from(1e19);
    let mut best_input = U256::from(0);
    let mut best_output = U256::from(0);

    // Perform binary search to find the optimal input amount
    while low <= high {
        let mid = (low + high) / U256::from(2);
        let mut current_amount = mid;

        // Simulate the swaps along the path
        for (i, (reserve_in, reserve_out)) in reserves.iter().enumerate() {
            let zero_for_one = path[i] < path[i + 1];
            if let Some(amount_out) =
                calculate_amount_out(*reserve_in, *reserve_out, current_amount, zero_for_one)
            {
                current_amount = amount_out;
            } else {
                // If the swap fails, set the output to zero
                current_amount = U256::from(0);
                break;
            }
        }

        // Update the best input and output if we found a better result
        if current_amount > best_output {
            best_input = mid;
            best_output = current_amount;
            low = mid + U256::from(1);
        } else {
            high = mid - U256::from(1);
        }
    }

    (best_input, best_output)
}

pub fn calculate_optimal_input(reserves: Vec<(BigUint, BigUint)>) -> Option<BigUint> {
    let fee_denominator = BigUint::from(1_000_000u32);
    let r = BigUint::from(997000u32);

    let (e_a, e_b) = calculate_virtual_pool_params(reserves, &r, &fee_denominator);

    let sqrt_e_a_e_b_r = integer_sqrt(&(&e_a * &e_b * &r / &fee_denominator));

    // Check if sqrt_e_a_e_b_r is greater than e_a before subtracting
    if sqrt_e_a_e_b_r > e_a {
        Some((&sqrt_e_a_e_b_r - &e_a) * &fee_denominator / &r)
    } else {
        None // No profitable arbitrage opportunity
    }
}

fn calculate_virtual_pool_params(
    reserves: Vec<(BigUint, BigUint)>,
    r: &BigUint,
    fee_denominator: &BigUint,
) -> (BigUint, BigUint) {
    let mut e_0 = reserves[0].0.clone();
    let mut e_1 = reserves[0].1.clone();

    for pair in reserves.iter().skip(1) {
        let r_0 = e_0;
        let r_1 = e_1;
        let r_1_prime = &pair.0;
        let r_2 = &pair.1;

        e_0 = &r_0 * r_1_prime / (r_1_prime + &r_1 * r / fee_denominator);
        e_1 = r * &r_1 * r_2 / (r_1_prime + &r_1 * r / fee_denominator);
    }

    (e_0, e_1)
}

fn integer_sqrt(n: &BigUint) -> BigUint {
    if n.is_zero() {
        return BigUint::zero();
    }

    let mut x = n.clone();
    let mut y = BigUint::one();
    while x > y {
        x = (&x + &y) / 2u32;
        y = n / &x;
    }
    x
}

// Helper function to convert U256 to BigUint if needed
fn u256_to_biguint(value: U256) -> BigUint {
    BigUint::from_str(&value.to_string()).unwrap()
}
*/
<<<<<<< HEAD
=======

>>>>>>> 86750add12efb7ea2e4a20a99cd4e7b0550e3e74
