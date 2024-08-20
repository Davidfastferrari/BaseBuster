use alloy::network::Ethereum;
use alloy::primitives::address;
use alloy::providers::Provider;
use alloy::providers::ProviderBuilder;
use alloy::providers::RootProvider;
use alloy::sol;
use alloy::sol_types::SolValue;
use alloy::transports::http::{Client, Http};
use alloy::{
    eips::BlockId,
    node_bindings::GethInstance,
    primitives::{Address, I256, U128, U256},
};
use alloy_sol_types::{abi::token, SolCall, SolInterface};
use anyhow::Result;
use core::panic;
use std::str::FromStr;
use pool_sync::{BalancerV2Pool, PoolType, UniswapV2Pool, UniswapV3Pool};
use revm::primitives::Bytecode;
use revm::Evm;
use revm::{
    db::{states::cache, AlloyDB, CacheDB},
    primitives::{keccak256, AccountInfo, ExecutionResult, Output, TransactTo},
};
use std::ops::Div;
use std::sync::Arc;
use std::sync::RwLock;
use std::{cmp::min, sync::RwLockReadGuard, time::Instant};
use uniswap_v3_math::tick_math::{MAX_SQRT_RATIO, MAX_TICK, MIN_SQRT_RATIO, MIN_TICK};

pub type AlloyCacheDB = CacheDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>;

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    MavQuoter,
    "src/abi/MavQuoter.json"
);

pub const U256_1: U256 = U256::from_limbs([1, 0, 0, 0]);

const MAX_IN_RATIO: U256 = U256::from_limbs([999999, 0, 0, 0]);
// Balancer V2 specific
pub const BONE: U256 = U256::from_limbs([0xDE0B6B3A7640000, 0, 0, 0]);
pub const U256_2: U256 = U256::from_limbs([2, 0, 0, 0]);
pub const ONE: U256 = U256::from_limbs([1, 0, 0, 0]);

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    contract MaverickQuoter {
        function calculateSwap(
            address pool,
            uint128 amount,
            bool tokenAIn,
            bool exactOutput,
            int32 tickLimit
        ) external returns (uint256 amountIn, uint256 amountOut, uint256 gasEstimate);
    }
);
pub struct CurrentState {
    amount_specified_remaining: I256,
    amount_calculated: I256,
    sqrt_price_x_96: U256,
    tick: i32,
    liquidity: u128,
}

#[derive(Default)]
pub struct StepComputations {
    pub sqrt_price_start_x_96: U256,
    pub tick_next: i32,
    pub initialized: bool,
    pub sqrt_price_next_x96: U256,
    pub amount_in: U256,
    pub amount_out: U256,
    pub fee_amount: U256,
}

pub struct Calculator {
    provider: Arc<RootProvider<Http<Client>>>,
    db: RwLock<AlloyCacheDB>,
}

impl Calculator {

    const ONE_18: I256 = I256::from_str("1000000000000000000").unwrap();
    const ONE_20: I256 = I256::from_str("100000000000000000000").unwrap();
    const ONE_36: I256 = I256::from_str("1000000000000000000000000000000000000").unwrap();

    const MAX_NATURAL_EXPONENT: I256 = I256::from_str("130000000000000000000").unwrap();
    const MIN_NATURAL_EXPONENT: I256 = I256::from_str("-41000000000000000000").unwrap();

    const LN_36_LOWER_BOUND: I256 = I256::from_str("999999999999999999").unwrap();
    const LN_36_UPPER_BOUND: I256 = I256::from_str("1000000000000000001").unwrap();

    const MILD_EXPONENT_BOUND: U256 = U256::from_str("2854495385411919762116571938898990272765493248").unwrap();

