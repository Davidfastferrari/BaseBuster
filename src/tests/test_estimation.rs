// All offchain calculation tests
#[cfg(test)]
mod test_estimation {
    use super::super::offchain::offchain_quote;
    use super::super::utils::{construct_market, construct_pool_map, load_and_filter_pools};
    use crate::calculation::Calculator;
    use crate::events::Event;
    use crate::estimator::Estimator;
    use crate::swap::{SwapPath, SwapStep};
    use crate::graph::ArbGraph;

    use alloy::primitives::{address, U256};
    use pool_sync::{PoolType, Pool, UniswapV2Pool};

    macro_rules! test_pool_estimation {
        ($test_name:ident, $pool_type:ident) => {
            #[tokio::test(flavor = "multi_thread")]
            pub async fn $test_name() {
                dotenv::dotenv().ok();
                // load and filter pools
                let (pools, last_synced_block) = load_and_filter_pools(
                    vec![
                    PoolType::UniswapV2,
                    PoolType::UniswapV3
                    ]
                ).await;
                // get all the cycles
                let cycles = ArbGraph::generate_cycles(pools.clone()).await;
                // Pool map for references
                let pool_map = construct_pool_map(pools.clone());
                // init a market state with the new relevant pools
                let (market, address_rx) = construct_market(pools.clone(), last_synced_block).await;
                // construct the estimator
                let mut estimator = Estimator::new(market.clone());
                estimator.process_pools(pools.clone());

                let calculator = Calculator::new(market.clone());

                // while we get an update (new block), test onchain and offchain for all pools
                while let Ok(Event::PoolsTouched(addresses, _)) = address_rx.recv() {
                    estimator.update_rates(&addresses);
                    for path in &cycles {
                        //let pool = pool_map.get(&address).unwrap();
                        // get an offchain quote
                        //let offchain = offchain_quote(&pool, market.clone());
                        println!("{:#?}", path);
                        let offchain = calculator.calculate_output(&path.clone());
                        let est = estimator.estimate_output_amount(&path);
                        println!("offchain {:?}, estimation {:?}", offchain, est);

                    }

                    println!("Iteration finished");
                }
            }
        };
    }

    test_pool_estimation!(test_uniswapv2_est, UniswapV2);
    test_pool_estimation!(test_sushiswapv2_est, SushiSwapV2);
    test_pool_estimation!(test_pancakeswapv2_est, PancakeSwapV2);
    test_pool_estimation!(test_baseswapv2_est, BaseSwapV2);
    test_pool_estimation!(test_swapbasedv2_est, SwapBasedV2);
    test_pool_estimation!(test_alienbasev2_est, AlienBaseV2);
    test_pool_estimation!(test_dackieswapv2_est, DackieSwapV2);
    test_pool_estimation!(test_uniswapv3_est, UniswapV3);
    test_pool_estimation!(test_sushiswapv3_est, SushiSwapV3);
    test_pool_estimation!(test_pancakeswapv3_est, PancakeSwapV3);
    test_pool_estimation!(test_alienbasev3_est, AlienBaseV3);
    test_pool_estimation!(test_dackieswapv3_est, DackieSwapV3);
    test_pool_estimation!(test_swapbasedv3_est, SwapBasedV3);
    test_pool_estimation!(test_baseswapv3_est, BaseSwapV3);
    test_pool_estimation!(test_slipstream_est, Slipstream);
    test_pool_estimation!(test_aerodrome_est, Aerodrome);


    // 
    #[tokio::test(flavor = "multi_thread")]
    async fn test_calculated_to_estimated() {
        dotenv::dotenv().ok();

        // load market with two pools
        let uni_pool = UniswapV2Pool {
            address: address!("88A43bbDF9D098eEC7bCEda4e2494615dfD9bB9C"),
            token0: address!("4200000000000000000000000000000000000006"),
            token1: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
            token0_name: "WETH".to_string(),
            token1_name: "USDC".to_string(),
            token0_decimals: 18,
            token1_decimals: 6,
            token0_reserves: U256::from(325032740126871996707_u128),
            token1_reserves: U256::from(1014189875851_u128),
            stable: None,
            fee: None,
        };
        let sushi_pool = UniswapV2Pool {
                address: address!("2F8818D1B0f3e3E295440c1C0cDDf40aAA21fA87"),
                token0: address!("4200000000000000000000000000000000000006"),
                token1: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                token0_name: "WETH".to_string(),
                token1_name: "USDC".to_string(),
                token0_decimals: 18,
                token1_decimals: 6,
                token0_reserves: U256::from(324239280299976672116_u128),
                token1_reserves: U256::from(1016689282374_u128),
                stable: None,
                fee: None,
        };
        let pools = vec![Pool::UniswapV2(uni_pool), Pool::SushiSwapV2(sushi_pool)];

        // mock a swappath
        let path = SwapPath {
            steps: vec![
                SwapStep {
                    pool_address: address!("2F8818D1B0f3e3E295440c1C0cDDf40aAA21fA87"),
                    token_in: address!("4200000000000000000000000000000000000006"),
                    token_out: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                    protocol: PoolType::SushiSwapV2,
                    fee: 0,
                },
                SwapStep {
                    pool_address: address!("88A43bbDF9D098eEC7bCEda4e2494615dfD9bB9C"),
                    token_in: address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                    token_out: address!("4200000000000000000000000000000000000006"),
                    protocol: PoolType::UniswapV2,
                    fee: 0,
                },
            ],
            hash: 0,
        };

        let (market, address_rx) = construct_market(pools.clone(), 30_000_000).await;

        // calculator and estimator
        let calculator = Calculator::new(market.clone());
        let mut estimator = Estimator::new(market.clone());
        estimator.process_pools(pools.clone());

        let calc = calculator.calculate_output(&path);
        let est = estimator.estimate_output_amount(&path).unwrap();
        let is_profit = estimator.is_profitable(&path, U256::ZERO);
        println!("{} {} {}", calc, est, is_profit);
    }
}






















