//use crate::build_graph::{construct_graph, find_best_arbitrage_path};
use alloy::primitives::address;
use alloy::primitives::Address;
use alloy::providers::ProviderBuilder;
use petgraph::algo;
use petgraph::prelude::*;
use pool_sync::filter::filter_top_volume;
use pool_sync::*;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use std::time::Instant;

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
    dotenv::dotenv().ok();
    let provider =
        Arc::new(ProviderBuilder::new().on_http("http://69.67.151.138:8545".parse().unwrap()));
    let pool_sync = PoolSync::builder()
        .add_pools(&[PoolType::UniswapV2])
        .chain(Chain::Ethereum)
        .rate_limit(100)
        .build()
        .unwrap();
    let pools = pool_sync.sync_pools(provider.clone()).await.unwrap();
    let addrs = read_addresses_from_file("addresses.json")?;

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

    let weth_node = *address_to_node
        .get(&address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"))
        .unwrap();

    println!("Finding all simple paths...");
    let start_path_finding = Instant::now();
    let cycles: Vec<Vec<NodeIndex>> =
        algo::all_simple_paths(&graph, weth_node, weth_node, 0, Some(3)).collect();
    println!("Path finding time: {:?}", start_path_finding.elapsed());
    println!("Number of cycles found: {}", cycles.len());

    let graph = Arc::new(graph);
    let filtered_pools = Arc::new(filtered_pools);

    println!("Traversing all cycles in parallel...");
    let start_traversal = Instant::now();

    for i in 0..10 {
        let iteration_start = Instant::now();
        cycles.par_iter().for_each(|cycle| {
            let mut cycle_pools = Vec::with_capacity(3);
            for i in 0..3 {
                let node1 = cycle[i];
                let node2 = cycle[(i + 1) % 3];
                if let Some(edge) = graph.find_edge(node1, node2) {
                    cycle_pools.push(graph[edge]);
                }
            }
            // Here you can do something with cycle_pools if needed
        });
        println!("Iteration {} time: {:?}", i + 1, iteration_start.elapsed());
    }

    println!("Total traversal time: {:?}", start_traversal.elapsed());

    Ok(())
}
