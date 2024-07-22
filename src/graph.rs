use alloy::primitives::Address;
use alloy::primitives::{U128, U256};
use petgraph::algo;
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
            // default amount in, 1 weth to check if it is profitable
            let mut current_amount : U256 = U256::from(1e17);
            let mut profitable: Vec<(Pool, U256)> = Vec::new();

            for window in cycle.windows(2) {
                // get the info we need for the cycle
                let token0 = window[0]; // token0 is the first token in the cycle
                let token1 = window[1]; // token1 is the second token in the cycle
                let edge = token_to_edge.get(&(token0, token1)).unwrap(); // get the edge index
                let pool_addr = graph[*edge]; // get the pool address
                let pool = address_to_pool.get(&pool_addr); // get the pool for token0, tokene

                let token0_address = graph[token0];
                let pool_token0 = pool.token0_address();

                let (reserves0, reserves1) = pool.reserves();
                if token0_address == pool_token0 {
                    current_amount = calculate_amount_out(reserves0, reserves1, current_amount);
                } else {
                    current_amount = calculate_amount_out(reserves1, reserves0, current_amount);
                }

                profitable.push((pool.clone(), current_amount));
            }

            // if at the end, the current amount is greater than 0.1wth, then we have found a successful path
            if current_amount > U256::from(1e17) {
                // for each path, pretty print the pool addrress and the reserves and then an arrow to the next pool
                println!("Found profitable path:");
                for (pool, amount) in profitable {
                    println!("{:?} {:?} {:?} -> ", pool.address(), pool.reserves(), format_eth(amount));
                }
            }
        });
        println!("Traversal took {:?}", start.elapsed());
    }

    // for each path in successful paths, calculate the optimal amount in and then construct a transaction to send
    // then send the transaction
    // then save the path to the database
    // then update the reserves

}

fn format_eth(wei: U256) -> String {
    let wei_str = wei.to_string();
    let eth_value = if wei_str.len() <= 18 {
        format!("0.{:0>18}", wei_str)
    } else {
        format!("{}.{:0>18}", &wei_str[..wei_str.len()-18], &wei_str[wei_str.len()-18..])
    };
    // Trim trailing zeros after the decimal point
    eth_value.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn calculate_amount_out(reserves0: U128, reserves1: U128, amount_in: U256) -> U256 {
    let amount_in_with_fee = match amount_in.checked_mul(U256::from(997)) {
        Some(val) => val,
        None => {
            //println!("Warning: Overflow in fee calculation. amount_in: {}", amount_in);
            return U256::ZERO;
        }
    };

    let reserves1_u256 = U256::from(reserves1);
    let numerator = match amount_in_with_fee.checked_mul(reserves1_u256) {
        Some(val) => val,
        None => {
            //println!("Warning: Overflow in numerator calculation. amount_in_with_fee: {}, reserves1: {}", amount_in_with_fee, reserves1);
            return U256::ZERO;
        }
    };

    let reserves0_u256 = U256::from(reserves0);
    let denominator = match reserves0_u256.checked_mul(U256::from(1000)) {
        Some(val) => match val.checked_add(amount_in_with_fee) {
            Some(sum) => sum,
            None => {
                //println!("Warning: Overflow in denominator addition. reserves0 * 1000: {}, amount_in_with_fee: {}", val, amount_in_with_fee);
                return U256::ZERO;
            }
        },
        None => {
            //println!("Warning: Overflow in denominator multiplication. reserves0: {}", reserves0);
            return U256::ZERO;
        }
    };

    match numerator.checked_div(denominator) {
        Some(amount_out) => amount_out,
        None => {
            //println!("Warning: Division by zero or overflow. numerator: {}, denominator: {}", numerator, denominator);
            U256::ZERO
        }
    }
}