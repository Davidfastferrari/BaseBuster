use alloy::network::Ethereum;
use alloy::network::EthereumWallet;
use alloy::node_bindings::Anvil;
use alloy::node_bindings::AnvilInstance;
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
use alloy::primitives::FixedBytes;
use gweiyser::addresses::amms;
use gweiyser::protocols::uniswap::v2::UniswapV2Pool;
use gweiyser::protocols::uniswap::v3::UniswapV3Pool;
use gweiyser::{Chain, Gweiyser};
use pool_sync::*;
use revm::interpreter::instructions::contract;
use sha2::digest::consts::U25;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::calculation::Calculator;
use crate::events::Event;
use crate::graph::SwapStep;
use crate::pool_manager;
use crate::pool_manager::PoolManager;
use crate::util::get_working_pools;
use crate::FlashSwap;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashQuoter,
    "src/abi/FlashQuoter.json"
);




// All offchain calculation tests
#[cfg(test)]
mod offchain_calculations {
    use super::*;

    // TODO
    // Aerodrome failed the test
    // need to impl BaseSwapV3 and Slipstream, pools first
    // Need to impl bigger function that runs them all in a loop for a while
    // need to impl alienbase

    // Test amount out on UNISWAP V2
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
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::UniswapV2, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // Test amount out on UNISWAP V3
    #[tokio::test]
    pub async fn test_uniswapv3_out() {
        let swap_step = SwapStep {
            pool_address: address!("a2d4a8e00daad32acace1a0dd0905f6aaf57e84e"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("6985884c4392d348587b19cb9eaaf157f13271cd"),
            protocol: PoolType::UniswapV3,
            fee: 3000,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::UniswapV3, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // Test amount out on SUSHISWAP V2
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
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::SushiSwapV2, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }


    // Test amount out on SUSHISWAP V3
    #[tokio::test]
    pub async fn test_sushiswapv3_out() {
        let swap_step =     SwapStep {
            pool_address: address!("a73f10b99551f6e08609ccdec5ff66d51e4e3700"),
            token_in: address!("532f27101965dd16442e59d40670faf5ebb142e4"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::SushiSwapV3,
            fee: 10000,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::SushiSwapV3, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // Test amount out on PANCAKESWAP V2
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
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::PancakeSwapV2, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // Test amount out on PANCAKESWAP V3
    #[tokio::test]
    pub async fn test_pancakeswapv3_out() {
        let swap_step = SwapStep {
                pool_address: address!("3c288a41c135fb0bae3f95b6a37b5e3e89f3fd95"),
                token_in: address!("4200000000000000000000000000000000000006"),
                token_out: address!("b1a03eda10342529bbf8eb700a06c60441fef25d"),
                protocol: PoolType::PancakeSwapV3,
                fee: 500,
            };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::PancakeSwapV3, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }


    // Test amount out on AERODROME
    #[tokio::test]
    pub async fn test_aerodrome_out() {
        let swap_step =     SwapStep {
            pool_address: address!("acb7907c232907934b2578315dfcfa1ba60e87af"),
            token_in: address!("9beec80e62aa257ced8b0edd8692f79ee8783777"),
            token_out: address!("4200000000000000000000000000000000000006"),
            protocol: PoolType::Aerodrome,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::Aerodrome, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }


    // Test amount out on SLIPSTREAM
    #[tokio::test]
    pub async fn test_slipstream_out() {
        todo!()
    }

    // Test amount out on BASESWAP V2
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
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::BaseSwapV2, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // Test amount out on BASESWAP V3
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
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::BaseSwapV3, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    // Test amount out on  V3
    #[tokio::test]
    pub async fn test_alienbase_out() {
        let swap_step = SwapStep {
            pool_address: address!("74cb6260be6f31965c239df6d6ef2ac2b5d4f020"),
            token_in: address!("4200000000000000000000000000000000000006"),
            token_out: address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            protocol: PoolType::BaseSwapV3,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        let onchain_amount_out =
            simulate_single_quote(swap_step, PoolType::BaseSwapV3, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        println!("onchain amount out: {:?}", onchain_amount_out);
        assert_eq!(offchain_amount_out, onchain_amount_out);
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_maverick_out() {
        dotenv::dotenv().ok();
        env_logger::init();
        let swap_step = SwapStep {
            pool_address: address!("5b6a0771c752e35b2ca2aff4f22a66b1598a2bc5"),
            token_in: address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            token_out: address!("dac17f958d2ee523a2206206994597c13d831ec7"),
            protocol: PoolType::MaverickV2,
            fee: 0,
        };
        let amount_in = U256::from(1e7);
        let calculator = Calculator::new().await;
        let zero_for_one = true;
        let tick_lim = i32::MAX;
        //let amount_out = calculator.calculate_maverick_out(amount_in, swap_step.pool_address, zero_for_one, tick_lim);
        //println!("amount out: {:?}", amount_out);
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_balancer_out() {
        dotenv::dotenv().ok();
        let swap_step = SwapStep {
            pool_address: address!("98b76fb35387142f97d601a297276bb152ae8ab0"),
            token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: address!("faba6f8e4a5e8ab82f62fe7c39859fa577269be3"),
            protocol: PoolType::BalancerV2,
            fee: 0,
        };
        let amount_in = U256::from(1e16);
        let offchain_amount_out = calculate_single_quote(swap_step.clone(), amount_in).await;
        //let onchain_amount_out =
         //   simulate_single_quote(swap_step, PoolType::BalancerV2, amount_in).await;
        println!("offchain amount out: {:?}", offchain_amount_out);
        //println!("onchain amount out: {:?}", onchain_amount_out);
        //assert_eq!(offchain_amount_out, onchain_amount_out);
    }
}




#[cfg(test)]
pub mod flash_swap {
    use super::*;
    #[tokio::test]
    pub async fn debug_swap() {
        dotenv::dotenv().ok();
        let swaps = vec![        
            SwapStep {
                pool_address: address!("f282e7c46be1a3758357a5961cf02e1f46a78b75"),
                token_in: address!("4200000000000000000000000000000000000006"),
                token_out: address!("2075f6e2147d4ac26036c9b4084f8e28b324397d"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("f609cdba05f08e850676f7434db0d9468b3701bd"),
                token_in: address!("2075f6e2147d4ac26036c9b4084f8e28b324397d"),
                token_out: address!("6921b130d297cc43754afba22e5eac0fbf8db75b"),
                protocol: PoolType::UniswapV3,
                fee: 10000,
            },
            SwapStep {
                pool_address: address!("088c39ee29fc30df8adc394e9f7dea33e3a26507"),
                token_in: address!("6921b130d297cc43754afba22e5eac0fbf8db75b"),
                token_out: address!("4200000000000000000000000000000000000006"),
                protocol: PoolType::Slipstream,
                fee: 8000,
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

        //calculate_full_quote(swaps).await;
        let profit = simulate_profit(converted_path).await;
        println!("Profit: {:?}", profit);
    }

}



pub mod info_sync {
    use super::*;

    #[tokio::test]
    pub async fn test_state_updates() {
        dotenv::dotenv().ok();
        let (pools, last_synced_block) = load_pools().await;
        let working_pools = get_working_pools(pools, 50, pool_sync::Chain::Base).await;
        let working_pools: Vec<Pool> = working_pools.into_iter().filter(|pool| {
            if pool.is_v3() {
                let v3_pool = pool.get_v3().unwrap();
                return v3_pool.liquidity > 0;
            };
            true
        }).collect();
        let (pool_manager, mut reserve_receiver) =
            construct_pool_manager(working_pools.clone(), last_synced_block).await;

        let mut gweiyser_pools_v2 = vec![];
        let mut gweiyser_pools_v3 = vec![];
        for pool in working_pools.iter() {
            if pool.is_v3() {
                let pool = get_v3_pool(&pool.address()).await;
                gweiyser_pools_v3.push(pool);
            } else {
                let pool = get_v2_pool(pool.address()).await;
                gweiyser_pools_v2.push(pool);
            }
        }

        while let Ok(Event::ReserveUpdate(updated_pools)) = reserve_receiver.recv().await {
            println!("Got update {}", updated_pools.len());

            for address in updated_pools.iter() {
                for pool in gweiyser_pools_v3.iter() {
                    if pool.address() == *address {
                        let slot0 = pool.slot0().await;
                        let liquidity = pool.liquidity().await;
                        let pool_manager_pool = pool_manager.get_v3pool(&pool.address());
                        
                        if slot0.tick != pool_manager_pool.tick || 
                        liquidity != pool_manager_pool.liquidity || 
                        slot0.sqrt_price_x96 != pool_manager_pool.sqrt_price {
                            println!("Mismatch found in V3 pool: {}", pool.address());
                            println!("Gweiyser pool: tick: {}, liquidity: {}, sqrt_price_x96: {}", 
                                    slot0.tick, liquidity, slot0.sqrt_price_x96);
                            println!("Pool manager pool: tick: {}, liquidity: {}, sqrt_price: {}", 
                                    pool_manager_pool.tick, pool_manager_pool.liquidity, pool_manager_pool.sqrt_price);
                            panic!("V3 pool mismatch");
                        }
                    }
                }

                for pool in gweiyser_pools_v2.iter_mut() {
                    if pool.address() == *address {
                        let reserve0 = pool.token0_reserves().await;
                        let reserve1 = pool.token1_reserves().await;
                        let pool_manager_pool = pool_manager.get_v2pool(&pool.address());
                        
                        if reserve0 != U256::from(pool_manager_pool.token0_reserves) || 
                        reserve1 != U256::from(pool_manager_pool.token1_reserves) {
                            println!("Mismatch found in V2 pool: {}", pool.address());
                            println!("Gweiyser pool: reserve0: {}, reserve1: {}", reserve0, reserve1);
                            println!("Pool manager pool: token0_reserves: {}, token1_reserves: {}", 
                                    pool_manager_pool.token0_reserves, pool_manager_pool.token1_reserves);
                            panic!("V2 pool mismatch");
                        }
                    }
                }
            }
            println!("success");
        }
    }
}


#[cfg(test)]
mod test_path_quotes{
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn quote() {
        dotenv::dotenv().ok();
        let swaps = vec![
            SwapStep {
                pool_address: address!("4114fd8554e63a9e0f09ca2480977883fea06430"),
                token_in: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                token_out: address!("dac17f958d2ee523a2206206994597c13d831ec7"),
                protocol: PoolType::BalancerV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("e618785149a1ee7d3042e304e2f899f7a4616b7d"),
                token_in: address!("dac17f958d2ee523a2206206994597c13d831ec7"),
                token_out: address!("34950ff2b487d9e5282c5ab342d08a2f712eb79f"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("1b7143e445b4d1424fa24f0c3ba0c5778da43c5b"),
                token_in: address!("34950ff2b487d9e5282c5ab342d08a2f712eb79f"),
                token_out: address!("1f9840a85d5af5bf1d1762f925bdaddc4201f984"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
            SwapStep {
                pool_address: address!("d3d2e2692501a5c9ca623199d38826e513033a17"),
                token_in: address!("1f9840a85d5af5bf1d1762f925bdaddc4201f984"),
                token_out: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                protocol: PoolType::UniswapV2,
                fee: 0,
            },
        ];


        let amount = U256::from(1e16);
        let simulated_quote = simulate_full_quote(swaps.clone(), amount).await;
        let calculated_quote = calculate_full_quote(swaps, amount).await;
        //println!("Profit: {:?}", simulated_quote);
        println!("Calculated profit: {:?}", calculated_quote);

    }


}







// HELPER FUNCTIONS FOR THE TESTS
// ------------------------------------------------------

// Use the quoter contract to get the amount out from a swap path
pub async fn simulate_full_quote(swap_steps: Vec<SwapStep>, amount: U256) -> U256 {
    // deploy the quoter
    let url = std::env::var("FULL").unwrap();
    let anvil = Anvil::new()
        .fork(url)
        .port(9100_u16)
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
    let gweiyser = Gweiyser::new(anvil_signer.clone(), Chain::Ethereum);
    let weth = gweiyser.token(address!("4200000000000000000000000000000000000006")).await;
    let weth = gweiyser.token(address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")).await;
    weth.deposit(amount).await; // deposit into signers account, account[0] here
    weth.transfer_from(anvil.addresses()[0], *flash_quoter.address(), amount).await;
    weth.approve(*flash_quoter.address(), amount).await;

    let converted_path = swappath_to_flashquote(swap_steps.clone()).await;

    let provider = ProviderBuilder::new().on_http("http://localhost:9100".parse().unwrap());
    let flash_quoter = FlashQuoter::new(*flash_quoter.address(), provider.clone());
    let FlashQuoter::executeArbitrageReturn { _0: profit } = flash_quoter
        .executeArbitrage(converted_path, amount)
        .from(anvil.addresses()[0])
        .call()
        .await
        .unwrap();
    profit
}


// use out offchain calculations to get the amount out from a swap path
pub async fn calculate_full_quote(steps: Vec<SwapStep>, amount: U256) -> U256 {
    // get all of the pools in the swap path and put them into the pool manager
    let pools = steps.iter().map(|step| step.pool_address).collect(); 
    let (pool_manager, mut reserve_receiver) = pool_manager_with_pools(pools).await;
    let calculator = Calculator::new().await;

    // wait for a new update so we are working wtih fresh set
    let mut amount_in = amount;
    if let Ok(_) = reserve_receiver.recv().await {
        for step in steps {
            println!("Amount in: {}", amount_in);
            amount_in = calculator.get_amount_out(amount_in, &pool_manager, &step);

            println!("Amount out: {}", amount_in);
        }
    }
    amount_in
}


// use the offchain calculations to get the amount out for a single swap
pub async fn calculate_single_quote(swap_step: SwapStep, amount_in: U256) -> U256 {
    let (pool_manager, mut reserve_receiver) =
        pool_manager_with_pools(vec![swap_step.pool_address]).await;

    let calculator = Calculator::new().await;
    if let Ok(_) = reserve_receiver.recv().await {
        let output = calculator.get_amount_out(amount_in, &pool_manager, &swap_step);
        println!("output: {:#?}", output);
        return output;
    }
    U256::ZERO
}


// use the onchain qouters to get the amount out for a single swap
pub async fn simulate_single_quote(
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
        | PoolType::BaseSwapV2 
        | PoolType::AlienBase=> {
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
                PoolType::AlienBase => address!("8c1A3cF8f83074169FE5D7aD50B978e1cD6b37c7"),
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
        PoolType::UniswapV3 | PoolType::PancakeSwapV3 | PoolType::SushiSwapV3 | PoolType::Slipstream=> {
            sol!(
                #[sol(rpc)]
                contract V3Quoter {
                    struct QuoteExactInputSingleParams {
                        address tokenIn;
                        address tokenOut;
                        uint256 amountIn;
                        uint24 fee;
                        uint160 sqrtPriceLimitX96;
                    }
                    function quoteExactInputSingle(QuoteExactInputSingleParams memory params)
                    external
                    returns (
                        uint256 amountOut,
                        uint160 sqrtPriceX96After,
                        uint32 initializedTicksCrossed,
                        uint256 gasEstimate
                    );

                }
            );

            let address = match pool_type {
                PoolType::UniswapV3 => address!("3d4e44Eb1374240CE5F1B871ab261CD16335B76a"),
                PoolType::PancakeSwapV3 => address!("B048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997"),
                PoolType::SushiSwapV3 => address!("b1E835Dc2785b52265711e17fCCb0fd018226a6e"),
                PoolType::Slipstream => address!("254cF9E1E6e233aa1AC962CB9B05b2cfeAaE15b0"),
                _ => panic!("Invalid pool type"),
            };

            let contract = V3Quoter::new(address, provider.clone());

            let params = V3Quoter::QuoteExactInputSingleParams {
                tokenIn: swap_step.token_in,
                tokenOut: swap_step.token_out,
                fee: swap_step.fee,
                amountIn: amount_in,
                sqrtPriceLimitX96: U256::ZERO,
            };

            let V3Quoter::quoteExactInputSingleReturn { amountOut , ..} = contract
                .quoteExactInputSingle(params)
                .call()
                .await
                .unwrap();
            return amountOut;
        }
        PoolType::BaseSwapV3 => {
            sol!(
                #[sol(rpc)]
                contract V3Router {
                    struct ExactInputSingleParams {
                        address tokenIn;
                        address tokenOut;
                        uint24 fee;
                        address recipient;
                        uint256 deadline;
                        uint256 amountIn;
                        uint256 amountOutMinimum;
                        uint160 sqrtPriceLimitX96;
                    }
                    function exactInputSingle(ExactInputSingleParams memory params) external payable returns (uint256 amountOut);
                }
            );

            todo!();
        }
        PoolType::Aerodrome => {
            sol!(
                #[sol(rpc)]
                contract Aerodrome {
                    function getAmountOut(uint256 amountIn, address tokenIn) external view returns (uint256 amountOut);
                }
            );

            let contract = Aerodrome::new(
                swap_step.pool_address,
                provider,
            );


            let Aerodrome::getAmountOutReturn { amountOut } = contract
                .getAmountOut(amount_in, swap_step.token_in)
                .call()
                .await
                .unwrap();
            return amountOut;
        }
        PoolType::BalancerV2 => {
            sol!(
                #[sol(rpc)]
                contract BalancerV2Vault {
                    enum SwapKind { GIVEN_IN, GIVEN_OUT }

                    struct BatchSwapStep {
                        bytes32 poolId;
                        uint256 assetInIndex;
                        uint256 assetOutIndex;
                        uint256 amount;
                        bytes userData;
                    }

                    struct FundManagement {
                        address sender;
                        bool fromInternalBalance;
                        address payable recipient;
                        bool toInternalBalance;
                    }

                    function queryBatchSwap(
                        SwapKind kind,
                        BatchSwapStep[] memory swaps,
                        address[] memory assets,
                        FundManagement memory funds
                    ) external returns (int256[] memory);
                }
            );
        
            let vault_address = address!("BA12222222228d8Ba445958a75a0704d566BF2C8");
            let contract = BalancerV2Vault::new(vault_address, provider);
        
            let pool_id = get_balancer_pool_id(swap_step.pool_address).await;
            println!("Pool ID: {:#?}", pool_id);
        
            let single_swap = BalancerV2Vault::BatchSwapStep {
                poolId: pool_id,
                assetInIndex: U256::from(0),
                assetOutIndex: U256::from(1),
                amount: amount_in,
                userData: vec![].into(),
            };
        
            let fund_management = BalancerV2Vault::FundManagement {
                sender: Address::ZERO,
                fromInternalBalance: false,
                recipient: Address::ZERO,
                toInternalBalance: false,
            };

            let transaction = contract.queryBatchSwap(
                BalancerV2Vault::SwapKind::GIVEN_IN,
                vec![single_swap],
                vec![swap_step.token_in, swap_step.token_out],
                fund_management,
            ).into_transaction_request();
            let provider =
                ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
            
            let trace = provider.debug_trace_call(transaction, alloy::eips::BlockNumberOrTag::Latest, get_tracing_options().clone()).await.unwrap();
            println!("Trace: {:#?}", trace);

            // Process the trace to extract the amount out
            // This part depends on how you want to interpret the trace
            // For now, we'll return a placeholder value
            U256::from(0)
        }
        _ => todo!(),
    }
}


pub async fn swappath_to_flashquote(steps: Vec<SwapStep>) -> Vec<FlashQuoter::SwapStep> {
    steps.iter().map(|step| FlashQuoter::SwapStep {
        poolAddress: step.pool_address,
        tokenIn: step.token_in,
        tokenOut: step.token_out,
        protocol: step.as_u8(),
        fee: step.fee,
    }).collect()
}

// get a pool manaager that is populated wtih the pools from our address space
pub async fn pool_manager_with_pools(
    addresses: Vec<Address>,
) -> (Arc<PoolManager>, broadcast::Receiver<Event>) {
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

        println!("Pools: {:#?}", pools);
    let (pool_manager, mut reserve_receiver) =
        construct_pool_manager(pools.clone(), last_synced_block).await;
    (pool_manager, reserve_receiver)
}

// simulated profit based off of the call to the contract
async fn simulate_profit(steps: Vec<FlashSwap::SwapStep>) -> Option<U256> {
    let url = std::env::var("FULL").unwrap();
    let provider = ProviderBuilder::new().on_http(url.parse().unwrap());

    let anvil = Anvil::new()
        .fork(url)
        .port(9100_u16)
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
            //PoolType::UniswapV2,
            PoolType::BalancerV2,
            //PoolType::SushiSwapV2,
            //PoolType::UniswapV3,
            //PoolType::Slipstream,
            //PoolType::SushiSwapV3,
            //PoolType::PancakeSwapV2,
            //PoolType::PancakeSwapV3,
            //PoolType::Aerodrome,
            //PoolType::BaseSwapV3,
            //PoolType::BaseSwapV2
        ])
        .chain(pool_sync::Chain::Ethereum)
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


async fn get_balancer_pool_id(pool_address: Address) -> FixedBytes<32> {
    sol!(
        #[sol(rpc)]
        contract BalancerPool {
            function getPoolId() external view returns (bytes32);
        }
    );

    let provider = ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap());
    let contract = BalancerPool::new(pool_address, provider);

    let BalancerPool::getPoolIdReturn { _0: pool_id } = contract.getPoolId().call().await.unwrap();
    pool_id
}
