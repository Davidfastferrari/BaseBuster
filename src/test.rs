#[cfg(test)]
mod tests {
    use alloy::network::Ethereum;
    use alloy::network::EthereumWallet;
    use alloy::node_bindings::Anvil;
    use alloy::primitives::U256;
    use alloy::primitives::{address, Address};
    use alloy::providers::ext::DebugApi;
    use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
    use alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer;
    use alloy::rpc::types::trace::geth::GethDebugTracingOptions;
    use alloy::rpc::types::trace::geth::{
        CallConfig, CallFrame, GethDebugTracerConfig, GethDebugTracerType,
        GethDebugTracingCallOptions, GethDefaultTracingOptions, GethTrace,
    };
    use alloy::signers::local::PrivateKeySigner;
    use alloy::sol;
    use alloy::transports::http::{Client, Http};
    use gweiyser::protocols::uniswap::v2::UniswapV2Pool;
    use gweiyser::protocols::uniswap::v3::UniswapV3Pool;
    use gweiyser::{Chain, Gweiyser};
    use pool_sync::*;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    use crate::events::Event;
    use crate::graph::SwapStep;
    use crate::pool_manager;
    use crate::pool_manager::PoolManager;
    use crate::util::get_working_pools;
    use crate::FlashSwap;

    #[tokio::test]
    pub async fn test_state_updates() {
        dotenv::dotenv().ok();
        let (pools, last_synced_block) = load_pools().await;
        let working_pools = get_working_pools(pools, 100, pool_sync::Chain::Base).await;
        let (pool_manager, mut reserve_receiver) =
            construct_pool_manager(working_pools.clone(), last_synced_block).await;

        let mut gweiyser_pools_v2 = vec![];
        let mut gweiyser_pools_v3 = vec![];
        println!("constructing pools");
        for pool in working_pools.iter() {
            if pool.is_v3() {
                let pool = get_v3_pool(&pool.address()).await;
                gweiyser_pools_v3.push(pool);
            } else {
                let pool = get_v2_pool(pool.address()).await;
                gweiyser_pools_v2.push(pool);
            }
        }
        println!("constructined pools");

        while let Ok(Event::ReserveUpdate(updated_pools)) = reserve_receiver.recv().await {
            println!("Got update {}", updated_pools.len());

            for pool in gweiyser_pools_v3.iter() {
                let slot0 = pool.slot0().await;
                let liquidity = pool.liquidity().await;

                let pool_manager_pool = pool_manager.get_v3pool(&pool.address());
                assert_eq!(slot0.tick, pool_manager_pool.tick);
                assert_eq!(liquidity, pool_manager_pool.liquidity);
                assert_eq!(slot0.sqrt_price_x96, pool_manager_pool.sqrt_price);
            }

            for pool in gweiyser_pools_v2.iter_mut() {
                let reserve0 = pool.token0_reserves().await;
                let reserve1 = pool.token1_reserves().await;

                let pool_manager_pool = pool_manager.get_v2pool(&pool.address());
                assert_eq!(reserve0, U256::from(pool_manager_pool.token0_reserves));
                assert_eq!(reserve1, U256::from(pool_manager_pool.token1_reserves));
            }

            println!("success");
        }
    }

    #[tokio::test]
    pub async fn test_sim_speed() {
        todo!()
    }

