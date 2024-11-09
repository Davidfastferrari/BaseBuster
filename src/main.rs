use alloy::primitives::U256;
use anyhow::Result;
use ignition::start_workers;
use lazy_static::lazy_static;
use log::{info, LevelFilter};
use pool_sync::*;

mod calculation;
mod graph;
mod ignition;
//mod market;
mod simulator;
mod stream;
mod tx_sender;
//mod tests;
mod bytecode;
mod cache;
mod events;
mod filter;
mod gen;
mod market_state;
mod estimator;
mod quoter;
mod searcher;
mod state_db;
mod swap;
mod gas_station;
mod tracing;

// initial amount we are trying to arb over
lazy_static! {
    pub static ref AMOUNT: U256 = U256::from(1e16); //0.1eth
}

#[tokio::main]
async fn main() -> Result<()> {
    // init dots and logger
    dotenv::dotenv().ok();
    env_logger::Builder::new()
        .filter_module("BaseBuster", LevelFilter::Info)
        .format(|buf, record| {
            use std::io::Write;
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            writeln!(
                buf,
                "{} {} - {}",
                timestamp,
                record.level(),
                record.args()
            )
        })
        .init();

    // Load in all the pools
    info!("Loading and syncing pools...");
    let pool_sync = PoolSync::builder()
        .add_pools(&[
            PoolType::UniswapV2,
            //PoolType::SushiSwapV2,
            //PoolType::PancakeSwapV2,
            //PoolType::AlienBaseV2,
            //PoolType::SwapBasedV2,
            //PoolType::DackieSwapV2,
            //PoolType::BaseSwapV2,
            PoolType::Slipstream,
            PoolType::UniswapV3,
            //PoolType::SushiSwapV3, 

        ])
        .chain(Chain::Base)
        .rate_limit(1000)
        .build()?;
    let (pools, last_synced_block) = pool_sync.sync_pools().await?;

    start_workers(pools, last_synced_block).await;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
    }
}
