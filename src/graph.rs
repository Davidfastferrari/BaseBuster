use alloy::primitives::{address, Address, U128, U256};
use alloy::providers::RootProvider;
use alloy::sol;
use alloy::transports::http::{Client, Http};
use log::info;
use petgraph::algo;
use petgraph::prelude::*;
use pool_sync::{Pool, PoolInfo};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::concurrent_pool::ConcurrentPool;
use crate::events::{ArbPath, Event};

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract UniswapV2Router {
        function getAmountsOut(uint amountIn, address[] memory path) external view returns (uint[] memory amounts);
    }
);

pub struct ArbGraph {
    // The graph that interconnects all trading paths
    graph: Graph<Address, Address, Undirected>,
    // Mapping from two graph nodes to their corresponding pool (edge)
    nodes_to_pool: FxHashMap<(NodeIndex, NodeIndex), Pool>,
}

impl ArbGraph {
    // Constructor
    pub fn new() -> Self {
        Self {
            graph: Graph::new_undirected(),
            nodes_to_pool: FxHashMap::default(),
        }
    }


    pub fn build_graph(
        &mut self,
        top_volume_pools: &Vec<Pool>
    ) {
        // go through all the pools we want to consider
        for pool in &top_volume_pools {
            // extract the addresses, used for node values
            let addr0 = pool.token0_address();
            let addr1 = pool.token1_address();

            // insert the nodes and add an edge between them
            // todo!(), make the edge the pool/identifier??
            let node0 = *address_to_node
                .entry(addr0)
                .or_insert_with(|| self.graph.add_node(addr0));
            let node1 = *address_to_node
                .entry(addr1)
                .or_insert_with(|| self.graph.add_node(addr1));
            let edge_index = self.graph.add_edge(node0, node1, pool.address());
        }

}

pub fn build_graph(
    pools: &Vec<Pool>,
    top_volume_tokens: HashSet<Address>,
    address_to_node: &mut FxHashMap<Address, NodeIndex>,
    address_to_pool: &mut ConcurrentPool,
    token_to_edge: &mut FxHashMap<(NodeIndex, NodeIndex), EdgeIndex>,
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
pub fn calculate_amount_out(
    reserves_in: U128,
    reserves_out: U128,
    amount_in: U256,
) -> Option<U256> {
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
    anvil_provider: Arc<RootProvider<Http<Client>>>,
    address_to_pool: Arc<ConcurrentPool>,
    token_to_edge: Arc<FxHashMap<(NodeIndex, NodeIndex), EdgeIndex>>,
    mut reserve_update_receiver: Receiver<Event>,
    mut tx_sender: Sender<ArbPath>,
) {
    //let contract = UniswapV2Router::new(
        //address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D"),
        //anvil_provider.clone(),
    //);
    


    // Once we have updated the reserves from the new block, we can search for new opportunities
    while let Ok(event) = reserve_update_receiver.recv().await {
        info!("Searching for arbs...");
        let start = std::time::Instant::now();

        // 
        let profitable_paths: Vec<_> = cycles
            .par_iter()
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

                    //let reserves = nodes_to_pool.get(&(token0, token1)).get_reserves();

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

        for path in profitable_paths {
            let call_path = path.2.clone();
            let UniswapV2Router::getAmountsOutReturn { amounts } = contract
                .getAmountsOut(U256::from(1e17), call_path)
                .call()
                .await
                .unwrap();
            println!(
                "Router amounts: {:?}, Calculated amounts: {:?}",
                amounts.last(),
                path.1
            );
            /*

            let token_path = path.2.clone();
            let amount_in = U256::from(1e17 as u64);
            let arb_path = ArbPath { path:token_path, amount_in , expected_out: path.1.clone() };
            tx_sender.send(arb_path);
            */
        }

        // Process profitable paths here...
    }
}

