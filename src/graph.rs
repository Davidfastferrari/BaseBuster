use crate::events::{ArbPath, Event};
use crate::pool_manager::PoolManager;
use alloy::primitives::{Address, U256};
use pool_sync::pools::pool_structure::{UniswapV2Pool, UniswapV3Pool};
use log::info;
use petgraph::algo;
use petgraph::graph::UnGraph;
use petgraph::prelude::*;
use pool_sync::{Pool, PoolInfo, PoolType};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use crossbeam_queue::SegQueue;

use crate::calculation::{calculate_v2_out, calculate_v3_out};
use crate::FlashSwap;

// All information we need to look for arbitrage opportunities
pub struct ArbGraph {
    graph: UnGraph<Address, Pool>,
    pool_manager: Arc<PoolManager>,
    cycles: Vec<Vec<SwapStep>>,
    pools_to_paths: HashMap<Address, HashSet<usize>>,
}

#[derive(Debug, Clone)]
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
            PoolType::UniswapV3 => 3,
            PoolType::SushiSwapV3 => 4,
            _=> panic!("Unsupported protocol")
        }
    }
}

impl SwapStep {
    pub fn get_amount_out(&self, amount_in: U256, pool_manager: &PoolManager) -> U256 {
        let zero_to_one = pool_manager.zero_to_one(self.token_in, &self.pool_address);
        match self.protocol {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 | PoolType::PancakeSwapV2 => {
                let v2_pool: UniswapV2Pool = pool_manager.get_v2pool(&self.pool_address);
                calculate_v2_out(
                    amount_in, 
                    v2_pool.token0_reserves, 
                    v2_pool.token1_reserves, 
                    zero_to_one
            )
            }
            PoolType::UniswapV3 | PoolType::SushiSwapV3 => {
                let v3_pool = pool_manager.get_v3pool(&self.pool_address);
                calculate_v3_out(amount_in, v3_pool, zero_to_one).unwrap()
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
        println!("Found {}  paths", cycles.len());
        //println!("Cycles  {:#?}", cycles);

        let mut pools_to_paths = HashMap::new();
        for (index, cycle) in cycles.iter().enumerate() {
            for step in cycle {
                pools_to_paths.entry(step.pool_address).or_insert_with(HashSet::new).insert(index);
            }
        }

        Self {
            graph,
            pool_manager,
            cycles,
            pools_to_paths
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
        while let Ok(Event::ReserveUpdate(updated_pools)) = reserve_update_receiver.recv().await {
            info!("Searching for arbs...");
            let start = std::time::Instant::now(); // timer


            let affected_paths: HashSet<usize> = updated_pools.iter()
                .flat_map(|pool| self.pools_to_paths.get(pool).cloned().unwrap_or_default())
                .collect();

            let profitable_paths = Arc::new(SegQueue::new());

            affected_paths.into_par_iter().for_each(|path_index| {
                let cycle = &self.cycles[path_index];
                let mut current_amount = U256::from(25e15);
                for swap in cycle {
                    current_amount = swap.get_amount_out(current_amount, &self.pool_manager);
                }


                if current_amount > U256::from(26e15) {
                    profitable_paths.push((cycle.clone(), current_amount));
                }
            });


            // get all the profitable paths
            info!("Searched all paths in {:?}", start.elapsed());
            info!("Found {} profitable paths", profitable_paths.len());
            //info!("Profitable paths {:#?}", profitable_paths);

            //info!("Profitable paths {:#?}", profitable_paths);

            // get the cycle with the highest profit
            if profitable_paths.len() != 0 {
                let mut best_path = None;
                let mut max_profit = U256::ZERO;
        
                while let Some(path) = profitable_paths.pop() {
                    //println!{"Path {:#?}", path};
                    //println!("Path {:#?}", path);
                    match arb_sender.send(Event::NewPath(path.0.clone())) {
                        Err(e) => info!("Path send failed: {:?}", e),
                        _ => {}
                    }
                    if path.1 > max_profit {
                        max_profit = path.1;
                        best_path = Some(path);
                    }
                }
                // send off to the optimizer
                let best_path = best_path.unwrap();
                //println!("Path with highest profit: {:#?}, profit: {:?}", best_path.0, best_path.1);

            }

        }
    }


}



#[cfg(test)]
mod tests {
    use super::*;
    use alloy::network::EthereumWallet;
    use alloy::primitives::{Address, address};
    use alloy::providers::{Provider, ProviderBuilder, RootProvider};
    use alloy::signers::k256::elliptic_curve::consts::U25;
    use alloy::signers::local::PrivateKeySigner;
    use alloy::node_bindings::Anvil;
    use alloy::rpc::types::trace::geth::GethDebugTracingOptions;
    use alloy::rpc::types::trace::geth::{
        CallConfig, CallFrame, GethDebugTracerConfig, GethDebugTracerType, GethDebugTracingCallOptions,
        GethDefaultTracingOptions, GethTrace,
    };
    use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer;
    use alloy::transports::http::{Client, Http};
    use alloy_sol_types::{SolCall, SolEvent};
    use alloy::primitives::U256;
    use pool_sync::PoolSync;
    use crate::graph::SwapStep;
    use crate::pool_manager;
    use serde_json::json;
    use alloy::network::Ethereum;
    use FlashSwap::FlashSwapInstance;
    use alloy::providers::ext::DebugApi;
    use gweiyser::{Gweiyser, Chain};
    use gweiyser::addresses::tokens::base_tokens::WETH;


    #[tokio::test]
    pub async fn multi() {
        /* 
        let steps = vec![
            SwapStep {
                pool_address: address!("88980fa24d6c628382a80a5514870e62ed9beb58"),
                token_in: address!("4200000000000000000000000000000000000006"),
                token_out: address!("6dba065721435cfca05caa508f3316b637861373"),
                protocol: PoolType::UniswapV3,
                fee: 10000,
            },
            SwapStep {
                pool_address: address!("d2381c14465927464b9652dd9c51d602c5998d38"),
                token_in: address!("6dba065721435cfca05caa508f3316b637861373"),
                token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
                protocol: PoolType::UniswapV3,
                fee: 10000,
            },
            SwapStep {
                pool_address: address!("c5accd3e4f6df1912498775807e0972b3ec43f29"),
                token_in: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
                token_out: address!("d9aaec86b65d86f6a7b5b1b0c42ffa531710b6ca"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("e902ef54e437967c8b37d30e80ff887955c90db6"),
                token_in: address!("d9aaec86b65d86f6a7b5b1b0c42ffa531710b6ca"),
                token_out: address!("4200000000000000000000000000000000000006"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
        ];


        let pools = load_pools().await;
        println!("got here");
        let pool_manager = pool_manager::PoolManager::new(pools).await;

        println!("blah");
        let calculate_profit = calculate_profit(steps.clone(), pool_manager);
        println!("Calculated profit: {:?}", calculate_profit);
        let simulate_profit = simulate_profit(steps.clone()).await;

        let pool_addr = address!("88980fa24d6c628382a80a5514870e62ed9beb58");

        let provider = Arc::new(ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap()));
        let gweiyser = Gweiyser::new(provider.clone(), Chain::Base);
        let pool = gweiyser.uniswap_v3_pool(pool_addr).await;
        let out = pool.get_amount_out(WETH, U256::from(5e16)).unwrap();
        println!("Out: {:?}", out);


        //println!("Calculated profit: {:?}", calculate_profit);
        //println!("Simulated profit: {:?}", simulate_profit);
    }

    // Calculates the expected profit based off of the swap steps
    fn calculate_profit(steps: Vec<SwapStep>, pool_manager: Arc<PoolManager>) -> U256 {
        let mut amount = U256::from(5e16);
        for step in steps {
            println!("step: {:?}", step);
            amount = step.get_amount_out(amount, &pool_manager);
        }
        amount
        */

    }

    // simulated profit based off of the call to the contract
    async fn simulate_profit(steps: Vec<SwapStep>) -> Option<U256> {

        let steps = build_from_steps(steps);

        dotenv::dotenv().ok();

        let url = std::env::var("FULL").unwrap();
        let provider = ProviderBuilder::new().on_http(url.parse().unwrap());
        let fork_block = provider.get_block_number().await.unwrap();

        let anvil = Anvil::new()
            .fork(url)
            .port(9100_u16)
            .fork_block_number(fork_block)
            .try_spawn()
            .unwrap();
        let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
        let wallet =  EthereumWallet::from(signer);
        let anvil_signer = Arc::new(
            ProviderBuilder::new()
                .with_recommended_fillers()
                .network::<alloy::network::AnyNetwork>()
                .wallet(wallet)
                .on_http(anvil.endpoint_url()),
        );

        let flash_contract = FlashSwap::deploy(anvil_signer.clone()).await.unwrap();
        let flash_address = flash_contract.address();
        let options = get_tracing_options();


        let provider = Arc::new(ProviderBuilder::new().on_http("http://localhost:9100".parse().unwrap()));
        let contract = FlashSwap::new(*flash_address, provider.clone());

        //println!("{:?}", FlashSwap::executeArbitrageCall::SELECTOR);
        let tx = contract.executeArbitrage(steps, U256::from(5e16)).from(anvil.addresses()[0]).into_transaction_request();
        let output = provider.debug_trace_call(tx, alloy::eips::BlockNumberOrTag::Latest, options.clone()).await.unwrap();
        println!("Output: {:#?}", output);
        match output {
            GethTrace::CallTracer(call_trace) => {
                //let output = process_output(&call_trace);

                return Some(U256::from(5e16)); // this is not right
            }
            _ => return None
        }
    }


    // loads in all of the pools 
    pub async fn load_pools() -> Vec<Pool> {
        dotenv::dotenv().ok();
        let url = std::env::var("FULL").unwrap();

        let pool_sync = PoolSync::builder()
            .add_pools(&[
                PoolType::UniswapV2,
                //PoolType::SushiSwapV2,
                //PoolType::PancakeSwapV2,
                PoolType::UniswapV3,
                //PoolType::SushiSwapV3,
            ])
            .chain(pool_sync::Chain::Base)
            .rate_limit(100)
            .build()
            .unwrap();
        let pools = pool_sync.sync_pools().await.unwrap(); 
        pools
    }

    // convert from our swap steps to the flash swap steps
    fn build_from_steps(steps: Vec<SwapStep>) -> Vec<FlashSwap::SwapStep> {
        let mut res = Vec::new();
        for step in steps {
            let flash_step = FlashSwap::SwapStep {
                poolAddress: step.pool_address,
                tokenIn: step.token_in,
                tokenOut: step.token_out,
                protocol: step.protocol as u8,
                fee: step.fee,
            };
            res.push(flash_step);
        }
        res
    }


    // just return the tracing options
    fn get_tracing_options() -> GethDebugTracingCallOptions {
        let options = GethDebugTracingCallOptions {
            tracing_options: GethDebugTracingOptions {
                config: GethDefaultTracingOptions {
                    disable_memory: Some(true),
                    disable_stack: Some(true),
                    disable_storage: Some(true),
                    debug: Some(true),
                    disable_return_data: Some(true),
                    ..Default::default()
                },
                tracer: Some(GethDebugTracerType::BuiltInTracer(CallTracer)),
                tracer_config: GethDebugTracerConfig(serde_json::to_value(
                    CallConfig { 
                        only_top_call: Some(false),
                        with_log: Some(true),
                    }
                ).unwrap().into()),
                timeout: None,
                ..Default::default()
            },
            state_overrides: None,
            block_overrides: None,
        };
        options
    }
}
  /* 
        let anvil = Anvil::new().fork(url).port(8500_u16).try_spawn().unwrap();
        let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
        println!("{:?}", anvil.endpoint());
        let wallet = EthereumWallet::new(signer);
        let provider = Arc::new(
            ProviderBuilder::new()
                .with_recommended_fillers()
                .wallet(wallet)
                .on_http("http://localhost:8500".parse().unwrap()),
        );
    
    

        let gweiyser = Gweiyser::new(provider.clone(), Chain::Base);
        // give the account some weth
        let weth = gweiyser.token(WETH).await;
        weth.deposit(ONE_ETH).await;
        weth.approve(QUOTER, U256::from(5e18)).await;
    

        let start = address!("4200000000000000000000000000000000000006");
        let end = address!("6dba065721435cfca05caa508f3316b637861373");
        let pool_addr = address!("88980fa24d6c628382a80a5514870e62ed9beb58");
        let step_test = steps[0].clone();


        
        let mut manager = PoolManager::default();
        let pool_sync_pool = pools.into_iter().find(|pool| pool.address() == pool_addr).unwrap();
        println!("Pool: {:?}", pool_sync_pool);
        match pool_sync_pool {
            Pool::SushiSwapV3(p) => {
                let v3_state = UniswapV3PoolState {
                    address: p.address,
                    liquidity: p.liquidity.to::<u128>(),
                    sqrt_price: p.sqrt_price,
                    tick: p.tick,
                    fee: p.fee,
                    tick_spacing: p.tick_spacing,
                    tick_bitmap: p.tick_bitmap,
                    ticks: p.ticks,
                };
                manager.insert_v3_pool_state(p.address, v3_state);

            }
            _ => panic!("Not a v3 pool"),
        }

        let pool = gweiyser.uniswap_v3_pool(pool_addr).await;

        //println!("Pool: {:#?}", pool);

        let out = pool.get_amount_out(WETH, U256::from(5e16)).unwrap();
        println!("Out: {:?}", out);
        println!("pool: {:?}", pool);


        let quoter = gweiyser.uniswap_v3_quoter();
        let expected = quoter.quote_exact_input_single(U256::from(5e16), start, end, 10000).await;
        println!("Expected: {:?}", expected);

        let res = step_test.get_amount_out(U256::from(5e16), &manager);
        println!("Amount out: {:?}", res);

        */
    