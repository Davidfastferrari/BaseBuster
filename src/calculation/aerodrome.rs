use alloy::primitives::{Address, U256};
use pool_sync::UniswapV2Pool;
use std::sync::RwLockReadGuard;
use super::Calculator;

pub fn aerodrome_out(
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
        let d = _d(x0, y);
        if d == U256::ZERO { return U256::ZERO }
        if k < xy {
            let mut dy = ((xy - k) * U256::from(1e18)) / d;
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
            let mut dy = ((k - xy) * U256::from(1e18)) / d;
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
