use crate::events::{ArbPath, Event};
use crate::pool_manager::PoolManager;
use alloy::primitives::{Address, U256};
use log::info;
use petgraph::algo;
use petgraph::graph::UnGraph;
use petgraph::prelude::*;
use pool_sync::{Pool, PoolInfo, PoolType};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::calculation::{calculate_v2_out, calculate_v3_out};

// All information we need to look for arbitrage opportunities
pub struct ArbGraph {
    graph: UnGraph<Address, Pool>,
    pool_manager: Arc<PoolManager>,
    cycles: Vec<Vec<SwapStep>>,
}


#[derive(Debug)]
pub struct SwapStep {
    pool_address: Address,
    token_in: Address,
    token_out: Address,
    protocol: PoolType,
}


impl SwapStep {
    pub fn get_amount_out(&self, amount_in: U256, pool_manager: &PoolManager) -> U256 {
        match self.protocol {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 | PoolType::PancakeSwapV2 => {
                let (reserve0, reserve1) = pool_manager.get_v2(&self.pool_address);
                let zero_to_one = pool_manager.zero_to_one(self.token_in, &self.pool_address);
                calculate_v2_out(amount_in, reserve0, reserve1, zero_to_one)
            }
            PoolType::UniswapV3 | PoolType::SushiSwapV3 => {
                let (sqrt_price_x96, tick, liquidity) = pool_manager.get_v3(&self.pool_address);
                let zero_to_one = pool_manager.zero_to_one(self.token_in, &self.pool_address);
                calculate_v3_out(amount_in, sqrt_price_x96, tick, liquidity, zero_to_one).unwrap()
            },
            _=> todo!()
        }
    }
}

impl ArbGraph {
    // Constructor, takes the set of working tokens we are interested in searching over
    pub fn new(pool_manager: Arc<PoolManager>, working_pools: Vec<Pool>, token: Address) -> Self {
        // build the graph
        let graph = ArbGraph::build_graph(working_pools);

        // get start node and construct cycles
        let start_node = graph.node_indices().find(|node| graph[*node] == token).unwrap();
        let cycles = ArbGraph::find_all_arbitrage_paths(&graph, start_node, 4);
        println!("Found {}  paths", cycles.len());
        //println!("Cycles  {:#?}", cycles);

        Self {
            graph,
            pool_manager,
            cycles,
        }
    }

    // Build the graph from the working set of pools
    pub fn build_graph(working_pools: Vec<Pool>) -> UnGraph<Address, Pool> {
        let mut graph: UnGraph<Address, Pool> = UnGraph::new_undirected();
        let mut inserted_nodes: HashSet<Address> = HashSet::new();

        for pool in working_pools {
            // add the nodes ot the graph if they have not already been added
            if !inserted_nodes.contains(&pool.token0_address()) {
                graph.add_node(pool.token0_address());
                inserted_nodes.insert(pool.token0_address());
            }
            if !inserted_nodes.contains(&pool.token1_address()) {
                graph.add_node(pool.token1_address());
                inserted_nodes.insert(pool.token1_address());
            }

            // get the indicies
            let node1 = graph
                .node_indices()
                .find(|node| graph[*node] == pool.token0_address())
                .unwrap();
            let node2 = graph
                .node_indices()
                .find(|node| graph[*node] == pool.token1_address())
                .unwrap();

            // add the edge
            graph.add_edge(node1, node2, pool.clone());
        }
        graph
    }

    fn find_all_arbitrage_paths(
        graph: &UnGraph<Address, Pool>,
        start_node: NodeIndex,
        max_hops: usize,
    ) -> Vec<Vec<SwapStep>> {
        //let mut all_paths = Vec::new();
        let mut all_paths: Vec<Vec<SwapStep>> = Vec::new();
        let mut current_path = Vec::new();
        let mut visited = HashSet::new();

        Self::construct_cycles(
            graph,
            start_node,
            start_node,
            max_hops,
            &mut current_path,
            &mut visited,
            &mut all_paths,
        );

        all_paths
    }

    // Build all of the cycles
    fn construct_cycles(
        graph: &UnGraph<Address, Pool>,
        current_node: NodeIndex,
        start_node: NodeIndex,
        max_hops: usize,
        current_path: &mut Vec<(NodeIndex, Pool, NodeIndex)>,
        visited: &mut HashSet<NodeIndex>,
        all_paths: &mut Vec<Vec<SwapStep>>, // all_paths: &mut Vec<Vec<(Address, Protocol)>
    ) {
        if current_path.len() >= max_hops {
            return;
        }

        for edge in graph.edges(current_node) {
            let next_node = edge.target();
            let protocol = edge.weight().clone();

            if next_node == start_node {
                if current_path.len() >= 2
                    || (current_path.len() == 1
                        && current_path[0].1.pool_type() != protocol.pool_type())
                {
                    let mut new_path = current_path.clone();
                    new_path.push((current_node, protocol, next_node));

                    let mut swap_path = Vec::new();
                    for (base, pool, quote) in new_path.iter() {
                        let swap = SwapStep {
                            pool_address: pool.address().clone(),
                            token_in: graph[*base].clone(),
                            token_out: graph[*quote].clone(),
                            protocol: pool.pool_type().clone(),
                        };
                        swap_path.push(swap);
                    }

                    all_paths.push(swap_path);
                }
            } else if !visited.contains(&next_node) {
                current_path.push((current_node, protocol, next_node));
                visited.insert(next_node);

                Self::construct_cycles(
                    graph,
                    next_node,
                    start_node,
                    max_hops,
                    current_path,
                    visited,
                    all_paths,
                );

                current_path.pop();
                visited.remove(&next_node);
            }
        }
    }

    // Search for paths
    pub async fn search_paths(
        &self,
        arb_sender: Sender<Event>,
        mut reserve_update_receiver: Receiver<Event>,
    ) {
        // Once we have updated the reserves from the new block, we can search for new opportunities
        while (reserve_update_receiver.recv().await).is_ok() {
            info!("Searching for arbs...");
            let start = std::time::Instant::now(); // timer

            // get all the profitable paths
            let profitable_paths: Vec<_> = self
                .cycles
                .par_iter() // parallel iterator
                .filter_map(|cycle| {
                    let mut current_amount = U256::from(1e17);
                    // each element in the cycle represents a swap
                    for swap in cycle {
                        // I want to swap on each step and get the amount out
                        current_amount = swap.get_amount_out(current_amount, &self.pool_manager);
                    }

                    if current_amount > U256::from(1e17 as u64) {
                        Some(cycle)
                    } else {
                        None
                    }
                })
                .collect();
            info!("Searched all paths in {:?}", start.elapsed());
            info!("Found {} profitable paths", profitable_paths.len());

            // send off to the optimizer
            for path in profitable_paths {
                /* 
                let arb_path = ArbPath {
                    path: path.0,
                    reserves: path.1,
                };
                arb_sender.send(Event::NewPath(arb_path)).unwrap();
                */
            }
        }
    }
}

