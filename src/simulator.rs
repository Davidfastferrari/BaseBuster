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
use tokio::sync::broadcast::{Receiver, Sender};

use crate::{events::*, AMOUNT};
use crate::util::deploy_flash_swap;
use crate::FlashSwap;

// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub async fn simulate_paths(
    tx_sender: Sender<(Vec<FlashSwap::SwapStep>, U256)>,
    mut arb_receiver: Receiver<Event>,
) {
    let FLASH_LOAN_FEE: U256 = U256::from(9) / U256::from(10000); // 0.09% flash loan fee
    let GAS_ESTIMATE: U256 = U256::from(400_000); // Estimated gas used
    let MIN_PROFIT_WEI: U256 = U256::from(1e15); // Minimum profit in wei (0.001 ETH)

    //let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
    //let address = address!("Da7C2a18d51fa876C4DCd4382ae452B811C2A766");
    //let contract = FlashSwap::new(address, provider.clone());
    // deploy the contract and get the address
    let (anvil, flash_addr) = deploy_flash_swap().await;
    // setup the provider on the anvil instance and construt the contract
    let provider = ProviderBuilder::new().on_http("http://localhost:9100".parse().unwrap());
    let contract = FlashSwap::new(flash_addr, provider.clone());

    // simuaation options
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

    // wait for a new arbitrage path
    while let Ok(Event::NewPath(arb_path)) = arb_receiver.recv().await {
        // convert the path from the searcher format to the flash swap format
        let expected = arb_path.1;
        let arb_path = arb_path.0;
        let converted_path: Vec<FlashSwap::SwapStep> = arb_path
            .clone()
            .iter()
            .map(|step| FlashSwap::SwapStep {
                poolAddress: step.pool_address,
                tokenIn: step.token_in,
                tokenOut: step.token_out,
                protocol: step.as_u8(),
                fee: step.fee,
            })
            .collect();

        // simulate the arbitrage and get the result
        let tx = contract
            .executeArbitrage(converted_path.clone(), U256::from(AMOUNT))
            .into_transaction_request();
        let output = provider
            .debug_trace_call(tx, alloy::eips::BlockNumberOrTag::Pending, options.clone())
            .await;

        // process the output
        match output {
            Ok(GethTrace::CallTracer(call_trace)) => {
                if call_trace.error.is_none() {
                    // we have a profitable path, send it over to the sender

                    let profit = extract_profit_log(&call_trace).unwrap();
                    let flash_loan_fee = U256::from(AMOUNT) * FLASH_LOAN_FEE;
                    let gas_cost = U256::from(provider.get_gas_price().await.unwrap()) * GAS_ESTIMATE;
                    
                    let total_cost = U256::from(AMOUNT) + flash_loan_fee + gas_cost;
                    let profit = profit.checked_sub(total_cost).unwrap_or(U256::ZERO);
                    info!("about to send a path");
                    match tx_sender.send((converted_path, profit)) {
                        Ok(_) => info!("Successful path sent"),
                        Err(e) => warn!("Successful path send failed: {:?}", e),
                    }
                    //if profit > MIN_PROFIT_WEI {
                    //} else {
                        //info!("Not sending path, profit too low");
                    //}
                }  else {
                    info!("Path, {:#?}", converted_path);
                    info!("Expected profit: {:#?}", expected);
                    info!("Failed to simulate {:#?}", call_trace.revert_reason);

                } 
            }
            _ => {}
        }
    }
}


fn extract_profit_log(call_frame: &CallFrame) -> Option<U256> {
    // First, check if this frame has the Profit event log
    for log in &call_frame.logs {
        if log.topics.as_ref().map_or(false, |topics| {
            topics.get(0) == Some(&"0x357d905f1831209797df4d55d79c5c5bf1d9f7311c976afd05e13d881eab9bc8".parse().unwrap())
        }) {
            if let Some(data) = &log.data {
                if data.len() >= 32 {
                    return Some(U256::from_be_bytes::<32>(data[0..32].try_into().unwrap()));
                }
            }
        }
    }
    
    // If not found in this frame, recursively check all subcalls
    for subcall in &call_frame.calls {
        if let Some(profit) = extract_profit_log(subcall) {
            return Some(profit);
        }
    }
    
    None
}