    pub async fn new() -> Self {
        let provider = Arc::new(
            ProviderBuilder::new().on_http(std::env::var("FULL").unwrap().parse().unwrap()),
        );

        let alloy_db = AlloyDB::new(provider.clone(), BlockId::latest()).unwrap();

        let mut db = CacheDB::new(alloy_db);

        // insert account info
        //let maverick_quoter = address!("b40AfdB85a07f37aE217E7D6462e609900dD8D7A");
        //init_account(maverick_quoter, &mut db, &provider).await;

        let bytecode = Bytecode::new_raw(MavQuoter::BYTECODE.clone());
        let code_hash = bytecode.hash_slow();
        let account_info = AccountInfo {
            balance: U256::ZERO,
            nonce: 0_u64,
            code: Some(bytecode),
            code_hash,
        };
        let quoter_addr = address!("A5C381211A406b48A073E954e6949B0D49506bc0");
        db.insert_account_info(quoter_addr, account_info);

        let pool_address = address!("5b6a0771c752e35b2ca2aff4f22a66b1598a2bc5");
        let token1 = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        let token2 = address!("dac17f958d2ee523a2206206994597c13d831ec7");
        init_account(token1, &mut db, &provider).await;
        init_account(token2, &mut db, &provider).await;

        let mocked_balance = U256::MAX.div(U256::from(2));
        insert_mapping_storage_slot(token1, U256::from(0), pool_address, mocked_balance, &mut db)
            .await
            .unwrap();
        insert_mapping_storage_slot(token2, U256::from(0), pool_address, mocked_balance, &mut db)
            .await
            .unwrap();

        //let pool = address!("5b6a0771c752e35b2ca2aff4f22a66b1598a2bc5");
        //init_account(pool, &mut db, &provider).await;
        //let token = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        //init_account(token, &mut db, &provider).await;
        //let token = address!("dac17f958d2ee523a2206206994597c13d831ec7/");
        //init_account(token, &mut db, &provider).await;

        // pool

        // outtoken

        Self {
            provider,
            db: RwLock::new(db),
        }
    }
    // Calcualte the amount out for a uniswapv2 swap
    #[inline]
    pub fn calculate_v2_out(
        amount_in: U256,
        reserve0: U128,
        reserve1: U128,
        zero_to_one: bool,
        pool_type: PoolType,
    ) -> U256 {
        let (fee, scalar) = match pool_type {
            PoolType::UniswapV2 | PoolType::SushiSwapV2 => (U256::from(997), U256::from(1000)),
            PoolType::PancakeSwapV2 | PoolType::BaseSwapV2 => (U256::from(9975), U256::from(10000)),
            _ => panic!("Invalid pool type"),
        };

        let reserve0 = U256::from(reserve0);
        let reserve1 = U256::from(reserve1);

        let (reserve0, reserve1) = if zero_to_one {
            (reserve0, reserve1)
        } else {
            (reserve1, reserve0)
        };

        let amount_in_with_fee = amount_in * fee;
        let numerator = amount_in_with_fee * reserve1;
        let denominator = reserve0 * scalar + amount_in_with_fee;
        numerator / denominator
    }

