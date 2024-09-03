use super::balancer::balancer_v2_out;
use super::uniswap::{uniswap_v2_out, uniswap_v3_out};
use super::aerodrome::aerodrome_out;

use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::primitives::{address, Address, U128, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::sol;
use alloy::sol_types::{SolCall, SolValue};
use alloy::transports::http::{Client, Http};
use anyhow::Result;
use eyre::InstallError;
use crate::pool_manager::PoolManager;
use core::panic;
use std::time::Instant;
use pool_sync::PoolType;
use pool_sync::{UniswapV2Pool, UniswapV3Pool};
use revm::primitives::Bytecode;
use revm::Evm;
use revm::{
    db::{AlloyDB, CacheDB},
    primitives::{AccountInfo, ExecutionResult, TransactTo},
};
use crate::db::RethDB;
use crate::graph::SwapStep;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;


pub type AlloyCacheDB = CacheDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    MavQuoter,
    "src/abi/MavQuoter.json"
);

// Calculator used for calculatiing amoung out along steps
pub struct Calculator {
    provider: Arc<RootProvider<Http<Client>>>,
    pub db: RwLock<CacheDB<RethDB>>,
}

impl Calculator {
    pub async fn new() -> Self {
        let provider = Arc::new(
            ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap()),
        );

        // setup the db to our node
        let data_path = "/home/docker/volumes/eth-docker_reth-el-data/_data";
        let db = CacheDB::new(RethDB::new(data_path, None).unwrap());

        Self {
            provider,
            db: RwLock::new(db),
        }
    }

    pub fn get_amount_out(
        &self,
        amount_in: U256,
        pool_manager: &PoolManager,
        swap_step: &SwapStep
    ) -> U256 {
        let protocol = swap_step.protocol;
        let pool_address = swap_step.pool_address;
        let token_in = swap_step.token_in;
        let token_out = swap_step.token_out;


        let zero_to_one = pool_manager.zero_to_one(token_in, &pool_address);
        match protocol {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 | PoolType::PancakeSwapV2| PoolType::BaseSwapV2 |
            PoolType::AlienBaseV2 | PoolType::SwapBasedV2 | PoolType::DackieSwapV2 => {
                let v2_pool = pool_manager.get_v2pool(&pool_address);
                uniswap_v2_out(
                    amount_in,
                    v2_pool.token0_reserves,
                    v2_pool.token1_reserves,
                    zero_to_one,
                    protocol,
                )
            }
            PoolType::UniswapV3 | PoolType::SushiSwapV3 | PoolType::BaseSwapV3 | PoolType::Slipstream | PoolType::PancakeSwapV3 |
            PoolType::AlienBaseV3 | PoolType::SwapBasedV3 | PoolType::DackieSwapV3 => {
                let mut v3_pool = pool_manager.get_v3pool(&pool_address);
                uniswap_v3_out(amount_in, &mut v3_pool, zero_to_one).unwrap()
            }
            PoolType::Aerodrome => {
                let v2_pool = pool_manager.get_v2pool(&pool_address);
                aerodrome_out(amount_in, token_in, &v2_pool)
            }
            PoolType::MaverickV1 | PoolType::MaverickV2 => {
                let zero_for_one = pool_manager.zero_to_one(token_in, &pool_address);
                let tick_lim = if zero_for_one { i32::MAX } else { i32::MIN };
                self.maverick_v2_out(amount_in, pool_address, zero_for_one, tick_lim)
            }
            PoolType::BalancerV2 => {
                let balancer_pool = pool_manager.get_balancer_pool(&pool_address);
                println!("Balancer pool: {:#?}", balancer_pool);
                
                let token_in_index = balancer_pool.get_token_index(&token_in).unwrap();
                let token_out_index = balancer_pool.get_token_index(&token_out).unwrap();
                let start = Instant::now();
                let amount = balancer_v2_out(
                    amount_in,
                    &balancer_pool,
                    token_in_index,
                    token_out_index,
                );
                let end = Instant::now();
                println!("Balancer V2 out took {:?}", end.duration_since(start));
                amount
            }
            PoolType::CurveTwoCrypto => {
                let curve_pool = pool_manager.get_curve_two_pool(&pool_address);
                let (index_in, index_out) = if token_in == curve_pool.token0 {
                    (U256::ZERO, U256::from(1))
                } else {
                    (U256::from(1), U256::ZERO)
                };
                self.curve_out(index_in, index_out, amount_in, pool_address)
            }
            PoolType::CurveTriCrypto => {
                let curve_pool = pool_manager.get_curve_tri_pool(&pool_address);
                let index_in= U256::from(curve_pool.get_token_index(&token_in).unwrap());
                let index_out = U256::from(curve_pool.get_token_index(&token_out).unwrap());
                self.curve_out(index_in, index_out, amount_in, pool_address)
            }
        }
    }
}
