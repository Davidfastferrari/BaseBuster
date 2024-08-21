use alloy::primitives::{U256, I256};
use pool_sync::BalancerV2Pool;
use std::sync::RwLockReadGuard;
use alloy::primitives::{Signed, Uint};
use std::str::FromStr;


pub fn balancer_v2_out(
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

    let scaling_factor = 18 - pool.token0_decimals as i8;
    let scaled_amount_in = scaled(amount_in, -scaling_factor);
    let scaled_amount_in_without_fee = sub(
        scaled_amount_in,
        mul_up(scaled_amount_in, swap_fee_percentage),
    );
    let amount_in = scaled(scaled_amount_in_without_fee, scaling_factor);

    let denominator = add(balance_in, amount_in);
    let base = div_up(balance_in, denominator);
    let exponent = div_down(weight_in, weight_out);
    let power = pow_up(base, exponent);

    mul_down(balance_out, complement(power))
}

fn scaled(value: U256, decimals: i8) -> U256 {
    value * U256::from(10).pow(U256::from(decimals))
}

fn add(a: U256, b: U256) -> U256 {
    a + b
}

fn sub(a: U256, b: U256) -> U256 {
    a - b
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

fn mul_down(a: U256, b: U256) -> U256 {
    let one = U256::from(1e18);
    let product = a * b;
    //if a != U256::ZERO || product / a != b {
    //return U256::ZERO;
    //};
    product / one
}

fn pow_up(x: U256, y: U256) -> U256 {
    let max_pow_relative_error = U256::from(10000);
    let one = U256::from(1e18);
    let two = one * U256::from(2);
    let four = one * U256::from(4);
    if y == one {
        x
    } else if y == two {
        mul_up(x, x)
    } else if y == four {
        let square = mul_up(x, x);
        return mul_up(square, square);
    } else {
        let raw = x.pow(y); // this raw could be wrong
        println!("the raw {:?}", raw);

        let max_error = add(mul_up(raw, max_pow_relative_error), U256::from(1));
        return add(raw, max_error);
    }
}


fn complement(x: U256) -> U256 {
    let one = U256::from(1e18);
    if x < one {
        one - x
    } else {
        U256::ZERO
    }
}

struct LogExpMath;
impl LogExpMath {

    fn pow(x: U256, y: U256) -> U256 {
        if y == U256::ZERO  {
            return Self::one_18();
        }

        if x == U256::ZERO {
            return U256::ZERO;
        }

        let x_int256 = I256::from_raw(x);
        let y_int256 = I256::from_raw(y);


        let mut logx_times_y = I256::ZERO;

        let ln_36_lower_bound = Self::ln_36_lower_bound();

        if (ln_36_lower_bound < x_int256) {
        }
        U256::ZERO

    }


    fn x0() -> U256 { U256::from(128000000000000000000_u128) }
    fn a0() -> U256 { U256::from_str("38877084059945950922200000000000000000000000000000000000").unwrap() }
    fn x1() -> U256 { U256::from(64000000000000000000_u128) }
    fn a1() -> U256 { U256::from(6235149080811616882910000000_u128) }

    // 20 decimal constants
    fn x2() -> U256 { U256::from(3200000000000000000000_u128) } // 2^5
    fn a2() -> U256 { U256::from_str("7896296018268069516100000000000000").unwrap() } // e^(x2)
    fn x3() -> U256 { U256::from(1600000000000000000000_u128) } // 2^4
    fn a3() -> U256 { U256::from(888611052050787263676000000_u128) } // e^(x3)
    fn x4() -> U256 { U256::from(800000000000000000000_u128) } // 2^3
    fn a4() -> U256 { U256::from(298095798704172827474000_u128) } // e^(x4)
    fn x5() -> U256 { U256::from(400000000000000000000_u128) } // 2^2
    fn a5() -> U256 { U256::from(5459815003314423907810_u128) } // e^(x5)
    fn x6() -> U256 { U256::from(200000000000000000000_u128) } // 2^1
    fn a6() -> U256 { U256::from(738905609893065022723_u128) } // e^(x6)
    fn x7() -> U256 { U256::from(100000000000000000000_u128) } // 2^0
    fn a7() -> U256 { U256::from(271828182845904523536_u128) } // e^(x7)
    fn x8() -> U256 { U256::from(50000000000000000000_u128) } // 2^-1
    fn a8() -> U256 { U256::from(164872127070012814685_u128) } // e^(x8)
    fn x9() -> U256 { U256::from(25000000000000000000_u128) } // 2^-2
    fn a9() -> U256 { U256::from(128402541668774148407_u128) } // e^(x9)
    fn x10() -> U256 { U256::from(12500000000000000000_u128) } // 2^-3
    fn a10() -> U256 { U256::from(113314845306682631683_u128) } // e^(x10)
    fn x11() -> U256 { U256::from(6250000000000000000_u128) } // 2^-4
    fn a11() -> U256 { U256::from(106449445891785942956_u128) } // e^(x11)

    // You might want to add these utility constants as well
    fn one_18() -> U256 { U256::from(1_000_000_000_000_000_000_u128) }
    fn one_20() -> U256 { U256::from(100_000_000_000_000_000_000_u128) }
    fn one_36() -> U256 { U256::from_str("1000000000000000000000000000000000000").unwrap() }


    fn ln_36_lower_bound() -> I256 { I256::from_raw(one_18() - U256::from(1e17)) }
}






