    // calculate the amount out for a uniswapv3 swap
    pub fn calculate_v3_out(
        amount_in: U256,
        pool: &mut RwLockReadGuard<UniswapV3Pool>,
        zero_to_one: bool,
    ) -> Result<U256> {
        if amount_in.is_zero() {
            return Ok(U256::ZERO);
        }

        // Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending on zero_for_one
        let sqrt_price_limit_x_96 = if zero_to_one {
            MIN_SQRT_RATIO + U256_1
        } else {
            MAX_SQRT_RATIO - U256_1
        };

        // Initialize a mutable state state struct to hold the dynamic simulated state of the pool
        let mut current_state = CurrentState {
            sqrt_price_x_96: pool.sqrt_price, //Active price on the pool
            amount_calculated: I256::ZERO,    //Amount of token_out that has been calculated
            amount_specified_remaining: I256::from_raw(amount_in), //Amount of token_in that has not been swapped
            tick: pool.tick,
            liquidity: pool.liquidity, //Current available liquidity in the tick range
        };
        while current_state.amount_specified_remaining != I256::ZERO
            && current_state.sqrt_price_x_96 != sqrt_price_limit_x_96
        {
            // Initialize a new step struct to hold the dynamic state of the pool at each step
            let mut step = StepComputations {
                // Set the sqrt_price_start_x_96 to the current sqrt_price_x_96
                sqrt_price_start_x_96: current_state.sqrt_price_x_96,
                ..Default::default()
            };

            // Get the next tick from the current tick
            (step.tick_next, step.initialized) =
                uniswap_v3_math::tick_bitmap::next_initialized_tick_within_one_word(
                    &pool.tick_bitmap,
                    current_state.tick,
                    pool.tick_spacing,
                    zero_to_one,
                )?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
            // Note: this could be removed as we are clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            // Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 =
                uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)?;

            // Target spot price
            let swap_target_sqrt_ratio = if zero_to_one {
                if step.sqrt_price_next_x96 < sqrt_price_limit_x_96 {
                    sqrt_price_limit_x_96
                } else {
                    step.sqrt_price_next_x96
                }
            } else if step.sqrt_price_next_x96 > sqrt_price_limit_x_96 {
                sqrt_price_limit_x_96
            } else {
                step.sqrt_price_next_x96
            };

            // Compute swap step and update the current state
            (
                current_state.sqrt_price_x_96,
                step.amount_in,
                step.amount_out,
                step.fee_amount,
            ) = uniswap_v3_math::swap_math::compute_swap_step(
                current_state.sqrt_price_x_96,
                swap_target_sqrt_ratio,
                current_state.liquidity,
                current_state.amount_specified_remaining,
                pool.fee,
            )?;

            // Decrement the amount remaining to be swapped and amount received from the step
            current_state.amount_specified_remaining = current_state
                .amount_specified_remaining
                .overflowing_sub(I256::from_raw(
                    step.amount_in.overflowing_add(step.fee_amount).0,
                ))
                .0;

            current_state.amount_calculated -= I256::from_raw(step.amount_out);

            // If the price moved all the way to the next price, recompute the liquidity change for the next iteration
            if current_state.sqrt_price_x_96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net = if let Some(info) = pool.ticks.get(&step.tick_next) {
                        info.liquidity_net
                    } else {
                        0
                    };

                    // we are on a tick boundary, and the next tick is initialized, so we must charge a protocol fee
                    if zero_to_one {
                        liquidity_net = -liquidity_net;
                    }

                    current_state.liquidity = if liquidity_net < 0 {
                        if current_state.liquidity < (-liquidity_net as u128) {
                            return Ok(U256::ZERO); // this orignally returned an error
                        } else {
                            current_state.liquidity - (-liquidity_net as u128)
                        }
                    } else {
                        current_state.liquidity + (liquidity_net as u128)
                    };
                }
                // Increment the current tick
                current_state.tick = if zero_to_one {
                    step.tick_next.wrapping_sub(1)
                } else {
                    step.tick_next
                }
                // If the current_state sqrt price is not equal to the step sqrt price, then we are not on the same tick.
                // Update the current_state.tick to the tick at the current_state.sqrt_price_x_96
            } else if current_state.sqrt_price_x_96 != step.sqrt_price_start_x_96 {
                current_state.tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(
                    current_state.sqrt_price_x_96,
                )?;
            }
        }

        let amount_out = (-current_state.amount_calculated).into_raw();

        Ok(amount_out)
    }

    pub fn calculate_aerodrome_out(
        amount_in: U256,
        token_in: Address,
        pool: &RwLockReadGuard<UniswapV2Pool>,
    ) -> U256 {
        let (mut _reserve0, mut _reserve1) = (
            U256::from(pool.token0_reserves),
            U256::from(pool.token1_reserves),
        );
        let mut amount_in = amount_in;
        amount_in -= (amount_in * pool.fee.unwrap()) / U256::from(10000);

        let token0_decimals = U256::from(10).pow(U256::from(pool.token0_decimals));
        let token1_decimals = U256::from(10).pow(U256::from(pool.token1_decimals));
        let stable = pool.stable.unwrap();
        if stable {
            let xy = Self::_k(
                _reserve0,
                _reserve1,
                stable,
                token0_decimals,
                token1_decimals,
            );
            _reserve0 = (_reserve0 * U256::from(1e18)) / token0_decimals;
            _reserve1 = (_reserve1 * U256::from(1e18)) / token1_decimals;
            let (reserve_a, reserve_b) = if token_in == pool.token0 {
                (_reserve0, _reserve1)
            } else {
                (_reserve1, _reserve0)
            };
            amount_in = if token_in == pool.token0 {
                (amount_in * U256::from(1e18)) / token0_decimals
            } else {
                (amount_in * U256::from(1e18)) / token1_decimals
            };
            let y = reserve_b
                - Self::_get_y(
                    amount_in + reserve_a,
                    xy,
                    reserve_b,
                    stable,
                    token0_decimals,
                    token1_decimals,
                );
            if token_in == pool.token0 {
                return (y * token1_decimals) / U256::from(1e18);
            } else {
                return (y * token0_decimals) / U256::from(1e18);
            }
        } else {
            let (reserve_a, reserve_b) = if token_in == pool.token0 {
                (_reserve0, _reserve1)
            } else {
                (_reserve1, _reserve0)
            };
            return (amount_in * reserve_b) / (reserve_a + amount_in);
        }
    }

    fn _k(x: U256, y: U256, stable: bool, decimals0: U256, decimals1: U256) -> U256 {
        if stable {
            let _x = (x * U256::from(1e18)) / decimals0;
            let _y = (y * U256::from(1e18)) / decimals1;
            let _a = (_x * _y) / U256::from(1e18);
            let _b = (_x * _x) / U256::from(1e18) + (_y * _y) / U256::from(1e18);
            return (_a * _b) / U256::from(1e18);
        } else {
            return x * y;
        }
    }

    fn _get_y(x0: U256, xy: U256, y: U256, stable: bool, decimals0: U256, decimals1: U256) -> U256 {
        let mut y = y;
        for _ in 0..255 {
            let k = Self::_f(x0, y);
            if k < xy {
                let mut dy = ((xy - k) * U256::from(1e18)) / Self::_d(x0, y);
                if dy == U256::ZERO {
                    if k == xy {
                        return y;
                    }
                    if Self::_k(x0, y + U256::from(1), stable, decimals0, decimals1) > xy {
                        return y + U256::from(1);
                    }
                    dy = U256::from(1);
                }
                y = y + dy;
            } else {
                let mut dy = ((k - xy) * U256::from(1e18)) / Self::_d(x0, y);
                if dy == U256::ZERO {
                    if k == xy || Self::_f(x0, y - U256::from(1)) < xy {
                        return y;
                    }
                    dy = U256::from(1);
                }
                y = y - dy;
            }
        }
        U256::ZERO
    }

    fn _f(x0: U256, y: U256) -> U256 {
        let _a = (x0 * y) / U256::from(1e18);
        let _b = (x0 * x0) / U256::from(1e18) + (y * y) / U256::from(1e18);
        return (_a * _b) / U256::from(1e18);
    }

    fn _d(x0: U256, y: U256) -> U256 {
        return U256::from(3) * x0 * ((y * y) / U256::from(1e18)) / U256::from(1e18)
            + (((x0 * x0) / U256::from(1e18)) * x0) / U256::from(1e18);
    }

    pub fn calculate_maverick_out(
        &self,
        amount_in: U256,
        pool: Address,
        zero_for_one: bool,
        tick_lim: i32,
    ) -> U256 {
        println!("Start");
        /*
            let calldata = MaverickQuoter::calculateSwapCall {
                pool,
                amount: amount_in.to::<u128>(),
                tokenAIn: zero_for_one,
                exactOutput: false,
                tickLimit: tick_lim
            }.abi_encode();
        */

        let mut db = self.db.write().unwrap();
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

        let calldata = MavQuoter::getAmountOutCall {
            pool,
            zeroForOne: false,
            amountIn: amount_in,
        }
        .abi_encode();
        let quoter_addr = address!("A5C381211A406b48A073E954e6949B0D49506bc0");

        let mut evm = Evm::builder()
            .with_db(&mut *db)
            .modify_tx_env(|tx| {
                tx.caller = address!("0000000000000000000000000000000000000001");
                tx.transact_to = TransactTo::Call(quoter_addr);
                tx.data = calldata.into();
                tx.value = U256::ZERO;
            })
            .build();

        let start = Instant::now();
        let ref_tx = evm.transact().unwrap();
        let end = start.elapsed();
        //println!("result: {:#?}", ref_tx);
        let result = ref_tx.result;

        let value = match result {
            ExecutionResult::Revert { output: value, .. } => value.to_vec(),
            _ => panic!("failed"),
        };

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
        /*
        println!("result: {:#?}", ref_tx.clone());

        let value = match result {
            ExecutionResult::Success {
                output: Output::Call(value),
                ..
            } => value,
            result => panic!("failed"),
        };


        let (_, amount_out, _) = <(U256, U256, U256)>::abi_decode(&value, false).unwrap();
        let end = start.elapsed();
        println!("time elapsed: {:?}", end);
        amount_out
        */

        //amountOut
    }

    pub fn calculate_balancer_v2_out(
        &self,
        amount_in: U256,
        pool: &RwLockReadGuard<BalancerV2Pool>,
        token_in_index: usize,
        token_out_index: usize,
    ) -> U256 {
        let balance_in = pool.balances[token_in_index];
        let balance_out = pool.balances[token_out_index];
        let weight_in = pool.weights[token_in_index];
        let weight_out = pool.weights[token_out_index];
        let swap_fee_percentage = pool.swap_fee;

        /*
        let scaling_factor = 18 - pool.token0_decimals as i8;
        let scaled_amount_in = Self::scaled(amount_in, -scaling_factor);
        let scaled_amount_in_without_fee = Self::sub(
            scaled_amount_in,
            Self::mul_up(scaled_amount_in, swap_fee_percentage),
        );
        let amount_in = Self::scaled(scaled_amount_in_without_fee, scaling_factor);
        println!("asdf {:?}", amount_in);
        */

        let denominator = Self::add(balance_in, amount_in);
        let base = Self::div_up(balance_in, denominator);
        let exponent = Self::div_down(weight_in, weight_out);
        let power = Self::pow_up(base, exponent);

        Self::mul_down(balance_out, Self::complement(power))
    }


    fn div_up(a: U256, b: U256) -> U256 {
        let one = U256::from(1e18);
        if a == U256::ZERO {
            return U256::ZERO;
        }
        let a_inflated = a * one;
        ((a_inflated - U256::from(1)) / b) + U256::from(1)
    }

    fn div_down(a: U256, b: U256) -> U256 {
        let one = U256::from(1e18);
        if a == U256::ZERO {
            return U256::ZERO;
        }
        let a_inflated = a * one;
        a_inflated / b
    }

    fn scaled(value: U256, decimals: i8) -> U256 {
        value * U256::from(10).pow(U256::from(decimals))
    }

    fn pow_up(x: U256, y: U256) -> U256 {
        let MAX_POW_RELATIVE_ERROR = U256::from(10000);
        let one = U256::from(1e18);
        let two = one * U256::from(2);
        let four = one * U256::from(4);
        if y == one {
            return x;
        } else if y == two {
            println!("ran here");
            return Self::mul_up(x, x);
        } else if y == four {
            let square = Self::mul_up(x, x);
            return Self::mul_up(square, square);
        } else {
            let raw = x.pow(y); // this raw could be wrong
            println!("the raw {:?}", raw);

            let max_error = Self::add(Self::mul_up(raw, MAX_POW_RELATIVE_ERROR), U256::from(1));
            return Self::add(raw, max_error);
        }
    }


    fn sub(a: U256, b: U256) -> U256 {
        a - b
    }

    fn add(a: U256, b: U256) -> U256 {
        a + b
    }

    fn mul_down(a: U256, b: U256) -> U256 {
        let one = U256::from(1e18);
        let product = a * b;
        //if a != U256::ZERO || product / a != b {
        //return U256::ZERO;
        //};
        product / one
    }

    fn mul_up(a: U256, b: U256) -> U256 {
        let one = U256::from(1e18);
        let product = a * b;
        if a != U256::ZERO || product / a != b {
            return U256::ZERO;
        };

        if product == U256::ZERO {
            U256::ZERO
        } else {
            ((product - U256::from(1)) / one) + U256::from(1)
        }
    }

    fn complement(x: U256) -> U256 {
        let one = U256::from(1e18);
        if x < one {
            one - x
        } else {
            U256::ZERO
        }

        /*
        if x < one {
            one - x
        } else {
            U256::ZERO
        }
        */
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

pub async fn insert_mapping_storage_slot(
    contract: Address,
    slot: U256,
    slot_address: Address,
    value: U256,
    cache_db: &mut AlloyCacheDB,
) -> Result<()> {
    let hashed_balance_slot = keccak256((slot_address, slot).abi_encode());

    cache_db.insert_account_storage(contract, hashed_balance_slot.into(), value)?;
    Ok(())
}
