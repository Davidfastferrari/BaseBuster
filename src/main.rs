use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::node_bindings::Anvil;
use alloy::primitives::address;
use log::{info, LevelFilter};
use pool_sync::*;
use std::sync::Arc;

use crate::graph::ArbGraph;
use crate::ignition::start_workers;
use crate::pool_manager::PoolManager;
use crate::util::get_working_pools;

mod calculation;
mod events;
mod gas_manager;
mod graph;
mod ignition;
mod market;
mod optimizer;
mod pool_manager;
mod simulation;
mod stream;
mod tx_sender;
mod util;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // initializations
    dotenv::dotenv().ok();
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info) // or Info, Warn, etc.
        .init();

    // construct the providers
    info!("Constructing providers...");
    // Http provider, utilizing anvil instance
    let url = std::env::var("HTTP").unwrap();
    let http_provider = Arc::new(ProviderBuilder::new().on_http(url.parse().unwrap()));
    let fork_block = http_provider.get_block_number().await.unwrap();
    let anvil = Anvil::new()
        .fork(url)
        .fork_block_number(fork_block)
        //.port(portpicker::pick_unused_port().unwrap())
        .try_spawn()
        .unwrap();
    info!("Anvil endpoint: {}", anvil.endpoint_url());
    // Wallet signers
    let anvil_provider = Arc::new(ProviderBuilder::new().on_http(anvil.endpoint_url()));
    let block = anvil_provider.get_block_number().await.unwrap();
    print!("Block number: {:?}", block);
    // Websocket provider
    let ws_url = WsConnect::new(std::env::var("WS").unwrap());
    let ws_provider = Arc::new(ProviderBuilder::new().on_ws(ws_url).await.unwrap());

    // Load in all the pools
    info!("Loading and syncing pools...");
    let pool_sync = PoolSync::builder()
        .add_pools(&[PoolType::UniswapV2, PoolType::SushiSwap])
        .chain(Chain::Ethereum)
        .rate_limit(100)
        .build()
        .unwrap();
    let pools = pool_sync.sync_pools(http_provider.clone()).await.unwrap();

    // load in the tokens that have had the top volume
    info!("Getting our set of working pools...");
    let working_pools = get_working_pools(pools, 3000, Chain::Ethereum).await;

    // Maintains reserves updates and pool state
    info!("Constructing the pool manager and getting initial reserves...");
    let pool_manager =
        Arc::new(PoolManager::new(working_pools.clone(), anvil_provider.clone()).await);

    // build the graph and populate mappings
    info!("Constructing graph and generating cycles...");
    let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let graph = ArbGraph::new(pool_manager.clone(), working_pools.clone(), weth);

    info!("Starting workers...");
    start_workers(anvil_provider, ws_provider, pool_manager, graph).await;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }

    Ok(())
}
