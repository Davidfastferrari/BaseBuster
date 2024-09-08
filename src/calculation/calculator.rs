use crate::pool_manager::PoolManager;
use crate::swap::*;
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use dashmap::DashMap;
use pool_sync::{Pool, PoolInfo, PoolType};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use crate::cache::Cache;
use crate::db::RethDB;
use revm::db::CacheDB;

pub struct Calculator {
    pub provider: Arc<RootProvider<Http<Client>>>,
    pub pool_manager: Arc<PoolManager>,
    //pub db: RwLock<CacheDB<RethDB>>,
    pub cache: Arc<Cache>
}

impl Calculator {
    pub async fn new(pool_manager: Arc<PoolManager>) -> Self {
        let provider = Arc::new(
            ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap()),
        );

        //let mut db = CacheDB::new(RethDB::new());

        //let num_pools = pool_manager.get_num_pools();
        let num_pools  =500;

        Self {
            provider,
            pool_manager,
            //db: RwLock::new(db),
            cache: Arc::new(Cache::new(num_pools))
        }
    }

    #[inline]
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

    #[inline]
    fn get_amount_out(&self, amount_in: U256, swap_step: &SwapStep) -> U256 {
        let pool_address = swap_step.pool_address;
        if let Some(cached_amount) = self.cache.get(amount_in, pool_address) {
            return cached_amount;
        }

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
