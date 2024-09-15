use crate::swap::*;
use alloy::primitives::{Address, U256};
use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use revm::db::DatabaseRef;
use pool_sync::PoolType;
use crate::AMOUNT;
use std::sync::Arc;
use crate::cache::Cache;
use revm::db::CacheDB;
use crate::market_state::MarketState;

pub struct Calculator {
    pub market_state: Arc<MarketState>,
    pub cache: Arc<Cache>
}

impl Calculator {
    // construct a new calculator
    // contains the market state to access pool info and a cache for calculations
    pub async fn new(market_state: Arc<MarketState>) -> Self {
        Self {
            market_state,
            cache: Arc::new(Cache::new(500))
        }
    }

    // calculate the output amount 
    // we can get read access to the db since we know it will not change for duration of calculation
    #[inline]
    pub fn calculate_output(&self, path: &SwapPath) -> U256 {
        let mut amount = U256::from(AMOUNT);

        // for each step, calculate the amount out
        for step in &path.steps {
            amount = self.get_amount_out(amount, &step);
            if amount == U256::ZERO {
                return U256::ZERO;
            }
        }
        amount
    }


    // get the amount out for an individual swap
    #[inline]
    fn get_amount_out(&self, amount_in: U256, swap_step: &SwapStep) -> U256 {
        let pool_address = swap_step.pool_address;

        // check to see if we have a up to date cache
        if let Some(cached_amount) = self.cache.get(amount_in, pool_address) {
            return cached_amount;
        }

        // compute the output amount and then store it in a cache
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
        // get read access to the db
        let db_read = self.market_state.db.read().unwrap();
        let zero_to_one = db_read.zero_to_one(&pool_address, token_in).unwrap();
        //println!("{:?} {:?}, {}", pool_address, token_in, zero_to_one);
        

        match pool_type {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 | PoolType::SwapBasedV2 => {
                let (reserve0, reserve1) = db_read.get_reserves(&pool_address);
                self.uniswap_v2_out(
                    input_amount,
                    reserve0,
                    reserve1,
                    zero_to_one,
                    U256::from(9970),
                )
            }
            /* 
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
            */
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
