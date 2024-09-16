use anyhow::Result;
use ignition::start_workers;
use alloy::primitives::Address;
use log::{info, LevelFilter};
use pool_sync::*;
use std::collections::BTreeMap;

use alloy::sol;
use std::time::Instant;

mod graph;
mod calculation;
mod ignition;
mod market;
mod simulator;
mod stream;
mod tx_sender;
mod util;
//mod tests;
mod events;
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
pub const AMOUNT: u128 = 10000000000000000;

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
        .add_pools(&[PoolType::UniswapV2])
        .chain(Chain::Ethereum)
        .build()?;

    let (pools, last_synced_block) = pool_sync.sync_pools().await?;

    start_workers(pools, last_synced_block).await;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
    }
    
}
