use crate::swap::*;
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use pool_sync::{Pool, PoolType, PoolInfo};
use crate::pool_manager::PoolManager;
use std::sync::Arc;
use dashmap::DashMap;

pub struct Calculator {
    pub provider: Arc<RootProvider<Http<Client>>>,
    pub pool_manager: Arc<PoolManager>,
    pub ratio_cache: DashMap<(Address, Address), U256>, // (pool address, token_in) -> ratio
}

impl Calculator {
    pub async fn new(pool_manager: Arc<PoolManager>) -> Self {
        let provider = Arc::new(
            ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap()),
        );

        let calculator = Self {
            provider,
            pool_manager,
            ratio_cache: DashMap::new(),
        };

        // initialize the cache with all of the pools
        let all_pools = calculator.pool_manager.get_all_pools();
        calculator.update_cache(&all_pools);
        calculator
    }


    // update the ratios for the given pools
    pub fn update_cache(&self, all_pools: &Vec<Address>) {
        for addr in all_pools {
            let pool = self.pool_manager.get_pool(&addr);
            let token0 = pool.token0_address();
            let token1 = pool.token1_address();

            let ratio_0_to_1 = self.calculate_ratio(&pool, token0, token1);
            let ratio_1_to_0 = self.calculate_ratio(&pool, token1, token0);

            self.ratio_cache.insert((*addr, token0), ratio_0_to_1);
            self.ratio_cache.insert((*addr, token1), ratio_1_to_0);
        }
    }

    // calculate the ratio for the pool
    fn calculate_ratio(&self, pool: &Pool, token_in: Address, token_out: Address) -> U256 {
        let input_amount = U256::from(1e18);
        let zero_to_one = self.pool_manager.zero_to_one(token_in, &pool.address());

        let output_amount = match pool.pool_type() {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 | PoolType::SwapBasedV2 => {
                let pool = self.pool_manager.get_v2pool(&pool.address());
                self.uniswap_v2_out(
                    input_amount, 
                    pool.token0_reserves, 
                    pool.token1_reserves, 
                    zero_to_one, 
                    U256::from(9970)
                )
            }
            PoolType::PancakeSwapV2 | PoolType::BaseSwapV2 | PoolType::DackieSwapV2 => {
                let pool = self.pool_manager.get_v2pool(&pool.address());
                self.uniswap_v2_out(
                    input_amount, 
                    pool.token0_reserves, 
                    pool.token1_reserves, 
                    zero_to_one, 
                    U256::from(9975)
                )
            }
            PoolType::UniswapV3 | PoolType::SushiSwapV3 | PoolType::BaseSwapV3 | 
            PoolType::Slipstream | PoolType::PancakeSwapV3 | PoolType::AlienBaseV3 | 
            PoolType::SwapBasedV3 | PoolType::DackieSwapV3 => {
                //self.calculate_v3_out(pool, input_amount, zero_to_one)
                todo!()
            }
            PoolType::Aerodrome => todo!(), // self.calculate_aerodrome_out(pool, input_amount, token_in),
            PoolType::MaverickV1 | PoolType::MaverickV2 => {
                todo!()
                //self.calculate_maverick_out(pool, input_amount, zero_to_one)
            }
            PoolType::BalancerV2 => todo!(), //self.calculate_balancer_out(pool, input_amount, token_in, token_out),
            PoolType::CurveTwoCrypto | PoolType::CurveTriCrypto => {
                //self.calculate_curve_out(pool, input_amount, token_in, token_out)
                todo!()
            }
            _ => U256::ZERO,
        };

        (output_amount * U256::from(1e18)) / input_amount
    }

    pub fn calculate_output(&self, path: &SwapPath) -> U256 {
        let mut amount = U256::from(1e16);
        for step in &path.steps {
            amount = self.get_amount_out(amount, &step);
            if amount == U256::ZERO {
                return U256::ZERO;
            }
        }
        amount
    }

    fn get_amount_out(&self, amount_in: U256, swap_step: &SwapStep) -> U256 {
        if let Some(ratio) = self.ratio_cache.get(&(swap_step.pool_address, swap_step.token_in)) {
            return (amount_in * *ratio) / U256::from(1e18);
        }
        panic!("not found");
    }
}