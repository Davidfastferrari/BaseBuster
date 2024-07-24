use alloy::primitives::{address, Address, U128, U256};
use alloy::providers::RootProvider;
use alloy::transports::http::{Client, Http};
use log::info;
use rustc_hash::FxHashSet;
use std::collections::HashSet;
use petgraph::{algo, graph};
use petgraph::graph::UnGraph;
use petgraph::prelude::*;
use pool_sync::{Pool, PoolInfo};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use log::debug;
use tokio::sync::broadcast::{Receiver, Sender};
use crate::pool_manager::PoolManager;
use crate::events::{ArbPath, Event};
use crate::calculation::calculate_amount_out;


// All information we need to look for arbitrage opportunities
pub struct ArbGraph {
    graph: UnGraph<Address, Address>,
    pool_manager: Arc<PoolManager>,
    cycles: Vec<Vec<NodeIndex>>,
    nodes_to_address: FxHashMap<(NodeIndex, NodeIndex), Address>,
}

impl ArbGraph {
    // Constructor, takes the set of working tokens we are interested in searching over
    pub fn new(pool_manager: Arc<PoolManager>, working_pools: Vec<Pool>, token: Address) -> Self {
        // build the graph
        let (graph, nodes_to_address) = ArbGraph::build_graph(working_pools);
        // construct the cycles
        let cycles = ArbGraph::construct_cycles(&graph, token);

        Self {
            graph,
            pool_manager,
            cycles,
            nodes_to_address
        }
    }

    // Build the graph from the working set of pools
    pub fn build_graph(working_pools: Vec<Pool>) -> (UnGraph<Address, Address>, FxHashMap<(NodeIndex, NodeIndex), Address>) {
        let mut address_to_node = FxHashMap::default();
        let mut nodes_to_address = FxHashMap::default();
        let mut graph : UnGraph<Address, Address>= UnGraph::new_undirected();

        for pool in working_pools {
            let addr0 = pool.token0_address();
            let addr1 = pool.token1_address();

            let node0 = *address_to_node
                .entry(addr0)
                .or_insert_with(|| graph.add_node(addr0));
            let node1 = *address_to_node
                .entry(addr1)
                .or_insert_with(|| graph.add_node(addr1));

            let edge = graph.add_edge(node0, node1, pool.address());
            nodes_to_address.insert((node0, node1), pool.address());
            nodes_to_address.insert((node1, node0), pool.address());
        }
        (graph, nodes_to_address)
    }

    // Build all of the cycles
    pub fn construct_cycles(graph: &UnGraph<Address, Address>, token: Address) -> Vec<Vec<NodeIndex>> {
        // get the node index for the token
        let source_index = graph.node_indices().find(|index| graph[*index] == token).unwrap();
        // construct all the cycles
        let cycles: Vec<Vec<NodeIndex>> = algo::all_simple_paths(
            &graph,
             source_index, 
             source_index, 
             0, 
             Some(3)
        ).collect();
        debug!("Found {} cycles", cycles.len());
        cycles
    }

    // Search for paths
    pub async fn search_paths(
        &self,
        mut reserve_update_receiver: Receiver<Event>,
    ) {
        // Once we have updated the reserves from the new block, we can search for new opportunities
        while let Ok(event) = reserve_update_receiver.recv().await {
            info!("Searching for arbs...");
            let start = std::time::Instant::now(); // timer

            // get all the profitable paths
            let profitable_paths: Vec<_> = self.cycles
                .par_iter() // parallel iterator
                .filter_map(|cycle| {
                    // the current amount is how much of a token we currently have along the swap path
                    let mut current_amount = U256::from(1e17);

                    // process in steps of 2, represent a pool swap
                    for window in cycle.windows(2) {
                        // extract relevant info
                        let (token0, token1) = (window[0], window[1]); // the two tokens in the swap
                        let address = self.nodes_to_address.get(&(token0, token1)).unwrap(); // the pool that the tokens are in
                        let (reserve0, reserve1) = self.pool_manager.get_reserves(address);

                        // offchain swap simulation
                        let zero_to_one = self.graph[token0] < self.graph[token1];
                        current_amount =
                            calculate_amount_out(reserve0, reserve1, current_amount, zero_to_one)?
                    }

                    // if we have made a profit, return the cycle
                    if current_amount > U256::from(1e17 as u64) {
                        Some(cycle.clone())
                    } else {
                        None
                    }
                })
                .collect();
            // send off to the optimizer
            println!("Found profitable path: {:?}", profitable_paths);

        }
    }
}

