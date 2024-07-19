use alloy::primitives::address;
use alloy::primitives::Address;
use alloy::providers::ProviderBuilder;
use petgraph::dot::{Config, Dot};
use petgraph::{algo, prelude::*};
use pool_sync::filter::filter_top_volume;
use pool_sync::*;
use rand::Rng;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use crate::build_graph::{construct_graph, find_best_arbitrage_path};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let provider = ProviderBuilder::new().on_http("http://69.67.151.138:8545".parse().unwrap());
    let provider = Arc::new(provider);

    let pool_sync = PoolSync::builder()
        .add_pools(&[PoolType::UniswapV2])
        .chain(Chain::Ethereum)
        .rate_limit(100)
        .build()
        .unwrap();

    let pools = pool_sync.sync_pools(provider.clone()).await.unwrap();
    let filtered_pools = filter_top_volume(&pools, 2000).await.unwrap();
    let mut token_to_node: HashMap<Address, NodeIndex> = HashMap::new();
    let graph = construct_graph(&pools, &filtered_pools, &mut token_to_node);

    println!("Number of nodes (tokens): {}", graph.node_count());
    println!("Number of edges (pools): {}", graph.edge_count());

    let weth_address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let input_amount = 1_000_000_000_000_000_000u128; // 1 WETH (18 decimals)
    let max_depth = 3;

    if let Some((path, output_amount)) = find_best_arbitrage_path(&graph, weth_address, input_amount, max_depth) {
        println!("Best arbitrage opportunity:");
        println!("Path: {:?}", path.iter().map(|&node| graph[node]).collect::<Vec<_>>());
        println!("Input amount: {} WETH", input_amount as f64 / 1e18);
        println!("Output amount: {} WETH", output_amount as f64 / 1e18);
        let profit_percentage = (output_amount as f64 / input_amount as f64 - 1.0) * 100.0;
        println!("Profit: {:.2}%", profit_percentage);
    } else {
        println!("No profitable arbitrage opportunity found.");
    }
}