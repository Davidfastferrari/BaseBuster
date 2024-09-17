use alloy::providers::{ProviderBuilder, Provider};
use alloy::primitives::{address, U256};
use log::{info, LevelFilter};
use anyhow::Result;
use pool_sync::*;
use ignition::start_workers;

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

// initial amount we are trying to arb over
pub const AMOUNT: u128 = 10000000000000000;

#[tokio::main]
async fn main() -> Result<()> {
    // init dots and logger
    dotenv::dotenv().ok();
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info) // or Info, Warn, etc.
        .init();

    let provider = ProviderBuilder::new().on_http(std::env::var("ARCHIVE")?.parse()?);
    let address = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
    for i in 0..25 {
        let res = provider.get_storage_at(address, U256::from(i)).await?;
        println!("{:?}", res);

    }

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
