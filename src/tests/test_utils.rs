use alloy::network::Ethereum;
use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::node_bindings::AnvilInstance;
use alloy::primitives::U256;
use alloy::primitives::{address, Address};
use alloy::providers::ext::DebugApi;
use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer;
use alloy::rpc::types::trace::geth::GethDebugTracingOptions;
use alloy::rpc::types::trace::geth::{
    CallConfig, CallFrame, GethDebugTracerConfig, GethDebugTracerType,
    GethDebugTracingCallOptions, GethDefaultTracingOptions, GethTrace,
};
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use alloy::transports::http::{Client, Http};
use alloy::primitives::FixedBytes;
use gweiyser::addresses::amms;
use gweiyser::protocols::uniswap::v2::UniswapV2Pool;
use gweiyser::protocols::uniswap::v3::UniswapV3Pool;
use gweiyser::{Chain, Gweiyser};
use pool_sync::*;
use revm::interpreter::instructions::contract;
use sha2::digest::consts::U25;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::calculation::Calculator;
use crate::events::Event;
use crate::graph::SwapStep;
use crate::pool_manager;
use crate::pool_manager::PoolManager;
use crate::util::get_working_pools;
use crate::FlashSwap;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashQuoter,
    "src/abi/FlashQuoter.json"
);


// Load in all the pools 
pub async fn load_pools() -> (Vec<Pool>, u64) {
    dotenv::dotenv().ok();

    let pool_sync = PoolSync::builder()
        .add_pools(&[
            PoolType::UniswapV2,
            PoolType::BalancerV2,
        ])
        .chain(pool_sync::Chain::Ethereum)
        .build()
        .unwrap();
    pool_sync.sync_pools().await.unwrap()
}

// construct the pool manager from working pools
pub async fn construct_pool_manager(
    pools: Vec<Pool>,
    last_synced_block: u64,
) -> (Arc<PoolManager>, broadcast::Receiver<Event>) {
    let (update_sender, update_receiver) = broadcast::channel(200);
    let pool_manager = PoolManager::new(pools, update_sender, last_synced_block).await;
    (pool_manager, update_receiver)
}

// get a pool manaager that is populated wtih the pools from our address space
pub async fn pool_manager_with_pools(
    addresses: Vec<Address>,
) -> (Arc<PoolManager>, broadcast::Receiver<Event>) {
    let (pools, last_synced_block) = load_pools().await;
    let pools: Vec<Pool> = addresses
        .iter()
        .map(|address| {
            pools
                .clone()
                .into_iter()
                .find(|pool| pool.address() == *address)
                .unwrap()
        })
        .collect();

        println!("Pools: {:#?}", pools);
    let (pool_manager, mut reserve_receiver) =
        construct_pool_manager(pools.clone(), last_synced_block).await;
    (pool_manager, reserve_receiver)
}


// convert from our internal rep to contract rep
pub async fn swappath_to_flashquote(steps: Vec<SwapStep>) -> Vec<FlashQuoter::SwapStep> {
    steps.iter().map(|step| FlashQuoter::SwapStep {
        poolAddress: step.pool_address,
        tokenIn: step.token_in,
        tokenOut: step.token_out,
        protocol: step.as_u8(),
        fee: step.fee,
    }).collect()
}



// just return the tracing options
fn get_tracing_options() -> GethDebugTracingCallOptions {
    let options = GethDebugTracingCallOptions {
        tracing_options: GethDebugTracingOptions {
            config: GethDefaultTracingOptions {
                disable_memory: Some(true),
                disable_stack: Some(true),
                disable_storage: Some(true),
                debug: Some(true),
                disable_return_data: Some(true),
                ..Default::default()
            },
            tracer: Some(GethDebugTracerType::BuiltInTracer(CallTracer)),
            tracer_config: GethDebugTracerConfig(
                serde_json::to_value(CallConfig {
                    only_top_call: Some(false),
                    with_log: Some(true),
                })
                .unwrap()
                .into(),
            ),
            timeout: None,
            ..Default::default()
        },
        state_overrides: None,
        block_overrides: None,
    };
    options
}


