use anyhow::Result;
use ignition::start_workers;
use alloy::primitives::Address;
use log::{info, LevelFilter};
use market::Market;
use pool_sync::*;
use reth::api::NodeTypesWithDBAdapter;
use std::collections::BTreeMap;

use alloy::sol;
use std::time::Instant;

mod events;
mod graph;
mod calculation;
mod ignition;
mod market;
mod pool_manager;
mod simulator;
mod stream;
mod tx_sender;
mod util;
//mod tests;
mod db;
mod searcher;
mod swap;
mod cache;
mod state_db;
mod tracing;
mod gen;
mod market_state;
mod bytecode;



// define our flash swap contract
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashSwap,
    "src/abi/FlashSwap.json"
);

// initial amount we are trying to arb over
pub const AMOUNT: u128 = 7000000000000000;
use crate::market_state::MarketState;

use alloy::providers::Provider;
use alloy::transports::Transport;
use alloy::network::Network;
use alloy::providers::ext::TraceApi;
use alloy::providers::ProviderBuilder;
use alloy::rpc::types::{BlockId, TransactionRequest};
use alloy::rpc::types::BlockNumberOrTag;
use alloy::rpc::types::trace::common::TraceResult;
use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::PreStateTracer;
use alloy::rpc::types::trace::geth::GethDebugTracerType::BuiltInTracer;
use alloy::rpc::types::trace::geth::*;
use alloy::providers::ext::DebugApi;
use reth_provider::{BlockNumReader, ProviderFactory};
use std::path::Path;
use reth_db::open_db_read_only;
use reth_db::mdbx::{DatabaseArguments, DatabaseEnvKind};
use reth_db::models::ClientVersion;
use reth_provider::providers::StaticFileProvider;
use reth_node_ethereum::EthereumNode;
use reth_optimism_chainspec::BASE_MAINNET;
use reth_provider::StateProviderBox;
use revm::{Database, DatabaseRef, interpreter};
use revm::primitives::{AccountInfo, B256, Bytecode, KECCAK_EMPTY, U256};
use std::sync::Arc;
use std::sync::Mutex;
use reth_db::DatabaseEnv;


#[tokio::main]
async fn main() -> Result<()> {
    // init dots and logger
    dotenv::dotenv().ok();
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info) // or Info, Warn, etc.
        .init();

    // Load in all the pools
    info!("Loading and syncing pools...");
    let pool_sync = PoolSync::builder()
        .add_pools(&[
            PoolType::UniswapV2,
            //PoolType::SushiSwapV2,
            //PoolType::SwapBasedV2,
            //PoolType::BaseSwapV2,
            //PoolType::AlienBaseV2,
            //PoolType::PancakeSwapV2,
            //PoolType::DackieSwapV2,

            //PoolType::Aerodrome,

            //PoolType::UniswapV3,
            //PoolType::SushiSwapV3,
            //PoolType::PancakeSwapV3,
            //PoolType::BaseSwapV3,
        ])
        .chain(Chain::Ethereum)
        .build()?;

    let (pools, last_synced_block) = pool_sync.sync_pools().await?;

    start_workers(pools, last_synced_block).await;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
    }
    
}


pub async fn debug_trace_block<T: Transport + Clone, N: Network, P: Provider<T, N>> (
    client: P,
    block_id: BlockId,
    diff_mode: bool,
) {

    let tracer_opts = GethDebugTracingOptions { config: GethDefaultTracingOptions::default(), ..GethDebugTracingOptions::default() }
        .with_tracer(BuiltInTracer(PreStateTracer))
        .with_prestate_config(PreStateConfig { diff_mode: Some(diff_mode) });
    let start  = Instant::now();
    let results = client.debug_trace_block_by_number(BlockNumberOrTag::Latest, tracer_opts).await.unwrap();
    //println!("time taken: {:?}", start.elapsed());
    //println!("{:#?}", result);

    let mut pre: Vec<BTreeMap<Address, AccountState>> = Vec::new();
    let mut post: Vec<BTreeMap<Address, AccountState>> = Vec::new();

    for trace_result in results.into_iter() {
        if let TraceResult::Success {result, ..} = trace_result {
            match result {
                GethTrace::PreStateTracer(geth_trace_frame) => match geth_trace_frame {
                    PreStateFrame::Diff(diff_frame) => {
                        println!("PRE {:#?}", diff_frame.pre);
                        println!("POST {:#?}", diff_frame.post);
                    }
                    _ => println!("failed")
                }
                _ => println!("failed")
            }


        }


    }

}


