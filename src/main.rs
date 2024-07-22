//use crate::build_graph::{construct_graph, find_best_arbitrage_path};
use crate::concurrent_pool::ConcurrentPool;
use crate::events::Events;
use crate::graph::*;
use alloy::primitives::{U128, U256};
use alloy::sol;
use log::info;
use alloy::primitives::address;
use alloy::primitives::Address;
use alloy::providers::ProviderBuilder;
use alloy::providers::WsConnect;
use env_logger;
use petgraph::algo;
use petgraph::prelude::*;
use pool_sync::filter::filter_top_volume;
use pool_sync::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use stream::*;
use tokio::sync::mpsc;
use log::LevelFilter;

mod concurrent_pool;
mod events;
mod graph;
mod stream;
mod optimizer;


sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract UniswapV2Router {
        function getAmountsOut(uint amountIn, address[] memory path) external view returns (uint[] memory amounts);
    }
);

// Pair contract to get reserves
sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract UniswapV2Pair {
        function reserves() external view returns (uint112 reserve0, uint112 reserve1);
    }
);


// function that will take in a pool and update the reserves address and return the reserves


#[derive(Serialize, Deserialize)]
struct AddressSet(HashSet<Address>);
fn write_addresses_to_file(addresses: &HashSet<Address>, filename: &str) -> std::io::Result<()> {
    let file = File::create(filename)?;
    let writer = BufWriter::new(file);
    let address_set = AddressSet(addresses.clone());
    serde_json::to_writer(writer, &address_set)?;
    Ok(())
}

fn read_addresses_from_file(filename: &str) -> std::io::Result<HashSet<Address>> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let address_set: AddressSet = serde_json::from_reader(reader)?;
    Ok(address_set.0)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // initializations
    dotenv::dotenv().ok();
    env_logger::Builder::new()
        .filter_level(LevelFilter::Debug)  // or Info, Warn, etc.
        .init();

    // construct the providers
    info!("Constructing providers...");
    let http_url = std::env::var("HTTP").unwrap();
    let ws_url = WsConnect::new(std::env::var("WS").unwrap());
    let http_provider = Arc::new(ProviderBuilder::new().on_http(http_url.parse().unwrap()));
    let ws_provider = Arc::new(ProviderBuilder::new().on_ws(ws_url).await.unwrap());

    // load in all the pools
    info!("Loading pools...");
    let pool_sync = PoolSync::builder()
        .add_pools(&[PoolType::UniswapV2])
        .chain(Chain::Ethereum)
        .rate_limit(100)
        .build()
        .unwrap();

    // sync all the pools and get the top tokens
    info!("Syncing pools...");
    let pools = pool_sync.sync_pools(http_provider.clone()).await.unwrap();
    info!("Loading top volume tokens...");
    let top_volume_tokens = read_addresses_from_file("addresses.json")?;

    // all our mappings
    let mut address_to_pool = ConcurrentPool::new(); // for pool data
    let mut address_to_node: FxHashMap<Address, NodeIndex> = FxHashMap::default(); // for finding node idx
    let mut token_to_edge: FxHashMap<(NodeIndex, NodeIndex), EdgeIndex> = FxHashMap::default();

    // build the graph and populate the mappings
    let graph = Arc::new(build_graph(
        &pools,
        top_volume_tokens,
        &mut address_to_node,
        &mut address_to_pool,
        &mut token_to_edge,
    ));
    address_to_pool.sync_pools(http_provider.clone()).await;

    // rewrap it
    let address_to_pool = Arc::new(address_to_pool);

    // fetch the weth node index
    let node = *address_to_node
        .get(&address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"))
        .unwrap();

    // build all of the cycles
    let cycles = construct_cycles(&graph, node);

    println!("Found {} cycles", cycles.len());

    let (log_sender, mut log_receiver) = mpsc::channel(10);
    // spawn our tasks
    tokio::task::spawn(stream_sync_events(
        ws_provider,
        http_provider,
        address_to_pool.clone(),
        log_sender,
    ));
    tokio::task::spawn(search_paths(
        graph,
        cycles,
        address_to_pool.clone(),
        token_to_edge,
        log_receiver,
    ));
    //tokio::task::spawn(stream_new_blocks(ws_provider.clone()));

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }

    Ok(())
}
