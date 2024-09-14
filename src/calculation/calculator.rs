use crate::pool_manager::PoolManager;
use crate::swap::*;
use alloy::primitives::{Address, U256};
use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use pool_sync::PoolType;
use crate::AMOUNT;
use std::sync::Arc;
use crate::cache::Cache;
use crate::db::RethDB;
use revm::db::CacheDB;
use crate::market_state::MarketState;

pub struct Calculator {
    pub market_state: Arc<MarketState>,
    pub cache: Arc<Cache>
}

impl Calculator {
    // Construct a new calculator with a reference to the market state
    pub async fn new(market_state: Arc<MarketState>) -> Self {
        let num_pools  =500;
        Self {
            market_state,
            cache: Arc::new(Cache::new(num_pools))
        }
    }


    // top level call to calculate the output of a swap_path
    // we acquire read access to the db in this top level call since we know the db will not change while calculator is executing
    #[inline]
    pub fn calculate_output(&self, path: &SwapPath) -> U256 {
        let mut amount = U256::from(AMOUNT);
        let db_guard = self.market_state.db.read().unwrap();

        // calculate the output for each swap step
        for step in &path.steps {
            amount = self.get_amount_out(amount, &step);

            // if we have a zero profit, some error in calcualtion, shortcircuit
            if amount == U256::ZERO {
                return U256::ZERO;
            }
        }
        amount
    }

    #[inline]
    fn get_amount_out(&self, amount_in: U256, swap_step: &SwapStep) -> U256 {
        let pool_address = swap_step.pool_address;

        // check to see if it is cached
        if let Some(cached_amount) = self.cache.get(amount_in, pool_address) {
            return cached_amount;
        }

        // not cached, compute the amount out and insert it into the cache
        let output_amount = self.compute_amount_out(
            amount_in, pool_address, swap_step.token_in, swap_step.token_out, swap_step.protocol
        );
        self.cache.set(amount_in, pool_address, output_amount);


        return output_amount;
    }

    // calculate the ratio for the pool
    pub fn compute_amount_out(
        &self, 
        input_amount: U256,
        pool_address: Address,
        token_in: Address,
        token_out: Address,
        pool_type: PoolType,
    ) -> U256 {
        let zero_to_one = self.pool_manager.zero_to_one(token_in, &pool_address);

        match pool_type {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 | PoolType::SwapBasedV2 => {
                let pool = self.pool_manager.get_v2pool(&pool_address);
                self.uniswap_v2_out(
                    input_amount,
                    pool.token0_reserves,
                    pool.token1_reserves,
                    zero_to_one,
                    U256::from(9970),
                )
            }
            PoolType::PancakeSwapV2 | PoolType::BaseSwapV2 | PoolType::DackieSwapV2 => {
                let pool = self.pool_manager.get_v2pool(&pool_address);
                self.uniswap_v2_out(
                    input_amount,
                    pool.token0_reserves,
                    pool.token1_reserves,
                    zero_to_one,
                    U256::from(9975),
                )
            }
            PoolType::AlienBaseV2 => {
                let pool = self.pool_manager.get_v2pool(&pool_address);
                self.uniswap_v2_out(
                    input_amount,
                    pool.token0_reserves,
                    pool.token1_reserves,
                    zero_to_one,
                    U256::from(9984),
                )
            }
            PoolType::UniswapV3
            | PoolType::SushiSwapV3
            | PoolType::BaseSwapV3
            | PoolType::Slipstream
            | PoolType::PancakeSwapV3
            | PoolType::AlienBaseV3
            | PoolType::SwapBasedV3
            | PoolType::DackieSwapV3 => self
                .uniswap_v3_out(input_amount, pool_address, zero_to_one).unwrap(),
            PoolType::Aerodrome => self.aerodrome_out(input_amount, token_in, pool_address),
            PoolType::MaverickV1 | PoolType::MaverickV2 => todo!(),
            PoolType::BalancerV2 => {
                self.balancer_v2_out(input_amount, token_in, token_out, pool_address)
            }
            PoolType::CurveTwoCrypto | PoolType::CurveTriCrypto => todo!(),
            _ => U256::ZERO,
        }
    }

    #[inline]
    pub fn invalidate_cache(&self, updated_pools: &[Address]) {
        for pool in updated_pools {
            self.cache.invalidate(*pool)
        }
    }
}