    #[tokio::test]
    pub async fn test_uniswapv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("88a43bbdf9d098eec7bceda4e2494615dfd9bb9c"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::UniswapV2,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calc_offchain_amount(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            get_onchain_amount(swap_step, PoolType::UniswapV2, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    #[tokio::test]
    pub async fn test_uniswapv3_out() {
        let swap_step = SwapStep {
            pool_address: address!("4e0052d57836e49804ccdd9d2436be186a72af20"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("5babfc2f240bc5de90eb7e19d789412db1dec402"),
            protocol: PoolType::UniswapV3,
            fee: 3000,
        };

        let amount_in = U256::from(1e16);
        let offchain_amount_out = calc_offchain_amount(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            get_onchain_amount(swap_step, PoolType::UniswapV3, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    #[tokio::test]
    pub async fn test_sushiswapv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("98433581b5420bc67fc5fd2b5f9dd3e7ca43140b"),
            token_in: address!("47b464edb8dc9bc67b5cd4c9310bb87b773845bd"),
            token_out: address!("4200000000000000000000000000000000000006"),
            protocol: PoolType::SushiSwapV2,
            fee: 0,
        };

        let amount_in = U256::from(1e16);
        let offchain_amount_out = calc_offchain_amount(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            get_onchain_amount(swap_step, PoolType::SushiSwapV2, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    #[tokio::test]
    pub async fn test_sushiswapv3_out() {
        let swap_step = SwapStep {
            pool_address: address!("47c59970b19c1274f4e82f5852ca61bff50796e6"),
            token_in: address!("2ae3f1ec7f1f5012cfeab0185bfc7aa3cf0dec22"),
            token_out: address!("4200000000000000000000000000000000000006"),
            protocol: PoolType::SushiSwapV3,
            fee: 100,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calc_offchain_amount(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            get_onchain_amount(swap_step, PoolType::SushiSwapV3, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    #[tokio::test]
    pub async fn test_pancakeswapv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("60824b0543410d824291c29be32284456fcf1f8e"),
            token_in: address!("2ae3f1ec7f1f5012cfeab0185bfc7aa3cf0dec22"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::PancakeSwapV2,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calc_offchain_amount(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            get_onchain_amount(swap_step, PoolType::PancakeSwapV2, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    #[tokio::test]
    pub async fn test_pancakeswapv3_out() {
        todo!()
    }

    #[tokio::test]
    pub async fn test_aerodrome_out() {
        let swap_step = SwapStep {
            pool_address: address!("7f07005b7a6500ec8ad7191424122378f9f06607"),
            token_in: address!("afb89a09d82fbde58f18ac6437b3fc81724e4df6"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::Aerodrome,
            fee: 0,
        };

        let amount_in = U256::from(1e16);
        let offchain_amount_out = calc_offchain_amount(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            get_onchain_amount(swap_step, PoolType::Aerodrome, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    #[tokio::test]
    pub async fn test_slipstream_out() {
        todo!()
    }

    #[tokio::test]
    pub async fn test_baseswapv2_out() {
        let swap_step = SwapStep {
            pool_address: address!("1be25ca7954b8ce47978851a0689312518d85f0c"),
            token_in: address!("2ae3f1ec7f1f5012cfeab0185bfc7aa3cf0dec22"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::BaseSwapV2,
            fee: 0,
        };

        let amount_in = U256::from(1e16);
        let offchain_amount_out = calc_offchain_amount(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            get_onchain_amount(swap_step, PoolType::BaseSwapV2, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    #[tokio::test]
    pub async fn test_baseswapv3_out() {
        let swap_step = SwapStep {
            pool_address: address!("74cb6260be6f31965c239df6d6ef2ac2b5d4f020"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::BaseSwapV3,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calc_offchain_amount(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            get_onchain_amount(swap_step, PoolType::BaseSwapV3, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    #[tokio::test]
    pub async fn debug_swap() {
        dotenv::dotenv().ok();
        let swaps = vec![
            SwapStep {
                pool_address: address!("f012e803c8e73d185ac96d6c3f830893a1116e03"),
                token_in: address!("4200000000000000000000000000000000000006"),
                token_out: address!("4dd9077269dd08899f2a9e73507125962b5bc87f"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("69887520aad31258b090ff32b25b6141ca9eb396"),
                token_in: address!("4dd9077269dd08899f2a9e73507125962b5bc87f"),
                token_out: address!("532f27101965dd16442e59d40670faf5ebb142e4"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("36a46dff597c5a444bbc521d26787f57867d2214"),
                token_in: address!("532f27101965dd16442e59d40670faf5ebb142e4"),
                token_out: address!("4200000000000000000000000000000000000006"),
                protocol: PoolType::UniswapV3,
                fee: 500,
            },
        ];

        let converted_path: Vec<FlashSwap::SwapStep> = swaps
            .clone()
            .iter()
            .map(|step| FlashSwap::SwapStep {
                poolAddress: step.pool_address,
                tokenIn: step.token_in,
                tokenOut: step.token_out,
                protocol: step.as_u8(),
                fee: step.fee,
            })
            .collect();

        execute_swappath(swaps).await;
        let profit = simulate_profit(converted_path).await;
        println!("Profit: {:?}", profit);
    }

    // ALL OF THE HELPER FUNCTIONS THAT ARE USED IN THE TESTS
    // ------------------------------------------------------
    pub async fn calc_offchain_amount(swap_step: SwapStep, amount_in: U256) -> U256 {
        let (pool_manager, mut reserve_receiver) =
            pool_manager_with_pools(vec![swap_step.pool_address]).await;
        if let Ok(update) = reserve_receiver.recv().await {
            return swap_step.get_amount_out(amount_in, &pool_manager);
        }
        U256::ZERO
    }

    pub async fn execute_swappath(steps: Vec<SwapStep>) {
        let pools = steps.iter().map(|step| step.pool_address).collect();
        let (pool_manager, mut reserve_receiver) = pool_manager_with_pools(pools).await;

        let mut amount_in = U256::from(2e15);
        for step in steps {
            println!("amount in: {:?}", amount_in);
            amount_in = step.get_amount_out(amount_in, &pool_manager);
            println!("amount out: {:?}", amount_in);
        }
    }

    pub async fn get_onchain_amount(
        swap_step: SwapStep,
        pool_type: PoolType,
        amount_in: U256,
    ) -> U256 {
        dotenv::dotenv().ok();
        let provider =
            ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
        match pool_type {
            PoolType::UniswapV2
            | PoolType::SushiSwapV2
            | PoolType::PancakeSwapV2
            | PoolType::BaseSwapV2 => {
                sol!(
                    #[sol(rpc)]
                    contract V2Router {
                        function getAmountsOut(uint amountIn, address[] calldata path) external view returns (uint[] memory amounts);
                    }
                );

                let address = match pool_type {
                    PoolType::UniswapV2 => address!("4752ba5DBc23f44D87826276BF6Fd6b1C372aD24"),
                    PoolType::SushiSwapV2 => address!("6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891"),
                    PoolType::PancakeSwapV2 => address!("8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb"),
                    PoolType::BaseSwapV2 => address!("327Df1E6de05895d2ab08513aaDD9313Fe505d86"),
                    _ => panic!("will not reach here"),
                };

                let contract = V2Router::new(address, provider);

                let V2Router::getAmountsOutReturn { amounts } = contract
                    .getAmountsOut(amount_in, vec![swap_step.token_in, swap_step.token_out])
                    .call()
                    .await
                    .unwrap();
                return *amounts.last().unwrap();
            }
            PoolType::UniswapV3 | PoolType::PancakeSwapV3 => {
                todo!()
            }
            PoolType::SushiSwapV3 | PoolType::BaseSwapV3 | PoolType::Slipstream => {
                todo!()
            }
            PoolType::Aerodrome => {
                sol!(
                    #[sol(rpc)]
                    contract Aerodrome {
                        struct Route {
                            address from;
                            address to;
                            bool stable;
                            address factory;
                        }
                        function getAmountsOut(uint amountIn, Route[] calldata routes) external view returns (uint[] memory amounts);
                    }
                );

                let contract = Aerodrome::new(
                    address!("cF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43"),
                    provider,
                );

                let route = Aerodrome::Route {
                    from: swap_step.token_in,
                    to: swap_step.token_out,
                    stable: true,
                    factory: Address::ZERO,
                };

                let Aerodrome::getAmountsOutReturn { amounts } = contract
                    .getAmountsOut(amount_in, vec![route])
                    .call()
                    .await
                    .unwrap();
                return *amounts.last().unwrap();
            }
        }
    }

    pub async fn pool_manager_with_pools(
        addresses: Vec<Address>,
    ) -> (Arc<PoolManager>, broadcast::Receiver<Event>) {
        println!("addresses: {:#?}", addresses);
        let (pools, last_synced_block) = load_pools().await;
        let pools: Vec<Pool> = addresses
            .iter()
            .map(|address| {
                pools
                    .clone()
                    .into_iter()
                    .find(|pool| pool.address() == *address)
                    .unwrap()
            })
            .collect();

        println!("pool: {:#?}", pools);
        let (pool_manager, mut reserve_receiver) =
            construct_pool_manager(pools.clone(), last_synced_block).await;
        (pool_manager, reserve_receiver)
    }

    // simulated profit based off of the call to the contract
    async fn simulate_profit(steps: Vec<FlashSwap::SwapStep>) -> Option<U256> {
        let url = std::env::var("FULL").unwrap();
        let provider = ProviderBuilder::new().on_http(url.parse().unwrap());
        let fork_block = provider.get_block_number().await.unwrap();

        let anvil = Anvil::new()
            .fork(url)
            .port(9100_u16)
            .fork_block_number(18433480)
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

        let flash_contract = FlashSwap::deploy(anvil_signer.clone()).await.unwrap();
        let flash_address = flash_contract.address();
        let options = get_tracing_options();

        let provider =
            Arc::new(ProviderBuilder::new().on_http("http://localhost:9100".parse().unwrap()));
        let contract = FlashSwap::new(*flash_address, provider.clone());

        //println!("{:?}", FlashSwap::executeArbitrageCall::SELECTOR);
        let tx = contract
            .executeArbitrage(steps, U256::from(2e15))
            .from(anvil.addresses()[0])
            .into_transaction_request();
        let output = provider
            .debug_trace_call(tx, alloy::eips::BlockNumberOrTag::Latest, options.clone())
            .await
            .unwrap();
        println!("Output: {:#?}", output);
        match output {
            GethTrace::CallTracer(call_trace) => {
                //let output = process_output(&call_trace);

                let res = extract_final_balance(&call_trace);
                return res;
            }
            _ => return None,
        }
    }

    // Get a v3 pool from gweiyser
    pub async fn get_v3_pool(
        pool_address: &Address,
    ) -> UniswapV3Pool<RootProvider<Http<Client>>, Http<Client>, Ethereum> {
        let provider = Arc::new(
            ProviderBuilder::new()
                .network::<Ethereum>()
                .on_http(std::env::var("FULL").unwrap().parse().unwrap()),
        );
        let gweiyser = Gweiyser::new(provider.clone(), Chain::Base);
        gweiyser.uniswap_v3_pool(*pool_address).await
    }

    // Get a v3 pool fro gweiyser
    pub async fn get_v2_pool(
        pool_address: Address,
    ) -> UniswapV2Pool<RootProvider<Http<Client>>, Http<Client>, Ethereum> {
        let provider = Arc::new(
            ProviderBuilder::new()
                .network::<Ethereum>()
                .on_http(std::env::var("FULL").unwrap().parse().unwrap()),
        );
        let gweiyser = Gweiyser::new(provider.clone(), Chain::Base);
        gweiyser.uniswap_v2_pool(pool_address).await
    }

    // load all the pools from pool_sync
    pub async fn load_pools() -> (Vec<Pool>, u64) {
        dotenv::dotenv().ok();

        let pool_sync = PoolSync::builder()
            .add_pools(&[
                PoolType::UniswapV2,
                //PoolType::SushiSwapV2,
                PoolType::UniswapV3,
                //PoolType::SushiSwapV3,
                //PoolType::PancakeSwapV2,
                PoolType::Aerodrome,
                //PoolType::BaseSwapV2
            ])
            .chain(pool_sync::Chain::Base)
            .build()
            .unwrap();
        pool_sync.sync_pools().await.unwrap()
    }

    // construct the pool manager from working pools
    pub async fn construct_pool_manager(
        pools: Vec<Pool>,
        last_synced_block: u64,
    ) -> (Arc<PoolManager>, broadcast::Receiver<Event>) {
        let (update_sender, update_receiver) = broadcast::channel(200);
        let pool_manager = PoolManager::new(pools, update_sender, last_synced_block).await;
        (pool_manager, update_receiver)
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
                tracer_config: GethDebugTracerConfig(
                    serde_json::to_value(CallConfig {
                        only_top_call: Some(false),
                        with_log: Some(true),
                    })
                    .unwrap()
                    .into(),
                ),
                timeout: None,
                ..Default::default()
            },
            state_overrides: None,
            block_overrides: None,
        };
        options
    }

    // Extract the final balance from the call trace
    fn extract_final_balance(call_trace: &CallFrame) -> Option<U256> {
        // Function to recursively search for the last balance check
        fn search_calls(frame: &CallFrame) -> Option<U256> {
            // Check if this is a balance check call
            if frame.input.starts_with(b"\x70\xa0\x82\x31") && frame.output.is_some() {
                // This is likely a balanceOf call, parse the output
                return frame.output.as_ref().and_then(|output| {
                    if output.len() >= 32 {
                        Some(U256::from_be_bytes::<32>(output[0..32].try_into().unwrap()))
                    } else {
                        None
                    }
                });
            }
            // If not, search through subcalls in reverse order
            for subcall in frame.calls.iter().rev() {
                if let Some(balance) = search_calls(subcall) {
                    return Some(balance);
                }
            }
            None
        }
        search_calls(call_trace)
    }
}
