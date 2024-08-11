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
    use tokio::sync::broadcast;
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
        */
        let steps = vec![
            SwapStep {
                pool_address: address!("50982a32af4f9e090df95956f3b07ffe70badb21"),
                token_in: address!("4200000000000000000000000000000000000006"),
                token_out: address!("8b03d30b88e86fc5f447069c79ec56b8e7d87ab6"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("e05a18cb701c76004869aed16997051bd1ed0bbc"),
                token_in: address!("8b03d30b88e86fc5f447069c79ec56b8e7d87ab6"),
                token_out: address!("21eceaf3bf88ef0797e3927d855ca5bb569a47fc"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("263ea0a3cf3845fc52a30c6e81dbd985b7290fbf"),
                token_in: address!("21eceaf3bf88ef0797e3927d855ca5bb569a47fc"),
                token_out: address!("47b464edb8dc9bc67b5cd4c9310bb87b773845bd"),
                protocol: PoolType::UniswapV3,
                fee: 10000,
            },
            SwapStep {
                pool_address: address!("98433581b5420bc67fc5fd2b5f9dd3e7ca43140b"),
                token_in: address!("47b464edb8dc9bc67b5cd4c9310bb87b773845bd"),
                token_out: address!("4200000000000000000000000000000000000006"),
                protocol: PoolType::SushiSwapV2,
                fee: 0,
            },
        ];
    


        let pools = load_pools().await;
        println!("got here");
        let (arb_sender, arb_receiver) = broadcast::channel(200);
        let pool_manager = pool_manager::PoolManager::new(pools, arb_sender).await;

        println!("blah");
        let calculate_profit = calculate_profit(steps.clone(), pool_manager);
        let simulate_profit = simulate_profit(steps.clone()).await;

        let pool_addr =  address!("50982a32af4f9e090df95956f3b07ffe70badb21");
        

        println!("Calculated profit: {:?}", calculate_profit);
        println!("Simulated profit: {:?}", simulate_profit);
    }

    // Calculates the expected profit based off of the swap steps
    fn calculate_profit(steps: Vec<SwapStep>, pool_manager: Arc<PoolManager>) -> U256 {
        let mut amount = U256::from(1e16);
        for step in steps {
            println!("step: {:?}", step);
            amount = step.get_amount_out(amount, &pool_manager);
            println!("amount: {:?}", amount);
        }
        amount

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
        let tx = contract.executeArbitrage(steps, U256::from(1e16)).from(anvil.addresses()[0]).into_transaction_request();
        let output = provider.debug_trace_call(tx, alloy::eips::BlockNumberOrTag::Latest, options.clone()).await.unwrap();
        //println!("Output: {:#?}", output);
        match output {
            GethTrace::CallTracer(call_trace) => {
                //let output = process_output(&call_trace);

                let res = extract_final_balance(&call_trace);
                return res;
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
                PoolType::SushiSwapV2,
                //PoolType::PancakeSwapV2,
                PoolType::UniswapV3,
                PoolType::SushiSwapV3,
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
    