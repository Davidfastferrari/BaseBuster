use alloy::primitives::Address;
use petgraph::algo;
use std::sync::RwLock;
use rayon::prelude::*;
use petgraph::prelude::*;
use pool_sync::Pool;
use pool_sync::PoolInfo;
use rustc_hash::FxHashMap;
use std::collections::HashSet;


use crate::concurrent_pool::ConcurrentPool;


pub fn build_graph(
    pools: &Vec<Pool>,
    top_volume_tokens: HashSet<Address>,
    address_to_node: &mut FxHashMap<Address, NodeIndex>,
    index_to_pool: &mut FxHashMap<EdgeIndex, RwLock<Pool>>,
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
    }
    graph
}

pub fn construct_cycles(
    graph: &Graph<Address, Address, Undirected>,
    node: NodeIndex,
) -> Vec<Vec<NodeIndex>> {
    algo::all_simple_paths(&graph, node, node, 0, Some(3)).collect()
}

pub fn search_paths(
    graph: Graph<Address, Address, Undirected>, 
    cycles: Vec<Vec<NodeIndex>>,
    address_to_pool: &ConcurrentPool
) {
    // search all the cycles
    cycles.par_iter().for_each(|cycle| {
        // go though the elements in pairs
        for window in cycle.windows(2) {
            // get the info we need
            let token0 = window[0];
            let token1 = window[1];
            let pool_address = graph[graph.find_edge(token0, token1).unwrap()];

        }

    });
}