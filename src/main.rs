use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::primitives::address;
use alloy::node_bindings::Anvil;
use log::{info, LevelFilter};
use std::sync::Arc;
use pool_sync::*;

use crate::pool_manager::PoolManager;
use crate::ignition::start_workers;
use crate::util::get_working_pools;
use crate::graph::ArbGraph;

mod pool_manager;
mod events;
mod gas_manager;
mod graph;
mod market;
mod optimizer;
mod simulation;
mod calculation;
mod stream;
mod tx_sender;
mod ignition;
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
        .port(portpicker::pick_unused_port().unwrap())
        .try_spawn()
        .unwrap();
    // Wallet signers
    // let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    // let wallet = EthereumWallet::from(signer);
    let http_provider = Arc::new(ProviderBuilder::new().on_http(anvil.endpoint_url()));
    // Websocket provider
    let ws_url = WsConnect::new(std::env::var("WS").unwrap());
    let ws_provider = Arc::new(ProviderBuilder::new().on_ws(ws_url).await.unwrap());

    // Load in all the pools
    info!("Loading and sycning pools...");
    let pool_sync = PoolSync::builder()
        .add_pools(&[PoolType::UniswapV2])
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
    let pool_manager = Arc::new(PoolManager::new(working_pools.clone(), http_provider.clone()).await);

    // build the graph and populate mappings
    info!("Constructing graph and generating cycles...");
    let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let graph = ArbGraph::new(pool_manager.clone(), working_pools.clone(), weth);

    info!("Starting workers...");
    start_workers(http_provider, ws_provider, pool_manager, graph).await;


    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }

    Ok(())
}
