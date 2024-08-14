use crate::events::Event;
use crate::pool_manager::PoolManager;
use alloy::primitives::{Address, U256};
use crossbeam_queue::SegQueue;
use dashmap::DashMap;
use log::{info, warn};
use petgraph::graph::UnGraph;
use petgraph::prelude::*;
use pool_sync::{Pool, PoolInfo, PoolType};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::calculation::{calculate_v2_out, calculate_v3_out, calculate_aerodrome_out};
use crate::AMOUNT;

// All information we need to look for arbitrage opportunities
pub struct ArbGraph {
    graph: UnGraph<Address, Pool>,
    pool_manager: Arc<PoolManager>,
    cycles: Vec<Vec<SwapStep>>,
    pools_to_paths: FxHashMap<Address, HashSet<usize>>,
    path_index: DashMap<Address, Vec<usize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapStep {
    pub pool_address: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub protocol: PoolType,
    pub fee: u32,
}

impl SwapStep {
    pub fn as_u8(&self) -> u8 {
        match self.protocol {
            PoolType::UniswapV2 => 0,
            PoolType::SushiSwapV2 => 1,
            PoolType::PancakeSwapV2 => 2,
            PoolType::BaseSwapV2 => 3,
            PoolType::UniswapV3 => 4,
            PoolType::PancakeSwapV3 => 5,
            PoolType::SushiSwapV3 => 6,
            PoolType::BaseSwapV3 => 7,
            PoolType::Slipstream => 8,
            PoolType::Aerodrome => 9,
        }
    }
}

impl SwapStep {
    pub fn get_amount_out(&self, amount_in: U256, pool_manager: &PoolManager) -> U256 {
        let zero_to_one = pool_manager.zero_to_one(self.token_in, &self.pool_address);
        match self.protocol {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 | PoolType::PancakeSwapV2 | PoolType::BaseSwapV2 => {
                let v2_pool = pool_manager.get_v2pool(&self.pool_address);
                calculate_v2_out(
                    amount_in,
                    v2_pool.token0_reserves,
                    v2_pool.token1_reserves,
                    zero_to_one,
                    self.protocol
                )
            }
            PoolType::UniswapV3 | PoolType::SushiSwapV3 | PoolType::BaseSwapV3=> {
                let mut v3_pool = pool_manager.get_v3pool(&self.pool_address);
                calculate_v3_out(amount_in, &mut v3_pool, zero_to_one).unwrap()
            }
            PoolType::Aerodrome => {
               let v2_pool = pool_manager.get_v2pool(&self.pool_address);
               calculate_aerodrome_out(amount_in, self.token_in, &v2_pool)
            }
            _ => todo!(),
        }
    }
}

impl ArbGraph {
    // Constructor, takes the set of working tokens we are interested in searching over
    pub fn new(pool_manager: Arc<PoolManager>, working_pools: Vec<Pool>, token: Address) -> Self {
        // build the graph
        let graph = ArbGraph::build_graph(working_pools);

        // get start node and construct cycles
        let start_node = graph
            .node_indices()
            .find(|node| graph[*node] == token)
            .unwrap();
        let cycles = ArbGraph::find_all_arbitrage_paths(&graph, start_node, 4);
        info!("Found {}  paths", cycles.len());
        let mut pools_to_paths = FxHashMap::default();
        let path_index = DashMap::new(); 
        for (index, cycle) in cycles.iter().enumerate() {
            for step in cycle {
                path_index.entry(step.pool_address).or_insert_with(Vec::new).push(index);
                /* 
                pools_to_paths
                    .entry(step.pool_address)
                    .or_insert_with(HashSet::new)
                    .insert(index);
                */
            }
        }

        Self {
            graph,
            pool_manager,
            cycles,
            pools_to_paths,
            path_index
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

        // write the paths to a file

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
                            fee: pool.fee(),
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
        let FLASH_LOAN_FEE: U256 = U256::from(9) / U256::from(10000); // 0.09% flash loan fee
        let GAS_ESTIMATE: U256 = U256::from(400_000); // Estimated gas used
        let MIN_PROFIT_WEI: U256 = U256::from(1e15); // Minimum profit in wei (0.001 ETH)
        while let Ok(Event::ReserveUpdate(updated_pools)) = reserve_update_receiver.recv().await {
            info!("Searching for arbs...");
            let start = std::time::Instant::now(); // timer

            let affected_paths: Vec<usize> = updated_pools.iter()
                .flat_map(|pool| self.path_index.get(pool).map(|indices| indices.clone()))
                .flatten()
                .collect();
            info!("Searching {} paths", affected_paths.len());


            let profitable_paths: Vec<_> = affected_paths.par_iter()
                .filter_map(|&path_index| {
                    let cycle = &self.cycles[path_index];
                    let mut current_amount = U256::from(AMOUNT);
                    //println!("cycle: {:#?}", cycle);
                    for swap in cycle {
                        current_amount = swap.get_amount_out(current_amount, &self.pool_manager);
                        if current_amount <= U256::from(AMOUNT) {
                            return None;
                        }
                    }

                    if current_amount >= U256::from(AMOUNT) * FLASH_LOAN_FEE {
                        //println!("path: {:#?} Current amount: {:#?}", cycle, current_amount);
                        Some((cycle.clone(), current_amount))
                    } else  {
                        None
                    }
                })
                .collect();

            info!("Searched all paths in {:?}", start.elapsed());
            info!("Found {} profitable paths", profitable_paths.len());

            for path in profitable_paths {
                if let Err(e) = arb_sender.send(Event::NewPath(path)) {
                    warn!("Path send failed: {:?}", e);
                }
            }


            //info!("Found {} profitable paths", profitable_paths.len());

        }
    }
}