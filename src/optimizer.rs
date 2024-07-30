use alloy::primitives::Address;
use log::info;
use num_bigint::BigUint;
use num_traits::{FromPrimitive, One, ToPrimitive, Zero};
use std::str::FromStr;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::events::*;

use alloy::primitives::{U128, U256};
use pool_sync::Pool;

pub async fn optimize_paths(opt_sender: Sender<Event>, mut arb_receiver: Receiver<Event>) {
    /* 
    while let Ok(Event::NewPath(arb_path)) = arb_receiver.recv().await {

        let path = arb_path.path;
        let reserves = arb_path.reserves;
        let optimized = optimize_amount_in(path.clone(), reserves);

        //println!("Path {:#?}, Optimal input: {:#?}", path, optimized);
        let optimized_path = OptPath {
            path,
            optimal_input: optimized.0,
        };
        opt_sender
            .send(Event::OptimizedPath(optimized_path))
            .unwrap();
    }
    */
}

/* 
fn optimize_amount_in(path: Vec<Address>, reserves: Vec<(U128, U128)>) -> (U256, U256) {
    let mut low = U256::from(0);
    let mut high = U256::from(1e19);
    let mut best_input = U256::from(0);
    let mut best_output = U256::from(0);

    // Perform binary search to find the optimal input amount
    while low <= high {
        let mid = (low + high) / U256::from(2);
        let mut current_amount = mid;

        // Simulate the swaps along the path
        for (i, (reserve_in, reserve_out)) in reserves.iter().enumerate() {
            let zero_for_one = path[i] < path[i + 1];
            if let Some(amount_out) =
                calculate_amount_out(*reserve_in, *reserve_out, current_amount, zero_for_one)
            {
                current_amount = amount_out;
            } else {
                // If the swap fails, set the output to zero
                current_amount = U256::from(0);
                break;
            }
        }

        // Update the best input and output if we found a better result
        if current_amount > best_output {
            best_input = mid;
            best_output = current_amount;
            low = mid + U256::from(1);
        } else {
            high = mid - U256::from(1);
        }
    }

    (best_input, best_output)
}

pub fn calculate_optimal_input(reserves: Vec<(BigUint, BigUint)>) -> Option<BigUint> {
    let fee_denominator = BigUint::from(1_000_000u32);
    let r = BigUint::from(997000u32);

    let (e_a, e_b) = calculate_virtual_pool_params(reserves, &r, &fee_denominator);

    let sqrt_e_a_e_b_r = integer_sqrt(&(&e_a * &e_b * &r / &fee_denominator));

    // Check if sqrt_e_a_e_b_r is greater than e_a before subtracting
    if sqrt_e_a_e_b_r > e_a {
        Some((&sqrt_e_a_e_b_r - &e_a) * &fee_denominator / &r)
    } else {
        None // No profitable arbitrage opportunity
    }
}

fn calculate_virtual_pool_params(
    reserves: Vec<(BigUint, BigUint)>,
    r: &BigUint,
    fee_denominator: &BigUint,
) -> (BigUint, BigUint) {
    let mut e_0 = reserves[0].0.clone();
    let mut e_1 = reserves[0].1.clone();

    for pair in reserves.iter().skip(1) {
        let r_0 = e_0;
        let r_1 = e_1;
        let r_1_prime = &pair.0;
        let r_2 = &pair.1;

        e_0 = &r_0 * r_1_prime / (r_1_prime + &r_1 * r / fee_denominator);
        e_1 = r * &r_1 * r_2 / (r_1_prime + &r_1 * r / fee_denominator);
    }

    (e_0, e_1)
}

fn integer_sqrt(n: &BigUint) -> BigUint {
    if n.is_zero() {
        return BigUint::zero();
    }

    let mut x = n.clone();
    let mut y = BigUint::one();
    while x > y {
        x = (&x + &y) / 2u32;
        y = n / &x;
    }
    x
}

// Helper function to convert U256 to BigUint if needed
fn u256_to_biguint(value: U256) -> BigUint {
    BigUint::from_str(&value.to_string()).unwrap()
}
*/