use crate::events::Event;
use crate::pool_manager::PoolManager;
use crate::test::FlashQuoter;
use alloy::primitives::address;
use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::providers::Provider;
use alloy::primitives::{Address, U256};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use crossbeam_queue::SegQueue;
use std::time::{SystemTime, UNIX_EPOCH};
use dashmap::DashMap;
use gweiyser::{Chain, Gweiyser};
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
    pub stable: bool,
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
            PoolType::AlienBase => 10,
        }
    }
}

impl SwapStep {
    pub fn get_amount_out(&self, amount_in: U256, pool_manager: &PoolManager) -> U256 {
        let zero_to_one = pool_manager.zero_to_one(self.token_in, &self.pool_address);
        match self.protocol {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 | PoolType::PancakeSwapV2 | PoolType::BaseSwapV2 => {
                let v2_pool = pool_manager.get_v2pool(&self.pool_address);
                //println!("V2 pool: {:#?}", v2_pool);
                calculate_v2_out(
                    amount_in,
                    v2_pool.token0_reserves,
                    v2_pool.token1_reserves,
                    zero_to_one,
                    self.protocol
                )
            }
            PoolType::UniswapV3 | PoolType::SushiSwapV3 | PoolType::BaseSwapV3 | PoolType::Slipstream | PoolType::PancakeSwapV3 => {
                let mut v3_pool = pool_manager.get_v3pool(&self.pool_address);
                //println!("V3 pool: {:#?}", v3_pool);
                calculate_v3_out(amount_in, &mut v3_pool, zero_to_one).unwrap()
            }
            PoolType::Aerodrome => {
               let v2_pool = pool_manager.get_v2pool(&self.pool_address);
               //println!("V2 pool: {:#?}", v2_pool);
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
        all_paths: &mut Vec<Vec<SwapStep>>, 
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
                            stable: pool.stable(),
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
        let MINIMUM_PROFIT_PERCENTAGE: U256 = U256::from(2) / U256::from(100); // 2% minimum profit

        while let Ok(Event::ReserveUpdate(updated_pools)) = reserve_update_receiver.recv().await {
            info!("Searching for arbs...");
            let start = std::time::Instant::now(); // timer

            let affected_paths: Vec<usize> = updated_pools.iter()
                .flat_map(|pool| self.path_index.get(pool).map(|indices| indices.clone()))
                .flatten()
                .collect();
            //info!("Searching {} paths", affected_paths.len());


            let profitable_paths: Vec<_> = affected_paths.par_iter()
                .filter_map(|&path_index| {
                    let cycle = &self.cycles[path_index];
                    let initial_amount = U256::from(AMOUNT);
                    let mut current_amount = initial_amount;
                    for swap in cycle {
                        current_amount = swap.get_amount_out(current_amount, &self.pool_manager);
                        if current_amount <= U256::from(AMOUNT) {
                            return None;
                        }
                    }

                    let repayment_amount = initial_amount + (initial_amount * FLASH_LOAN_FEE);
                    if current_amount >= repayment_amount {
                        let profit = current_amount - repayment_amount;
                        let profit_percentage = profit * U256::from(10000) / initial_amount;

                        if profit_percentage >= MINIMUM_PROFIT_PERCENTAGE * U256::from(10000) {
                            Some((cycle.clone(), profit))
                        } else {
                            None
                        }
                    } else {
                        None
                    }

                    //if current_amount >= required_amount * PROFIT_THRESHOLD {//* FLASH_LOAN_FEE {
                        //println!("path: {:#?} Current amount: {:#?}", cycle, current_amount);
                        //Some((cycle.clone(), current_amount))
                    //} else  {
                     //   None
                    //}

                })
                .collect();

            //info!("Searched all paths in {:?}", start.elapsed());
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
            info!("done sim at timestamp {}:", now);
            //info!("Found {} profitable paths", profitable_paths.len());
            //simulate_quote(profitable_paths.clone(), U256::from(AMOUNT)).await;
            for path in profitable_paths {
                if let Err(e) = arb_sender.send(Event::NewPath(path.0)) {
                    warn!("Path send failed: {:?}", e);
                }
            }
            /* 
            if !profitable_paths.is_empty() {
                // Find the most profitable path
                let most_profitable_path = profitable_paths.iter()
                    .max_by_key(|&(_, profit)| profit)
                    .unwrap();
                let (most_profitable_cycle, profit) = most_profitable_path;
                if let Err(e) = arb_sender.send(Event::NewPath(most_profitable_cycle.clone())) {
                    warn!("Path send failed: {:?}", e);
                }
            } 
            info!("Searched all paths in {:?}", start.elapsed());


            //info!("Found {} profitable paths", profitable_paths.len());


            for path in profitable_paths {
                if let Err(e) = arb_sender.send(Event::NewPath(path.0)) {
                    warn!("Path send failed: {:?}", e);
                }
            }


            //info!("Found {} profitable paths", profitable_paths.len());

            */
        }
    }
}

pub async fn simulate_quote(swap_steps: Vec<(Vec<SwapStep>, U256)>, amount: U256) {
    info!("running");
    // deploy the quoter
    let url = std::env::var("FULL").unwrap();
    let provider = ProviderBuilder::new().on_http(url.parse().unwrap());
    let fork_block = provider.get_block_number().await.unwrap();
    let anvil = Anvil::new()
        .fork(url)
        .port(9101_u16)
        .fork_block_number(fork_block)
        .try_spawn()
        .unwrap();

    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let wallet = EthereumWallet::from(signer);
    let anvil_signer = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .network::<alloy::network::AnyNetwork>()
            .wallet(wallet)
            .on_http(anvil.endpoint_url()),
    );
    let flash_quoter = FlashQuoter::deploy(anvil_signer.clone()).await.unwrap();
    // give the account some weth and approve the quoter
    let gweiyser = Gweiyser::new(anvil_signer.clone(), Chain::Base);
    let weth = gweiyser.token(address!("4200000000000000000000000000000000000006")).await;
    weth.deposit(amount).await; // deposit into signers account, account[0] here
    weth.transfer_from(anvil.addresses()[0], *flash_quoter.address(), amount).await;
    weth.approve(*flash_quoter.address(), amount).await;


    for path in swap_steps {
        info!("Simulating quote for path ");
        let swap_steps = path.0;
        let calculated_profit = path.1;
        let converted_path: Vec<FlashQuoter::SwapStep> = swap_steps
            .clone()
            .iter()
            .map(|step| FlashQuoter::SwapStep {
                poolAddress: step.pool_address,
                tokenIn: step.token_in,
                tokenOut: step.token_out,
                protocol: step.as_u8(),
                fee: step.fee,
                stable: step.stable,
            })
            .collect();


        match  flash_quoter
            .executeArbitrage(converted_path, amount)
            .call()
            .await {
                Ok(FlashQuoter::executeArbitrageReturn { _0: profit }) => {
                    if profit != calculated_profit {
                        println!("Profit mismatch, path {:#?}, calculated profit: {}, actual profit: {}", swap_steps, calculated_profit, profit);
                    } else {
                        println!("Quote profit: {:?}, Calculated profit: {:?}", profit, calculated_profit);
                    }

                },
                Err(e) => println!("Error simulating quote: {:?}, path: {:#?}", e, swap_steps),
            }

    }

}

