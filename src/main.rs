use alloy::providers::{ProviderBuilder, WsConnect};
use alloy::sol;
use anyhow::Result;
use ignition::start_workers;
use log::{info, LevelFilter};
use pool_sync::*;
use std::sync::Arc;

mod calculation;
mod events;
mod gas_manager;
mod graph;
mod ignition;
mod market;
mod pool_manager;
mod simulator;
mod stream;
mod tx_sender;
mod util;

// define our flash swap contract
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashSwap,
    "src/abi/FlashSwap.json"
);

// initial amount we are trying to arb over
pub const AMOUNT: u128 = 1_000_000_000_000_000;

#[tokio::main]
async fn main() -> Result<()> {
    // init dots and logger
    dotenv::dotenv().ok();
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info) // or Info, Warn, etc.
        .init();

    // http provider and websocket provider
    info!("Constructing providers");
    let http_url = std::env::var("FULL").unwrap();
    let ws_url = WsConnect::new(std::env::var("WS").unwrap());
    let http_provider = Arc::new(ProviderBuilder::new().on_http(http_url.parse()?));
    let ws_provider = Arc::new(ProviderBuilder::new().on_ws(ws_url).await?);

    // Load in all the pools
    info!("Loading and syncing pools...");
    let pool_sync = PoolSync::builder()
        .add_pools(&[
            PoolType::UniswapV2,
            PoolType::SushiSwapV2,
            PoolType::UniswapV3,
            PoolType::SushiSwapV3,
        ])
        .chain(Chain::Base)
        .build()?;
    let pools = pool_sync.sync_pools().await?;

    start_workers(http_provider, ws_provider, pools).await;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
    }
}
