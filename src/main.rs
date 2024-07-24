//use crate::build_graph::{construct_graph, find_best_arbitrage_path};
use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::primitives::{address, Address};
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::signers::local::PrivateKeySigner;
use log::{info, LevelFilter};
use petgraph::prelude::*;
use pool_sync::*;
use rustc_hash::FxHashMap;
use std::sync::Arc;

use crate::concurrent_pool::ConcurrentPool;
use crate::graph::*;
use crate::util::*;

mod concurrent_pool;
mod events;
mod gas_manager;
mod graph;
mod market;
mod optimizer;
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

    // Anvil with http provider
    let url = std::env::var("HTTP").unwrap();
    let provider = ProviderBuilder::new().on_http(url.parse().unwrap());
    let fork_block = provider.get_block_number().await.unwrap();
    let anvil = Anvil::new()
        .fork(url)
        .fork_block_number(fork_block)
        .try_spawn()
        .unwrap();
    let http_provider = Arc::new(ProviderBuilder::new().on_http(anvil.endpoint_url()));

    // Websocket provider
    let ws_url = WsConnect::new(std::env::var("WS").unwrap());
    let ws_provider = Arc::new(ProviderBuilder::new().on_ws(ws_url).await.unwrap());

    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let wallet = EthereumWallet::from(signer);

    // Load in all the pools
    info!("Loading and sycning pools...");

    let pool_sync = PoolSync::builder()
        .add_pools(&[PoolType::UniswapV2, PoolType::SushiSwap])
        .chain(Chain::Ethereum)
        .rate_limit(100)
        .build()
        .unwrap();
    let pools = pool_sync.sync_pools(http_provider.clone()).await.unwrap();

    // load in the tokens that have had the top volume
    info!("Loading top volume tokens...");
    let top_volume_tokens = get_top_volume_tokens(Chain::Ethereum).unwrap();

    // all our mappings
    let mut address_to_pool = ConcurrentPool::new(); // for pool data
    let mut address_to_node: FxHashMap<Address, NodeIndex> = FxHashMap::default(); // for finding node idx
    let mut token_to_edge: FxHashMap<(NodeIndex, NodeIndex), EdgeIndex> = FxHashMap::default();

    // build the graph and populate mappings
    info!("Constructing Graph...");
    let graph = Arc::new(build_graph(
        &pools,
        top_volume_tokens,
        &mut address_to_node,
        &mut address_to_pool,
        &mut token_to_edge,
    ));
    let _ = address_to_pool.sync_pools(http_provider.clone()).await;

    // rewrap it
    let address_to_pool = Arc::new(address_to_pool);

    // fetch the weth node index
    let node = *address_to_node
        .get(&address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"))
        .unwrap();

    // build all of the cycles
    info!("Building cycles...");
    let cycles = construct_cycles(&graph, node);
    info!("Found {} cycles", cycles.len());

    // Start all of our workers
    info!("Starting workers...");
    start_workers(http_provider, ws_provider).await;

    Ok(())
}
