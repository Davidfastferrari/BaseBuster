use alloy::primitives::{Address, I256, U128, U256};
use anyhow::Result;
use core::panic;
use pool_sync::{
    pools::pool_structure::{UniswapV2Pool, UniswapV3Pool},
    PoolType,
};
use std::sync::RwLockReadGuard;
use uniswap_v3_math::tick_math::{MAX_SQRT_RATIO, MAX_TICK, MIN_SQRT_RATIO, MIN_TICK};

pub const U256_1: U256 = U256::from_limbs([1, 0, 0, 0]);

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
            current_state.tick =
                uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(current_state.sqrt_price_x_96)?;
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
        let xy = _k(
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
            - _get_y(
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
        let k = _f(x0, y);
        if k < xy {
            let mut dy = ((xy - k) * U256::from(1e18)) / _d(x0, y);
            if dy == U256::ZERO {
                if k == xy {
                    return y;
                }
                if _k(x0, y + U256::from(1), stable, decimals0, decimals1) > xy {
                    return y + U256::from(1);
                }
                dy = U256::from(1);
            }
            y = y + dy;
        } else {
            let mut dy = ((k - xy) * U256::from(1e18)) / _d(x0, y);
            if dy == U256::ZERO {
                if k == xy || _f(x0, y - U256::from(1)) < xy {
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
