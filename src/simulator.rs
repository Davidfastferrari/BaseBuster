use alloy::primitives::U256;
use alloy::providers::ext::DebugApi;
use alloy::primitives::address;
use alloy::providers::ProviderBuilder;
use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer;
use alloy::rpc::types::trace::geth::GethDebugTracingOptions;
use alloy::rpc::types::trace::geth::{
    CallFrame, GethDebugTracerConfig, GethDebugTracerType, GethDebugTracingCallOptions,
    GethDefaultTracingOptions, GethTrace,
};
use log::{debug, info, warn};
use serde_json::json;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::events::*;
use crate::util::deploy_flash_swap;
use crate::FlashSwap;

// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub async fn simulate_paths(
    tx_sender: Sender<Vec<FlashSwap::SwapStep>>,
    mut arb_receiver: Receiver<Event>,
) {
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
            .executeArbitrage(converted_path.clone(), U256::from(1e15))
            .into_transaction_request();
        let output = provider
            .debug_trace_call(tx, alloy::eips::BlockNumberOrTag::Latest, options.clone())
            .await;

        // process the output
        match output {
            Ok(GethTrace::CallTracer(call_trace)) => {
                if call_trace.error.is_none() {
                    // we have a profitable path, send it over to the sender
                    match tx_sender.send(converted_path) {
                        Ok(_) => info!("Successful path sent"),
                        Err(e) => warn!("Successful path send failed: {:?}", e),
                    }
                } 
            }
            Err(e) => info!("Failed to simulate {:?}", e),
            _ => {}
        }
    }
}
