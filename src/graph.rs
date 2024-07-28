use crate::calculation::calculate_amount_out;
use crate::events::{ArbPath, Event};
use crate::pool_manager::PoolManager;
use alloy::primitives::{Address, U256};
use log::info;
use petgraph::algo;
use petgraph::graph::UnGraph;
use petgraph::prelude::*;
use pool_sync::{Pool, PoolInfo};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};

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
            nodes_to_address,
        }
    }

    // Build the graph from the working set of pools
    pub fn build_graph(
        working_pools: Vec<Pool>,
    ) -> (
        UnGraph<Address, Address>,
        FxHashMap<(NodeIndex, NodeIndex), Address>,
    ) {
        let mut address_to_node = FxHashMap::default();
        let mut nodes_to_address = FxHashMap::default();
        let mut graph: UnGraph<Address, Address> = UnGraph::new_undirected();

        for pool in working_pools {
            let addr0 = pool.token0_address();
            let addr1 = pool.token1_address();
            let pool_type = pool.type();

            let node0 = *address_to_node
                .entry(addr0)
                .or_insert_with(|| graph.add_node(addr0));
            let node1 = *address_to_node
                .entry(addr1)
                .or_insert_with(|| graph.add_node(addr1));

            let _ = graph.add_edge(node0, node1, pool_typek);
            nodes_to_address.insert((node0, node1), pool.address());
            nodes_to_address.insert((node1, node0), pool.address());
        }
        (graph, nodes_to_address)
    }

    // Build all of the cycles
    fn construct_cycles(
        graph: &UnGraph<usize, Protocol>,
        current_node: NodeIndex,
        start_node: NodeIndex,
        max_hops: usize,
        current_path: &mut Vec<(NodeIndex, Protocol, NodeIndex)>,
        visited: &mut HashSet<NodeIndex>,
        all_paths: &mut Vec<Vec<(NodeIndex, Protocol, NodeIndex)>>,
    ) {
        if current_path.len() >= max_hops {
            return;
        }
    
        for edge in graph.edges(current_node) {
            let next_node = edge.target();
            let protocol = *edge.weight();
            
            if next_node == start_node {
                if current_path.len() >= 2 || (current_path.len() == 1 && current_path[0].1 != protocol) {
                    let mut new_path = current_path.clone();
                    new_path.push((current_node, protocol, next_node));
                    all_paths.push(new_path);
                }
            } else if !visited.contains(&next_node) {
                current_path.push((current_node, protocol, next_node));
                visited.insert(next_node);
                
                dfs(
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
                    // the current amount is how much of a token we currently have along the swap path
                    let mut current_amount = U256::from(1e17);
                    let mut path_reserves = Vec::new();

                    // process in steps of 2, represent a pool swap
                    for window in cycle.windows(2) {
                        // extract relevant info
                        let (token0, token1) = (window[0], window[1]); // the two tokens in the swap
                        let address = self.nodes_to_address.get(&(token0, token1)).unwrap(); // the pool that the tokens are in
                        let (reserve0, reserve1) = self.pool_manager.get_reserves(address);
                        path_reserves.push((reserve0, reserve1));

                        // offchain swap simulation
                        let zero_to_one = self.graph[token0] < self.graph[token1];
                        current_amount =
                            calculate_amount_out(reserve0, reserve1, current_amount, zero_to_one)?
                    }

                    // if we have made a profit, return the cycle
                    if current_amount > U256::from(1e17 as u64) {
                        let address_path: Vec<Address> =
                            cycle.iter().map(|node| self.graph[*node]).collect();
                        Some((address_path, path_reserves))
                    } else {
                        None
                    }
                })
                .collect();
            info!("Searched all paths in {:?}", start.elapsed());
            info!("Found {} profitable paths", profitable_paths.len());

            // send off to the optimizer
            for path in profitable_paths {
                let arb_path = ArbPath {
                    path: path.0,
                    reserves: path.1,
                };
                arb_sender.send(Event::NewPath(arb_path)).unwrap();
            }
        }
    }
}

/*
use petgraph::{graph::{NodeIndex, UnGraph}, visit::EdgeRef};
use std::collections::HashSet;
use rand::Rng;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Protocol {
    UniswapV2,
    UniswapV3,
    Curve,
}

fn find_all_arbitrage_paths(
    graph: &UnGraph<usize, Protocol>,
    start_node: NodeIndex,
    max_hops: usize,
) -> Vec<Vec<(NodeIndex, Protocol, NodeIndex)>> {
    let mut all_paths = Vec::new();
    let mut current_path = Vec::new();
    let mut visited = HashSet::new();

    dfs(
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

fn dfs(
    graph: &UnGraph<usize, Protocol>,
    current_node: NodeIndex,
    start_node: NodeIndex,
    max_hops: usize,
    current_path: &mut Vec<(NodeIndex, Protocol, NodeIndex)>,
    visited: &mut HashSet<NodeIndex>,
    all_paths: &mut Vec<Vec<(NodeIndex, Protocol, NodeIndex)>>,
) {
    if current_path.len() >= max_hops {
        return;
    }

    for edge in graph.edges(current_node) {
        let next_node = edge.target();
        let protocol = *edge.weight();
        
        if next_node == start_node {
            if current_path.len() >= 2 || (current_path.len() == 1 && current_path[0].1 != protocol) {
                let mut new_path = current_path.clone();
                new_path.push((current_node, protocol, next_node));
                all_paths.push(new_path);
            }
        } else if !visited.contains(&next_node) {
            current_path.push((current_node, protocol, next_node));
            visited.insert(next_node);
            
            dfs(
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

fn print_paths(paths: &Vec<Vec<(NodeIndex, Protocol, NodeIndex)>>) {
    println!("Total paths found: {}", paths.len());
    for (i, path) in paths.iter().enumerate() {
        print!("Path {}: ", i + 1);
        for (j, (from, protocol, to)) in path.iter().enumerate() {
            print!("({}, {:?}, {})", from.index(), protocol, to.index());
            if j < path.len() - 1 {
                print!(", ");
            }
        }
        println!();
    }
}

#
 */