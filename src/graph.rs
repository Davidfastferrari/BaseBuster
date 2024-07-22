use alloy::primitives::Address;
use alloy::primitives::{U128, U256};
use alloy::signers::k256::elliptic_curve::consts::U25;
use petgraph::algo;
use alloy::primitives::address;
use std::time::Instant;
use alloy::providers::ProviderBuilder;
use tokio::sync::broadcast::{Receiver, Sender};
use crate::events::Event;
use rayon::prelude::*;
use std::sync::Arc;
use petgraph::prelude::*;
use pool_sync::Pool;
use pool_sync::PoolInfo;
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use log::info;
use crate::concurrent_pool::ConcurrentPool;
use alloy::sol;
use crate::events::ArbPath;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract UniswapV2Router {
        function getAmountsOut(uint amountIn, address[] memory path) external view returns (uint[] memory amounts);
    }
);

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

#[inline]
pub fn calculate_amount_out(reserves_in: U128, reserves_out: U128, amount_in: U256) -> Option<U256> {
    if reserves_in.is_zero() || reserves_out.is_zero() {
        return None;
    }

    let amount_in_with_fee = amount_in.checked_mul(U256::from(997))?;
    let numerator = amount_in_with_fee.checked_mul(U256::from(reserves_out))?;
    let denominator = U256::from(reserves_in)
        .checked_mul(U256::from(1000))?
        .checked_add(amount_in_with_fee)?;

    if denominator.is_zero() {
        None
    } else {
        numerator.checked_div(denominator)
    }
}

pub async fn search_paths(
    graph: Arc<Graph<Address, Address, Undirected>>, 
    cycles: Vec<Vec<NodeIndex>>,
    address_to_pool: Arc<ConcurrentPool>,
    token_to_edge: Arc<FxHashMap<(NodeIndex, NodeIndex), EdgeIndex>>,
    mut reserve_update_receiver: Receiver<Event>,
    mut tx_sender: Sender<ArbPath>,
) {
    while let Ok(event) = reserve_update_receiver.recv().await {
        info!("Searching for arbs...");
        let start = std::time::Instant::now();
        
        let profitable_paths: Vec<_> = cycles.par_iter()
            .filter_map(|cycle| {
                let mut current_amount = U256::from(1e17 as u64);
                let mut swap_path = vec![graph[cycle[0]]];

                for window in cycle.windows(2) {
                    let (token0, token1) = (window[0], window[1]);
                    let edge = token_to_edge.get(&(token0, token1))?;
                    let pool_addr = graph[*edge];
                    let pool = address_to_pool.get(&pool_addr);
                    let token0_address = graph[token0];
                    let pool_token0 = pool.token0_address();
                    let (reserves0, reserves1) = address_to_pool.get_reserves(&pool_addr);
                    
                    current_amount = if token0_address == pool_token0 {
                        calculate_amount_out(reserves0, reserves1, current_amount)?
                    } else {
                        calculate_amount_out(reserves1, reserves0, current_amount)?
                    };

                    swap_path.push(graph[token1]);
                }

                if current_amount > U256::from(1e17 as u64) {
                    Some((cycle.clone(), current_amount, swap_path))
                } else {
                    None
                }
            })
            .collect();

        info!("Found {} profitable paths in {:?} {:?}", profitable_paths.len(), start.elapsed(), profitable_paths);
        for path in profitable_paths {
            let path = path.2.clone();
            let amount_in = U256::from(1e17 as u64);
            let arb_path = ArbPath { path, amount_in };
            tx_sender.send(arb_path).unwrap();
        }

        // Process profitable paths here...
    }
}