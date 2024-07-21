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

mod graph;
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

    // concurrent pool mapping
    let address_to_pool = Arc::new(ConcurrentPool::new());

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

    // mapping from addresses to node indexes in the grpah
    let mut address_to_node: FxHashMap<Address, NodeIndex> = FxHashMap::default();
    let mut index_to_pool: FxHashMap<EdgeIndex, RwLock<Pool>> = FxHashMap::default();
    let mut token_to_edge: FxHashMap<(NodeIndex, NodeIndex), EdgeIndex> = FxHashMap::default();
    let graph = build_graph(
        &pools, 
        top_volume_tokens, 
        &mut address_to_node, 
        &mut index_to_pool,
        &mut token_to_edge
    );


    // fetch the weth node index
    let node = *address_to_node
        .get(&address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"))
        .unwrap();

    // build all of the cycles
    let cycles = construct_cycles(&graph, node);
    println!("Found {} cycles", cycles.len());




    let graph = Arc::new(graph);
    println!("Traversing all cycles in parallel...");
    let start_traversal = Instant::now();

    for i in 0..10 {
        let iteration_start = Instant::now();
        cycles.par_iter().for_each(|cycle| {
            // state before the calc
            for window in cycle.windows(2) {
                let node1 = window[0];
                let node2 = window[1];
                let edge = token_to_edge.get(&(node1, node2));
                //let edge = graph.find_edge(node1, node2).unwrap();
                //let res = graph[edge];

                //let pool = edge_to_pool.read().unwrap();
                // we have the pool here, we can do some the calulactions
                //let pool = pool.get(&res);

            }
            // Here you can do something with cycle_pools if needed
        });
        println!("Iteration {} time: {:?}", i + 1, iteration_start.elapsed());
    }

    println!("Total traversal time: {:?}", start_traversal.elapsed());


    //
    // start the block stream
    //tokio::task::spawn(stream_blocks(ws, tracked_pool.clone()));

    loop {
    }

    Ok(())
}




































    /*
    println!("Filtering pools...");
    let start_filtering = Instant::now();
    let filtered_pools: Vec<Pool> = pools
        .into_iter()
        .filter(|pool| {
            addrs.contains(&pool.token0_address()) && addrs.contains(&pool.token1_address())
        })
        .collect();
    println!("Pool filtering time: {:?}", start_filtering.elapsed());

    println!("Constructing graph...");
    let start_graph_construction = Instant::now();
    let mut graph = Graph::new_undirected();
    let mut address_to_node: FxHashMap<Address, NodeIndex> = FxHashMap::default();
    for pool in &filtered_pools {
        let addr0 = pool.token0_address();
        let addr1 = pool.token1_address();
        let node0 = *address_to_node
            .entry(addr0)
            .or_insert_with(|| graph.add_node(addr0));
        let node1 = *address_to_node
            .entry(addr1)
            .or_insert_with(|| graph.add_node(addr1));
        graph.add_edge(node0, node1, pool.address());
    }
    println!(
        "Graph construction time: {:?}",
        start_graph_construction.elapsed()
    );
    */

    /*
    println!("Finding all simple paths...");
    let start_path_finding = Instant::now();
    let cycles: Vec<Vec<NodeIndex>> =
        algo::all_simple_paths(&graph, weth_node, weth_node, 0, Some(3)).collect();
    println!("Path finding time: {:?}", start_path_finding.elapsed());
    */

