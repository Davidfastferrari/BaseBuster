use pool_sync::{Pool, PoolInfo};
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy::network::Network;
use alloy::transports::Transport;
use std::sync::Arc;
use std::collections::{HashSet, HashMap};

use crate::AMOUNT;
use crate::calculation::Calculator;
use crate::market_state::MarketState;
use crate::swap::SwapPath;

const RATE_SCALE: u32 = 18; // 18 decimals for rate precision

// Handles initial estimation of path profitability before moving onto
// precise calculations and simulation
struct Estimator<T, N, P> 
where 
    T: Transport + Clone, 
    N: Network,
    P: Provider<T, N>
{
    // Mapping from pool address => token => rate
    rates: HashMap<Address, HashMap<Address, U256>>,
    // Tracks if a pool is based in weth 
    weth_based: HashMap<Address, bool>,
    // Calculator to calculate the outputs of swaps
    calculator: Calculator<T, N, P>,
    // Maps from a quote token to its aggregated weth rate
    aggregated_weth_rate: HashMap<Address, U256>
}

impl<T, N, P> Estimator<T, N, P> 
where 
    T: Transport + Clone, 
    N: Network,
    P: Provider<T, N>
{

    // Construct a new estimator
    pub fn new(market_state: Arc<MarketState<T, N, P>>) -> Self {
        Self {
            rates: HashMap::new(),
            weth_based: HashMap::new(),
            calculator: Calculator::new(market_state),
            aggregated_weth_rate: HashMap::new()
        }
    }

    // Given an initial set of filtered pools, estimate the exchange rates 
    pub fn process_pools(&mut self, pools: Vec<Pool>) {
        let weth: Address = std::env::var("WETH").unwrap().parse().unwrap();
        let mut alt_tokens: HashSet<Address> = HashSet::new();
        let mut weth_alt_cnt: HashMap<Address, u32> = HashMap::new();

        // amount is our arb input, this is to generalize the exchange rates to 
        // whatever we are trying to initially arb with
        let eth_input = *AMOUNT;

        // calcualte the rate for all pools with weth as a base/quote, we are very confident in these quotes
        for pool in pools.iter().filter(|p| p.token0_address() == weth || p.token1_address() == weth) {
            self.weth_based.insert(pool.address(), true);
            self.process_eth_pool(pool, weth, eth_input, &mut alt_tokens, &mut weth_alt_cnt);
        }

        // update the alt rates
        for token in alt_tokens {
            let aggregated_rate = self.aggregated_weth_rate.get(&token).unwrap();
            let averaged_rate = *aggregated_rate / U256::from(*weth_alt_cnt.get(&token).unwrap());
            *self.aggregated_weth_rate.get_mut(&token).unwrap() = averaged_rate;
        }

        // calculate the ratio for all pools that weth is neither a base/quote, this will use
        // an averaged input from the corresponding weth pair  
        for pool in pools.iter().filter(|p| p.token0_address() != weth && p.token1_address() != weth) {
            self.process_nonweth_pool(pool, eth_input);
        }
        // calculate every pool that is 
    }


    // Estimate the output from the rates for a swappath
    pub fn is_positive(&self, input: U256, swap_path: &SwapPath) -> bool {
        let mut rate = U256::from(1);
        todo!()
        // calculate the output rate of the path
    }


    // Calculate the rate for an weth based pool
    fn process_eth_pool(
        &mut self, 
        pool: &Pool, 
        weth: Address, 
        input: U256, 
        alt_tokens: &mut HashSet<Address>,
        weth_alt_cnt: &mut HashMap<Address, u32>
    ) {
        let pool_address = pool.address();
        let token0 = pool.token0_address();
        let token1 = pool.token1_address();

        // Get which token is weth and which is the quote token
        let (weth, alt) = if token0 == weth {
            (token0, token1)
        } else {
            (token1, token0)
        };
        alt_tokens.insert(alt);

        // get the output quote and then determine the rates
        let output = self.calculator.compute_pool_output(
            pool_address,
            weth,
            pool.pool_type(),
            pool.fee(),
            input
        );
        let zero_one_rate = output / input;
        let one_zero_rate =  input / output;

        // Initialize inner HashMap if it doesn't exist
        self.rates.entry(pool_address).or_insert_with(HashMap::new);
        
        // Insert the rates
        self.rates.get_mut(&pool_address).unwrap()
            .insert(token0, zero_one_rate);
        self.rates.get_mut(&pool_address).unwrap()
            .insert(token1, one_zero_rate);

        // update the aggregate
        if weth == token0 {
            *self.aggregated_weth_rate.entry(alt).or_insert(U256::ZERO) += zero_one_rate;
        } else {
            *self.aggregated_weth_rate.entry(alt).or_insert(U256::ZERO) += one_zero_rate;
        }
        *weth_alt_cnt.entry(alt).or_insert(0) += 1
    }

    fn process_nonweth_pool(&mut self, pool: &Pool, input: U256) {
        let pool_address = pool.address();
        let token0 = pool.token0_address();
        let token1 = pool.token1_address();

        let input = self.aggregated_weth_rate.get(&token0).unwrap();
        let output = self.calculator.compute_pool_output(
            pool_address,
            token0,
            pool.pool_type(),
            pool.fee(),
            *input
        );
        let zero_one_rate = output / input;
        let one_zero_rate =  input / output;
        // Initialize inner HashMap if it doesn't exist
        self.rates.entry(pool_address).or_insert_with(HashMap::new);
        
        // Insert the rates
        self.rates.get_mut(&pool_address).unwrap()
            .insert(token0, zero_one_rate);
        self.rates.get_mut(&pool_address).unwrap()
            .insert(token1, one_zero_rate);
    }

}
