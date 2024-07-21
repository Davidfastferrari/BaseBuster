//use crate::build_graph::{construct_graph, find_best_arbitrage_path};
use alloy::primitives::address;
use alloy::primitives::Address;
use alloy::providers::ProviderBuilder;
use env_logger;
use std::sync::RwLock;
use petgraph::algo;
use petgraph::prelude::*;
use pool_sync::filter::filter_top_volume;
use pool_sync::*;
use tokio::sync::mpsc;
use alloy::providers::WsConnect;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use std::time::Instant;
use crate::graph::*;
use stream::*;
use crate::concurrent_pool::ConcurrentPool;
use crate::events::Events;

mod graph;
mod events;
mod concurrent_pool;
mod stream;

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
    env_logger::builder().default_format().build();

    // construct the providers
    let http_url = std::env::var("HTTP").unwrap();
    let ws_url = WsConnect::new(std::env::var("WS").unwrap());
    let http_provider = Arc::new(ProviderBuilder::new().on_http(http_url.parse().unwrap()));
    let ws_provider = Arc::new(ProviderBuilder::new().on_ws(ws_url).await.unwrap());


    // load in all the pools
    let pool_sync = PoolSync::builder()
        .add_pools(&[PoolType::UniswapV2])
        .chain(Chain::Ethereum)
        .rate_limit(100)
        .build()
        .unwrap();

    // sync all the pools and get the top tokens
    let pools = pool_sync.sync_pools(http_provider.clone()).await.unwrap();
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
        &mut token_to_edge
    ));


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
    tokio::task::spawn(stream_blocks(ws_provider, address_to_pool.clone(), log_sender));
    tokio::task::spawn(search_paths(graph, cycles, address_to_pool.clone(), token_to_edge, log_receiver));

    loop {

    }

    Ok(())
}


