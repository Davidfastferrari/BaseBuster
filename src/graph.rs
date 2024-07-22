use alloy::primitives::Address;
use alloy::primitives::{U128, U256};
use petgraph::algo;
use alloy::primitives::address;
use std::time::Instant;
use alloy::providers::ProviderBuilder;
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
use alloy::sol;

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

pub async fn search_paths(
    graph: Arc<Graph<Address, Address, Undirected>>, 
    cycles: Vec<Vec<NodeIndex>>,
    address_to_pool: Arc<ConcurrentPool>,
    token_to_edge: FxHashMap<(NodeIndex, NodeIndex), EdgeIndex>,
    mut log_receiver: Receiver<Events>
) {
    info!("Traversing all cycles...");
    let mut successful_paths: Vec<Vec<NodeIndex>> = Vec::new();

    while let Some(event) = log_receiver.recv().await {
        let start = Instant::now();
        
        let futures = cycles.iter().map(|cycle| {
            let graph = graph.clone();
            let address_to_pool = address_to_pool.clone();
            let token_to_edge = token_to_edge.clone();
            
            async move {
                let http_url = std::env::var("HTTP").unwrap();
                let provider = Arc::new(ProviderBuilder::new().on_http(http_url.parse().unwrap()));
                let router = UniswapV2Router::new(address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D"), provider);
                
                let mut current_amount: U256 = U256::from(1e17);
                let mut profitable: Vec<(Pool, U256)> = Vec::new();
                let mut swap_path: Vec<Address> = vec![graph[cycle[0]]];

                for window in cycle.windows(2) {
                    let token0 = window[0];
                    let token1 = window[1];
                    
                    let t1_addr = graph[token1];
                    swap_path.push(t1_addr);
                    
                    let edge = token_to_edge.get(&(token0, token1)).unwrap();
                    let pool_addr = graph[*edge];
                    let pool = address_to_pool.get(&pool_addr);
                    let token0_address = graph[token0];
                    let pool_token0 = pool.token0_address();
                    let (reserves0, reserves1) = address_to_pool.get_reserves(&pool_addr);
                    
                    current_amount = if token0_address == pool_token0 {
                        calculate_amount_out(reserves0, reserves1, current_amount)
                    } else {
                        calculate_amount_out(reserves1, reserves0, current_amount)
                    };
                    
                    profitable.push((pool.clone(), current_amount));
                }



                if current_amount > U256::from(1e17) {
                    let amount_in = U256::from(1e17);
                    let expected_amount_out = router.getAmountsOut(amount_in, swap_path).call().await.unwrap();
                    println!("Found profitable path:");
                    for (pool, amount) in profitable {
                        println!("{:?} {:?} {:?} -> ", pool.address(), pool.reserves(), amount);
                    }
                    println!("Expected amount out: {:?}", expected_amount_out);
                    Some(cycle.clone())
                } else {
                    None
                }
            }
        });

        let results = futures::future::join_all(futures).await;
        successful_paths.extend(results.into_iter().filter_map(|x| x));

        println!("Traversal took {:?}", start.elapsed());
    }

    // Process successful_paths here...
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