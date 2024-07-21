use alloy::primitives::Address;
use petgraph::algo;
use std::sync::RwLock;
use std::time::Instant;
use tokio::sync::mpsc::Receiver;
use crate::events::Events;
use rayon::prelude::*;
use std::sync::Arc;
use petgraph::prelude::*;
use pool_sync::Pool;
use pool_sync::PoolInfo;
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use log::info;


use crate::concurrent_pool::ConcurrentPool;

// inital bootstrap of reserves
pub fn bootstrap_reserves(pools: &Vec<Pool>, address_to_pool: &mut ConcurrentPool) {
    // do through all the pools and update the reserves
}

pub fn build_graph(
    pools: &Vec<Pool>,
    top_volume_tokens: HashSet<Address>,
    address_to_node: &mut FxHashMap<Address, NodeIndex>,
    address_to_pool: &mut ConcurrentPool,
    token_to_edge: &mut FxHashMap<(NodeIndex, NodeIndex), EdgeIndex>
) -> Graph<Address, Address, Undirected> {
    // pools contains all of the pools on the entire blockchain,
    // we are just interested inones with trading volume
    // take the insersection fo the pools and the tokens with top volume
    let top_volume_pools: Vec<Pool> = pools
        .clone()
        .into_iter()
        .filter(|pool| {
            top_volume_tokens.contains(&pool.token0_address())
                && top_volume_tokens.contains(&pool.token1_address())
        })
        .collect();

    // graph
    let mut graph: Graph<Address, Address, Undirected> = Graph::new_undirected();

    for pool in &top_volume_pools {
        let addr0 = pool.token0_address();
        let addr1 = pool.token1_address();
        let node0 = *address_to_node
            .entry(addr0)
            .or_insert_with(|| graph.add_node(addr0));
        let node1 = *address_to_node
            .entry(addr1)
            .or_insert_with(|| graph.add_node(addr1));
        let edge_index = graph.add_edge(node0, node1, pool.address());
        token_to_edge.insert((node0, node1), edge_index);
        token_to_edge.insert((node1, node0), edge_index);
        address_to_pool.track_pool(pool.address(), pool.clone());
    }
    graph
}


pub fn construct_cycles(
    graph: &Graph<Address, Address, Undirected>,
    node: NodeIndex,
) -> Vec<Vec<NodeIndex>> {
    algo::all_simple_paths(&graph, node, node, 0, Some(3)).collect()
}

pub async fn search_paths(
    graph: Arc<Graph<Address, Address, Undirected>>, 
    cycles: Vec<Vec<NodeIndex>>,
    address_to_pool: Arc<ConcurrentPool>,
    token_to_edge: FxHashMap<(NodeIndex, NodeIndex), EdgeIndex>,
    mut log_receiver: Receiver<Events>
) {
    info!("Traversing all cycles...");

    // save the successufl paths that will be optimized
    let mut successful_paths: Vec<Vec<NodeIndex>> = Vec::new();

    // when we get a pool reserves update event, we will check all the cycles
    while let Some(event) = log_receiver.recv().await {
        let start = Instant::now();
        // search all the cycles
        cycles.par_iter().for_each(|cycle| {
            // default amount in, 0.1 weth to check if it is profitable
            let mut current_amount = 1e18;

            for window in cycle.windows(2) {
                // get the info we need for the cycle
                let token0 = window[0]; // token0 is the first token in the cycle
                let token1 = window[1]; // token1 is the second token in the cycle
                let edge = token_to_edge.get(&(token0, token1)).unwrap(); // get the edge index
                let pool_addr = graph[*edge]; // get the pool address
                let pool = address_to_pool.get(&pool_addr); // get the pool for token0, tokene

                // uniswapv2 pool have a constant product formula
                // using the reserves and the decimals, calculate the amount out based on the current amount in
                let reserves0 = pool.token0_reserves();
                let reserves1 = pool.token1_reserves();
                let amount_out = calculate_amount_out(reserves0, reserves1, current_amount);
                let current_amount = amount_out;
            }

            // at the end of the cycles, check if the current amount is greater than 0.1weth
            // if it is, then we have found a successful path
            if current_amount > 1e18 {
                successful_paths.push(cycle.clone());
            }
        });
        println!("Traversal took {:?}", start.elapsed());
    }

    // for each path in successful paths, calculate the optimal amount in and then construct a transaction to send
    // then send the transaction
    // then save the path to the database
    // then update the reserves

}


// given a list of profitable paths, optimize the greatest amount in
pub fn optimize_paths(paths: Vec<Vec<NodeIndex>>) {
    // find the greatest amount in
    let mut greatest_amount_in = 0;
    for path in paths {
        let amount_in = calculate_amount_in(path[0], path[1], 1e18);
        if amount_in > greatest_amount_in {
            greatest_amount_in = amount_in;
        }
    }
}

pub fn calculate_amount_in(token0: NodeIndex, token1: NodeIndex, amount_out: u128) -> u128 {
    unimplemented!()
}


// uninswapv2, given the reserves and the amount in calculate the amount out
pub fn calculate_amount_out(reserves0: u128, reserves1: u128, amount_in: u128) -> u128 {
    let amount_in_with_fee = amount_in * 997;
    let numerator = amount_in_with_fee * reserves1;
    let denominator = reserves0 * 1000 + amount_in_with_fee;
    numerator / denominator
}


