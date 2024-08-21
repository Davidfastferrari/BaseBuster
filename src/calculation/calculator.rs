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
use crate::pool_manager::PoolManager;
use core::panic;
use pool_sync::PoolType;
use pool_sync::{UniswapV2Pool, UniswapV3Pool};
use revm::primitives::Bytecode;
use revm::Evm;
use revm::{
    db::{AlloyDB, CacheDB},
    primitives::{AccountInfo, ExecutionResult, TransactTo},
};
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
    db: RwLock<AlloyCacheDB>,
}

impl Calculator {
    pub async fn new() -> Self {
        let provider = Arc::new(
            ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap()),
        );

        let mut db = CacheDB::new(AlloyDB::new(provider.clone(), BlockId::latest()).unwrap());

        // insert the quoter
        let bytecode = Bytecode::new_raw(MavQuoter::DEPLOYED_BYTECODE.clone());
        let code_hash = bytecode.hash_slow();
        let account_info = AccountInfo {
            balance: U256::ZERO,
            nonce: 0_u64,
            code: Some(bytecode),
            code_hash,
        };
        let quoter_addr = address!("A5C381211A406b48A073E954e6949B0D49506bc0");
        db.insert_account_info(quoter_addr, account_info);

        Self {
            provider,
            db: RwLock::new(db),
        }
    }

    pub fn calculate_maverick_out(&self, amount_in: U256, pool: Address) -> U256 {
        // get write access to the db
        let mut db = self.db.write().unwrap();

        // construct our calldata
        let calldata = MavQuoter::getAmountOutCall {
            pool,
            zeroForOne: false,
            amountIn: amount_in,
        }
        .abi_encode();

        let mut evm = Evm::builder()
            .with_db(&mut *db)
            .modify_tx_env(|tx| {
                tx.caller = address!("0000000000000000000000000000000000000001");
                tx.transact_to =
                    TransactTo::Call(address!("A5C381211A406b48A073E954e6949B0D49506bc0"));
                tx.data = calldata.into();
                tx.value = U256::ZERO;
            })
            .build();

        // transact
        let ref_tx = evm.transact().unwrap();
        let result = ref_tx.result;

        let value = match result {
            ExecutionResult::Revert { output: value, .. } => value.to_vec(),
            _ => panic!("failed"),
        };

        // extract the output
        let last_64_bytes = &value[value.len() - 64..];

        let (a, b) = match <(i128, i128)>::abi_decode(last_64_bytes, false) {
            Ok((a, b)) => (a, b),
            Err(e) => panic!("failed to decode: {:?}", e),
        };
        print!("a: {:#?}, b: {:#?}", a, b);
        let value_out = std::cmp::min(a, b);
        let value_out = -value_out;
        println!("value out, {:#?}", value_out);

        U256::ZERO
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
            PoolType::UniswapV2
            | PoolType::SushiSwapV2
            | PoolType::PancakeSwapV2
            | PoolType::BaseSwapV2 => {
                let v2_pool = pool_manager.get_v2pool(&pool_address);
                //println!("V2 pool: {:#?}", v2_pool);
                //
                uniswap_v2_out(
                    amount_in,
                    v2_pool.token0_reserves,
                    v2_pool.token1_reserves,
                    zero_to_one,
                    protocol,
                )
            }
            PoolType::UniswapV3
            | PoolType::SushiSwapV3
            | PoolType::BaseSwapV3
            | PoolType::Slipstream
            | PoolType::PancakeSwapV3 => {
                let mut v3_pool = pool_manager.get_v3pool(&pool_address);
                //println!("V3 pool: {:#?}", v3_pool);
                uniswap_v3_out(amount_in, &mut v3_pool, zero_to_one).unwrap()
            }
            PoolType::Aerodrome => {
                let v2_pool = pool_manager.get_v2pool(&pool_address);
                //println!("V2 pool: {:#?}", v2_pool);
                aerodrome_out(amount_in, token_in, &v2_pool)
            }
            PoolType::MaverickV1 | PoolType::MaverickV2 => {
                //let zero_for_one = pool_manager.zero_to_one(token_in, &pool_address);
                //let tick_lim = if zero_for_one { i32::MAX } else { i32::MIN };
                todo!()
            }
            PoolType::BalancerV2 => {
                let balancer_pool = pool_manager.get_balancer_pool(&pool_address);
                let token_in_index = balancer_pool.get_token_index(&token_in).unwrap();
                let token_out_index = balancer_pool.get_token_index(&token_out).unwrap();
                balancer_v2_out(
                    amount_in,
                    &balancer_pool,
                    token_in_index,
                    token_out_index,
                )
            }
            PoolType::CurveTwoCrypto | PoolType::CurveTriCrypto => {
                //let curve_pool = pool_manager.get_curve_pool(&self.pool_address);
                //calculate_curve_out(amount_in, self.token_in, &curve_pool)
                todo!()
            }
            PoolType::AlienBase => {
                todo!()
            }
        }
    }
}

pub async fn init_account(
    address: Address,
    cache_db: &mut AlloyCacheDB,
    provider: &Arc<RootProvider<Http<Client>>>,
) {
    let bytecode = Bytecode::new_raw(provider.get_code_at(address).await.unwrap());
    let code_hash = bytecode.hash_slow();
    let account_info = AccountInfo {
        balance: U256::ZERO,
        nonce: 0_u64,
        code: Some(bytecode),
        code_hash,
    };
    cache_db.insert_account_info(address, account_info);
}
