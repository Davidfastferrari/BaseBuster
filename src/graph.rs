use crate::events::{ArbPath, Event};
use crate::pool_manager::PoolManager;
use alloy::primitives::{Address, U256};
use log::info;
use petgraph::algo;
use petgraph::graph::UnGraph;
use petgraph::prelude::*;
use pool_sync::snapshot::{UniswapV2PoolState, UniswapV3PoolState};
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
                let v2_pool: UniswapV2PoolState = pool_manager.get_v2pool(&self.pool_address);
                calculate_v2_out(amount_in, v2_pool, zero_to_one)
            }
            PoolType::UniswapV3 | PoolType::SushiSwapV3 => {
                let v3_pool: UniswapV3PoolState = pool_manager.get_v3pool(&self.pool_address);
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
            info!("Searching {} paths", affected_paths.len());

            let profitable_paths = Arc::new(SegQueue::new());

            affected_paths.into_par_iter().for_each(|path_index| {
                let cycle = &self.cycles[path_index];
                let mut current_amount = U256::from(5e16);
                info!("Cycle {:#?}", cycle);
                for swap in cycle {
                    current_amount = swap.get_amount_out(current_amount, &self.pool_manager);
                }

                info!("Current amount: {:?}", current_amount);
                info!("{:#?}", U256::from(5e16));

                if current_amount > U256::from(5e16) {
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
                    if path.1 > max_profit {
                        max_profit = path.1;
                        best_path = Some(path);
                    }
                }
                // send off to the optimizer
                let best_path = best_path.unwrap();
                println!("Path with highest profit: {:#?}, profit: {:?}", best_path.0, best_path.1);
                arb_sender.send(Event::NewPath(best_path.0.clone())).unwrap();

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
    use alloy_sol_types::SolEvent;
    use alloy::primitives::U256;
    use crate::graph::SwapStep;
    use serde_json::json;
    use alloy::network::Ethereum;
    use FlashSwap::FlashSwapInstance;
    use alloy::providers::ext::DebugApi;
    use alloy::sol;


    #[tokio::test]
    pub async fn test_multi_swap() {
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



        //let calculate_profit = calculate_profit(steps.clone()).await;
        let simulate_profit = simulate_profit(steps.clone()).await;

        //println!("Calculated profit: {:?}", calculate_profit);
        println!("Simulated profit: {:?}", simulate_profit);
    }


    #[tokio::test]
    async fn get_uniswapv2_calc() {

        let steps = vec![
            SwapStep {
                pool_address: address!("b34380BA6a17B022782c7FC91e319C10c168FB98"),
                token_in: address!("4200000000000000000000000000000000000006"),
                token_out: address!("532f27101965dd16442E59d40670FaF5eBB142E4"),
                protocol: PoolType::UniswapV2,
                fee: 100,
            },
            SwapStep {
                pool_address: address!("76Bf0abD20f1e0155Ce40A62615a90A709a6C3D8"),
                token_in: address!("532f27101965dd16442E59d40670FaF5eBB142E4"),
                token_out: address!("4200000000000000000000000000000000000006"),
                protocol: PoolType::UniswapV3,
                fee: 3000,
            },
        ];

        let calculated_profit = calculate_profit(steps.clone()).await;
        let simulated_profit = simulate_profit(steps.clone()).await;

        println!("Calculated profit: {:?}", calculated_profit);
        println!("Simulated profit: {:?}", simulated_profit);
    }


    async fn calculate_profit(steps: Vec<SwapStep>) -> U256 {
        dotenv::dotenv().ok();
        let url = std::env::var("FULL").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(url.parse().unwrap()));

        let mut v2_pools: Vec<Address> = Vec::new();
        let mut v3_pools: Vec<Address> = Vec::new();
        for step in &steps {
            match step.protocol {
                PoolType::UniswapV2 | PoolType::SushiSwapV2 => {
                    v2_pools.push(step.pool_address);
                }
                PoolType::UniswapV3 | PoolType::SushiSwapV3 => {
                    v3_pools.push(step.pool_address);
                }
                _ => {}
            }
        }

        let pool_manager = PoolManager::new_with_addresses(v2_pools, v3_pools, provider.clone()).await;
        let mut amount = U256::from(1e17);

        for step in steps {
            amount = step.get_amount_out(amount, &pool_manager);
        }

        amount

    }

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

        let tx = contract.executeArbitrage(steps, U256::from(5e16)).into_transaction_request();
        let output = provider.debug_trace_call(tx, alloy::eips::BlockNumberOrTag::Latest, options.clone()).await.unwrap();
        println!("Output: {:#?}", output);
        match output {
            GethTrace::CallTracer(call_trace) => {
                let output = process_output(&call_trace);
                return output;
            }
            _ => return None
        }
    }



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


    fn get_tracing_options() -> GethDebugTracingCallOptions {
        let options = GethDebugTracingCallOptions {
            tracing_options: GethDebugTracingOptions {
                config: GethDefaultTracingOptions {
                    disable_memory: Some(true),
                    disable_stack: Some(true),
                    disable_storage: Some(true),
                    disable_return_data: Some(true),
                    ..Default::default()
                },
                tracer: Some(GethDebugTracerType::BuiltInTracer(CallTracer)),
                tracer_config: GethDebugTracerConfig(json!({
                    "withLog": true,
                })),
                timeout: None,
                ..Default::default()
            },
            state_overrides: None,
            block_overrides: None,
        };
        options
    }

    pub fn process_output(frame: &CallFrame) -> Option<U256> {
        let mut profit = None;

        for log in &frame.logs {
            let topics = log.topics.as_ref().unwrap();
            if topics.contains(&FlashSwap::ActualValue::SIGNATURE_HASH) {
                //let profit = FlashSwap::Profit::de(&log.data, false).unwrap();
                let profit = FlashSwap::ActualValue::decode_raw_log(topics, &log.data.clone().unwrap(), false).unwrap();
                println!("Profit {:?}", profit);
                return Some(U256::from(profit.value));
            }
        }
    
        for call in &frame.calls {
            if let Some(child_profit) = process_output(call) {
                profit = Some(child_profit);
            }
        }
        profit


    }


}
/*
        SwapStep {
            pool_address: 0xf64c6bca71eda75037a346267ab584b170bff1f7,
            token_in: 0x4200000000000000000000000000000000000006,
            token_out: 0x774194748a26fd0c2c30d6897b174d2bd14e245e,
            protocol: UniswapV3,
            fee: 100,
        },
        SwapStep {
            pool_address: 0xad39e5f569d0c064bd681c0c09e12964372ac609,
            token_in: 0x774194748a26fd0c2c30d6897b174d2bd14e245e,
            token_out: 0x532f27101965dd16442e59d40670faf5ebb142e4,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0xd072da9fffb946e94a33d3ad4db4d7c57061017b,
            token_in: 0x532f27101965dd16442e59d40670faf5ebb142e4,
            token_out: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0xcaeedd8f1acf55f2df259afc090d519069f72a2b,
            token_in: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            token_out: 0x4200000000000000000000000000000000000006,
            protocol: SushiSwapV3,
            fee: 500,
        },
    ]
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Current amount: 50000000000000000
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Cycle [
        SwapStep {
            pool_address: 0xcc9b81c3c5f62c1aaff2ac6244620047cbfbc47a,
            token_in: 0x4200000000000000000000000000000000000006,
            token_out: 0x2fcb1d674d6fdc7ada355abbabe03b29fda73709,
            protocol: UniswapV2,
            fee: 0,
        },
        SwapStep {
            pool_address: 0xbcd9941d794922af8684839423e71fc8ce7658ca,
            token_in: 0x2fcb1d674d6fdc7ada355abbabe03b29fda73709,
            token_out: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0x9f22b553a25857316fb0c63ebdea0093ca03c330,
            token_in: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            token_out: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            protocol: SushiSwapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0xcaeedd8f1acf55f2df259afc090d519069f72a2b,
            token_in: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            token_out: 0x4200000000000000000000000000000000000006,
            protocol: SushiSwapV3,
            fee: 500,
        },
    ]
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Current amount: 50000000000000000
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] 50000000000000000
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Cycle [
        SwapStep {
            pool_address: 0xe11d03bef391ee0a4b670176e23eb44aad490f12,
            token_in: 0x4200000000000000000000000000000000000006,
            token_out: 0x619c4bbbd65f836b78b36cbe781513861d57f39d,
            protocol: UniswapV3,
            fee: 3000,
        },
        SwapStep {
            pool_address: 0x0de17531691c79ac20bced934a67cefff5f596ba,
            token_in: 0x619c4bbbd65f836b78b36cbe781513861d57f39d,
            token_out: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0x4c301155889529998daa63288dc21489d4fc7509,
            token_in: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            token_out: 0xbeb0fd48c2ba0f1aacad2814605f09e08a96b94e,
            protocol: UniswapV2,
            fee: 0,
        },
        SwapStep {
            pool_address: 0xdb46c10bf6bbe65dbea1552c233a97ccae163624,
            token_in: 0xbeb0fd48c2ba0f1aacad2814605f09e08a96b94e,
            token_out: 0x4200000000000000000000000000000000000006,
            protocol: SushiSwapV3,
            fee: 10000,
        },
    ]
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Current amount: 50000000000000000
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] 50000000000000000
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Cycle [
        SwapStep {
            pool_address: 0x0d64931411d53e2bebee71dcf4aef2adde0b33a2,
            token_in: 0x4200000000000000000000000000000000000006,
            token_out: 0xa1b1652222f8e9dc6f359b021d8e87e355cc8fdd,
            protocol: UniswapV2,
            fee: 0,
        },
        SwapStep {
            pool_address: 0x58ab2046775ffce029838c1fa27e0dc48a4800a7,
            token_in: 0xa1b1652222f8e9dc6f359b021d8e87e355cc8fdd,
            token_out: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0x4587df381022922f6622f9096c22e754bcf27b4f,
            token_in: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            token_out: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0xcaeedd8f1acf55f2df259afc090d519069f72a2b,
            token_in: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            token_out: 0x4200000000000000000000000000000000000006,
            protocol: SushiSwapV3,
            fee: 500,
        },
    ]
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Current amount: 50000000000000000
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] 50000000000000000
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] 50000000000000000
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Cycle [
        SwapStep {
            pool_address: 0xcaeedd8f1acf55f2df259afc090d519069f72a2b,
            token_in: 0x4200000000000000000000000000000000000006,
            token_out: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            protocol: SushiSwapV3,
            fee: 500,
        },
        SwapStep {
            pool_address: 0x74a381c6073aaef9e044bf0ab1bef3e2965b2812,
            token_in: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            token_out: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            protocol: UniswapV3,
            fee: 3000,
        },
        SwapStep {
            pool_address: 0xa896b0d9ae008fbd8b7e584cde7efce7602aff9a,
            token_in: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            token_out: 0x0e0c9756a3290cd782cf4ab73ac24d25291c9564,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0x0b8a73dcc8355d0d778f3d33d7d859b1d6a6ccd4,
            token_in: 0x0e0c9756a3290cd782cf4ab73ac24d25291c9564,
            token_out: 0x4200000000000000000000000000000000000006,
            protocol: UniswapV2,
            fee: 0,
        },
    ]
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Current amount: 50000000000000000
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Cycle [
        SwapStep {
            pool_address: 0xfb08774bcebdc415d556c22c2a5f7e0ed76966ce,
            token_in: 0x4200000000000000000000000000000000000006,
            token_out: 0xa1b1652222f8e9dc6f359b021d8e87e355cc8fdd,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0x58ab2046775ffce029838c1fa27e0dc48a4800a7,
            token_in: 0xa1b1652222f8e9dc6f359b021d8e87e355cc8fdd,
            token_out: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0xd56da2b74ba826f19015e6b7dd9dae1903e85da1,
            token_in: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            token_out: 0xfde4c96c8593536e31f229ea8f37b2ada2699bb2,
            protocol: UniswapV3,
            fee: 100,
        },
        SwapStep {
            pool_address: 0xd92e0767473d1e3ff11ac036f2b1db90ad0ae55f,
            token_in: 0xfde4c96c8593536e31f229ea8f37b2ada2699bb2,
            token_out: 0x4200000000000000000000000000000000000006,
            protocol: UniswapV3,
            fee: 500,
        },
    ]
[2024-08-06T14:15:41Z INFO  BaseBuster::graph] Cycle [
        SwapStep {
            pool_address: 0xcaeedd8f1acf55f2df259afc090d519069f72a2b,
            token_in: 0x4200000000000000000000000000000000000006,
            token_out: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            protocol: SushiSwapV3,
            fee: 500,
        },
        SwapStep {
            pool_address: 0x4587df381022922f6622f9096c22e754bcf27b4f,
            token_in: 0x6985884c4392d348587b19cb9eaaf157f13271cd,
            token_out: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            protocol: UniswapV3,
            fee: 10000,
        },
        SwapStep {
            pool_address: 0x45dc8b3c7f05593d49745d804315348ff2a6f080,
            token_in: 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913,
            token_out: 0x340c070260520ae477b88caa085a33531897145b,
            protocol: UniswapV3,
            fee: 3000,
        },
        SwapStep {
            pool_address: 0x3e8c30c4c54377499de40e84e4c40f83f74aa1b7,
            token_in: 0x340c070260520ae477b88caa085a33531897145b,
            token_out: 0x4200000000000000000000000000000000000006,
            protocol: UniswapV3,
            fee: 500,
        },
    ]
[2024-08-06T14:1
 */