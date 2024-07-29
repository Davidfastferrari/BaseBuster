use alloy::primitives::{U256, U128, Address};


#[inline]
pub fn calculate_amount_out(
    reserves_in: U128,
    reserves_out: U128,
    amount_in: U256,
    zero_for_one: bool,
) -> Option<U256> {
    if reserves_in.is_zero() || reserves_out.is_zero() {
        return None;
    }

    let (reserves_in, reserves_out) = if zero_for_one {
        (reserves_in, reserves_out)
    } else {
        (reserves_out, reserves_in)
    };

    let amount_in_with_fee = amount_in.checked_mul(U256::from(997))?;
    let numerator = amount_in_with_fee.checked_mul(U256::from(reserves_out))?;
    let denominator = U256::from(reserves_in)
        .checked_mul(U256::from(1000))?
        .checked_add(amount_in_with_fee)?;

    if denominator.is_zero() {
        None
    } else {
        numerator.checked_div(denominator)
    }
}


pub fn calcualte_v2_out(amount_in: U256, pool_address: Address, token_in: Address) -> U256 {
    todo!()
}

pub fn calculate_v3_out(amount_in: U256, pool_address: Address, token_in: Address) -> U256 {
    todo!()
}


/* 

use alloy::primitives::{U256, U128};
use num_bigint::BigInt;
use num_traits::ToPrimitive;

/// Calculates the optimal input amount for a given pair of reserves
/// 
/// This function uses the formula derived from the constant product formula:
/// optimal_x = sqrt((r0 * r1 * 1000) / 997) - r0
/// where r0 is the input reserve and r1 is the output reserve
/// 
/// Parameters:
/// - reserves_in: The reserves of the input token
/// - reserves_out: The reserves of the output token
/// 
/// Returns:
/// - The optimal input amount as a U256
fn calculate_optimal_input(reserves_in: U128, reserves_out: U128) -> U256 {
    let r0 = BigInt::from(reserves_in.as_u128());
    let r1 = BigInt::from(reserves_out.as_u128());
    
    let numerator = r0.clone() * r1 * 1000;
    let denominator = BigInt::from(997);
    
    let sqrt_result = numerator.div(denominator).sqrt();
    
    let optimal_x = sqrt_result - r0;
    
    U256::from(optimal_x.to_u128().unwrap_or(u128::MAX))
}



/// Mirror router 'getAmountOut' calculation
pub fn get_amount_out(fee: u16, amount_in: u128, reserve_in: u128, reserve_out: u128) -> u128 {
    let amount_in_with_fee = U256::from(amount_in * (FEE_DENOMINATOR - fee as u128));
    // y0 = (y.x0)  / (x + x0)
    let amount_out = (U256::from(reserve_out) * amount_in_with_fee)
        / ((U256::from(reserve_in) * U256::from(FEE_DENOMINATOR)) + amount_in_with_fee);

    amount_out.as_u128()
}

/// Mirror router 'getAmountOut' calculation
pub fn get_amount_in(fee: u16, amount_out: u128, reserve_in: u128, reserve_out: u128) -> u128 {
    let numerator = reserve_in * amount_out * FEE_DENOMINATOR;
    let denominator = reserve_out - (amount_out * (FEE_DENOMINATOR - fee as u128));
    (numerator / denominator) + 1
}

/// `get_amount_out` with float (speed > precision)
pub fn get_amount_out_f(fee: u16, amount_in: u128, reserve_in: u128, reserve_out: u128) -> f64 {
    let amount_in_with_fee = (amount_in * (FEE_DENOMINATOR - fee as u128)) as f64;
    // y0 = (y.x0)  / (x + x0)
    let amount_out = ((reserve_out as f64) * amount_in_with_fee)
        / ((reserve_in as f64 * FEE_DENOMINATOR as f64) + amount_in_with_fee);

    amount_out
}

/// 2 ** 96
pub static X96: Lazy<U256> = Lazy::new(|| U256::from(2_u128.pow(96_u32)));
pub static Q96: Lazy<U256> = Lazy::new(|| U256::from(96));
static X96_F: Lazy<f64> = Lazy::new(|| 2_f64.powi(96));

pub fn get_next_sqrt_price_amount_0(
    liquidity: &U256,
    current_sqrt_p_x96: &U256,
    amount_0_in: &U256,
) -> U256 {
    let numerator_1 = liquidity << *Q96;
    let product = amount_0_in * current_sqrt_p_x96;
    let denominator = U512::from(numerator_1 + product);
    U256::try_from((U512::from(numerator_1) * U512::from(current_sqrt_p_x96)) / denominator)
        .expect("no overflow")
}

pub fn get_next_sqrt_price_amount_0_f(
    liquidity: f64,
    current_sqrt_p_x96: f64,
    amount_0_in: f64,
) -> f64 {
    let numerator_1 = liquidity * *X96_F;
    let product = amount_0_in * current_sqrt_p_x96;
    let denominator = numerator_1 + product;
    (numerator_1 * current_sqrt_p_x96) / denominator
}

pub fn get_next_sqrt_price_amount_1(
    liquidity: &U256,
    current_sqrt_p_x96: &U256,
    amount_1_in: &U256,
) -> U256 {
    let quotient = (amount_1_in << *Q96) / liquidity;
    current_sqrt_p_x96 + quotient
}

pub fn get_next_sqrt_price_amount_1_f(
    liquidity: f64,
    current_sqrt_p_x96: f64,
    amount_1_in: f64,
) -> f64 {
    let quotient = (amount_1_in * *X96_F) / liquidity;
    current_sqrt_p_x96 + quotient
}

pub fn get_next_sqrt_price_amount_0_output(
    liquidity: &U256,
    current_sqrt_p_x96: &U256,
    amount_out: &U256,
) -> U256 {
    let numerator_1 = liquidity << *Q96;
    let product = amount_out * current_sqrt_p_x96;
    let denominator = numerator_1 - product;

    ((U512::from(numerator_1) * U512::from(current_sqrt_p_x96)) / denominator)
        .try_into()
        .expect("fits 256")
}

pub fn get_next_sqrt_price_amount_1_output(
    liquidity: &U256,
    current_sqrt_p_x96: &U256,
    amount_out: &U256,
) -> U256 {
    // assume fits 160bits
    let quotient: U256 = ((U512::from(amount_out) << *Q96) / liquidity)
        .try_into()
        .expect("fits 256");
    current_sqrt_p_x96 - quotient
}

/// Get the amount0 delta between two prices
pub fn get_amount_0_delta_f(liquidity: f64, sqrt_ratio_aX96: f64, sqrt_ratio_bX96: f64) -> f64 {
    let (sqrt_ratio_aX96, sqrt_ratio_bX96) = if sqrt_ratio_aX96 > sqrt_ratio_bX96 {
        (sqrt_ratio_bX96, sqrt_ratio_aX96)
    } else {
        (sqrt_ratio_aX96, sqrt_ratio_bX96)
    };

    let liquidity = liquidity * *X96_F;
    let delta_sqrt_p = (sqrt_ratio_bX96 - sqrt_ratio_aX96).abs();

    ((liquidity * delta_sqrt_p) / sqrt_ratio_bX96) / sqrt_ratio_aX96
}

/// Get the amount0 delta between two prices
pub fn get_amount_0_delta(
    liquidity: &U256,
    sqrt_ratio_aX96: &U256,
    sqrt_ratio_bX96: &U256,
) -> U256 {
    let numerator_1 = liquidity << *Q96;
    let (sqrt_ratio_aX96, sqrt_ratio_bX96) = if sqrt_ratio_aX96 > sqrt_ratio_bX96 {
        (sqrt_ratio_bX96, sqrt_ratio_aX96)
    } else {
        (sqrt_ratio_aX96, sqrt_ratio_bX96)
    };
    let numerator_2 = sqrt_ratio_bX96 - sqrt_ratio_aX96;

    ((U512::from(numerator_1) * U512::from(numerator_2) / sqrt_ratio_bX96) / sqrt_ratio_aX96)
        .try_into()
        .expect("fits u256")
}

/// Get the amount1 delta between two prices
/// https://github.com/Uniswap/v3-core/blob/fc2107bd5709cdee6742d5164c1eb998566bcb75/contracts/libraries/SqrtPriceMath.sol#L182
pub fn get_amount_1_delta(
    liquidity: &U256,
    sqrt_ratio_aX96: &U256,
    sqrt_ratio_bX96: &U256,
) -> U256 {
    let delta_sqrt_p = sqrt_ratio_aX96.abs_diff(*sqrt_ratio_bX96);

    U256::try_from((U512::from(liquidity) * U512::from(delta_sqrt_p)) / U512::from(*X96))
        .expect("fits u256")
}

/// Get the amount1 delta between two prices
/// https://github.com/Uniswap/v3-core/blob/fc2107bd5709cdee6742d5164c1eb998566bcb75/contracts/libraries/SqrtPriceMath.sol#L182
pub fn get_amount_1_delta_f(liquidity: f64, sqrt_ratio_aX96: f64, sqrt_ratio_bX96: f64) -> f64 {
    (liquidity * (sqrt_ratio_bX96 - sqrt_ratio_aX96).abs()) / *X96_F
}

/// Get the amount out given some amount in
///
/// - `current_sqrt_p_x96` The √P.96
/// - `liquidity` The liquidity value
/// - `amount_in` the amount of tokens to input
///
/// Returns the amount of tokens output
pub fn get_amount_out(
    amount_in: u128,
    current_sqrt_p_x96: &U256,
    liquidity: &U256,
    fee_pips: u32,
    zero_for_one: bool,
) -> (U256, u128) {
    // calculate the expected price shift then return the amount out (i.e. price target is set exactly to required price shift)
    let amount_in_less_fee =
        U256::from(amount_in * (1_000_000_u32 - fee_pips) as u128) / U256::from(1_000_000_u128);
    if zero_for_one {
        let next_sqrt_p_x96 =
            get_next_sqrt_price_amount_0(liquidity, current_sqrt_p_x96, &amount_in_less_fee);
        (
            next_sqrt_p_x96,
            get_amount_1_delta(liquidity, &next_sqrt_p_x96, current_sqrt_p_x96).as_u128(), // TODO needs round up
        )
    } else {
        let next_sqrt_p_x96 =
            get_next_sqrt_price_amount_1(liquidity, current_sqrt_p_x96, &amount_in_less_fee);
        (
            next_sqrt_p_x96,
            get_amount_0_delta(liquidity, current_sqrt_p_x96, &next_sqrt_p_x96).as_u128(), // TODO: needs round up
        )
    }
}

pub fn get_amount_out_f(
    amount_in: u128,
    current_sqrt_p_x96: f64,
    liquidity: f64,
    fee_pips: u32,
    zero_for_one: bool,
) -> f64 {
    // calculate the expected price shift then return the amount out (i.e. price target is set exactly to required price shift)
    let amount_in_less_fee = (amount_in as f64 * (1_000_000_u32 - fee_pips) as f64) / 1_000_000_f64;
    if zero_for_one {
        let next_sqrt_p_x96 =
            get_next_sqrt_price_amount_0_f(liquidity, current_sqrt_p_x96, amount_in_less_fee);

        get_amount_1_delta_f(liquidity, next_sqrt_p_x96, current_sqrt_p_x96)
    } else {
        let next_sqrt_p_x96 =
            get_next_sqrt_price_amount_1_f(liquidity, current_sqrt_p_x96, amount_in_less_fee);

        get_amount_0_delta_f(liquidity, current_sqrt_p_x96, next_sqrt_p_x96)
    }
}

/// Get the amount in given some amount out
///
/// - `current_sqrt_p_x96` The √P.96
/// - `liquidity` The liquidity value
/// - `amount_out` the amount of tokens to output
///
/// Returns the amount of tokens to input and the new price
pub fn get_amount_in(
    amount_out: u128,
    current_sqrt_p_x96: &U256,
    liquidity: &U256,
    fee_pips: u32,
    zero_for_one: bool,
) -> (U256, u128) {
    // calculate the expected price shift then return the amount out (i.e. price target is set exactly to required price shift)
    let amount_out = &amount_out.into();
    if zero_for_one {
        // expect the order filled within one tick
        // trading in an amount of of token
        let next_sqrt_p_x96 =
            get_next_sqrt_price_amount_1_output(liquidity, current_sqrt_p_x96, amount_out);
        (
            next_sqrt_p_x96,
            ((get_amount_0_delta(liquidity, &next_sqrt_p_x96, current_sqrt_p_x96)
                * U256::from(1_000_000 - fee_pips))
                / U256::from(1_000_000))
            .as_u128(),
        )
    } else {
        // expect the order filled within one tick
        let next_sqrt_p_x96 =
            get_next_sqrt_price_amount_0_output(liquidity, current_sqrt_p_x96, amount_out);
        (
            next_sqrt_p_x96,
            ((get_amount_1_delta(liquidity, current_sqrt_p_x96, &next_sqrt_p_x96)
                * U256::from(1_000_000 - fee_pips))
                / U256::from(1_000_000))
            .as_u128(),
        )
    }
}
#[derive(Debug, PartialEq, DecodeStatic)]
pub struct UniswapV3Slot0 {
    pub sqrt_p_x96: U256,
    pub liquidity: u128,
}

#[inline(always)]
pub fn fee_from_path_bytes(buf: &[u8]) -> u32 {
    // OPTIMIZATION: nothing sensible should ever be longer than 2 ** 16 so we ignore the other bytes
    // ((unsafe { *buf.get_unchecked(0) } as u32) << 16) +
    ((unsafe { *buf.get_unchecked(1) } as u32) << 8) + (unsafe { *buf.get_unchecked(2) } as u32)
}
    */
