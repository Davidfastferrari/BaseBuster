use anyhow::Result;
use ignition::start_workers;
use log::{info, LevelFilter};
use pool_sync::*;
use alloy::sol;

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
mod tests;
mod db;
mod searcher;
mod swap;
mod cache;

// define our flash swap contract
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashSwap,
    "src/abi/FlashSwap.json"
);

// initial amount we are trying to arb over
pub const AMOUNT: u128 = 7000000000000000;

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
            PoolType::SushiSwapV2,
            PoolType::SwapBasedV2,
            PoolType::BaseSwapV2,
            PoolType::AlienBaseV2,
            PoolType::PancakeSwapV2,
            PoolType::DackieSwapV2,

            //PoolType::Aerodrome,

            PoolType::UniswapV3,
            PoolType::SushiSwapV3,
            PoolType::PancakeSwapV3,
            //PoolType::BaseSwapV3,
        ])
        .chain(Chain::Base)
        .build()?;

    let (pools, last_synced_block) = pool_sync.sync_pools().await?;

    start_workers(pools, last_synced_block).await;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
    }
    
}
