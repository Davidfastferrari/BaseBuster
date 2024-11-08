use pool_sync::{Pool, PoolInfo};
use alloy::primitives::{Address, U256};
use std::collections::HashMap;
use alloy::providers::Provider;
use alloy::network::Network;
use alloy::transports::Transport;
use std::sync::Arc;

use crate::AMOUNT;
use crate::calculation::Calculator;
use crate::market_state::MarketState;
use crate::swap::SwapPath;

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

        // amount is our arb input, this is to generalize the exchange rates to 
        // whatever we are trying to initially arb with
        let eth_input = *AMOUNT;

        // calcualte the rate for all pools with weth as a base/quote, we are very confident in these quotes
        for pool in pools.iter().filter(|p| p.token0_address() == weth || p.token1_address() == weth) {
            self.weth_based.insert(pool.address(), true);
            self.process_eth_pool(pool, weth, eth_input);
        }

        // calculate every pool that is 
    }

    // Estimate the output from the rates for a swappath
    pub fn estimate_output(&self, input: U256, swap_path: &SwapPath) -> U256 {
        let mut rate = U256::from(1);
        // calculate the output rate of the path
        for swap in &swap_path.steps {
            // aggreagate the rate
            rate *= self.rates[&swap.pool_address][&swap.token_in]
        }
        rate * input
    }


    // Calculate the rate for an weth based pool
    fn process_eth_pool(&mut self, pool: &Pool, weth: Address, input: U256) {
        // Get which token is weth and which is the quote token
        let (weth_is_token0, quote_token) = if pool.token0_address() == weth {
            (true, pool.token1_address())
        } else {
            (false, pool.token0_address())
        };

        // Calculate output for the weth -> quote direction
        let output = self.calculator.compute_pool_output(
            pool.address(),
            if weth_is_token0 { pool.token0_address() } else { pool.token1_address() },
            pool.pool_type(),
            pool.fee()
        );

        // Normalize to 18 decimals (multiply up if decimals < 18)
        let quote_decimals = pool.token1_decimals() as u8;
        let normalized_output = output * U256::from(10).pow(U256::from(18 - quote_decimals));

        // Calculate rates with 18 decimal precision
        let precision = U256::from(10).pow(U256::from(18));
        
        // weth -> quote rate
        let weth_to_quote_rate = (normalized_output * precision) / input;
        // quote -> weth rate (inverse)
        let quote_to_weth_rate = (input * precision) / normalized_output;

        // Store both rates in the hashmap
        let mut rates = HashMap::new();
        rates.insert(weth, weth_to_quote_rate);
        rates.insert(quote_token, quote_to_weth_rate);
        self.rates.insert(pool.address(), rates);
    }



}




// so, the funciton of this is to estimate the exchange rate of a pool so that we can calculate quotes much much rater
// for example, lets assume eth/usdc 1 eth = 1000usdc, then usdc/eth on sushi is 1 eth = 900 usdc
// the rate is 1000 usdc = 1 eth and 1 usdc = .00111111 eth. /
// if we have input of 1 eth, the output is 1 * 1000 * .00111111 which gives a positive arbitrage path.
// this makes sense, and now we need to genearlize it to many differnt pools, and many differnt paths
// if there is eth as a quote/base in the pool, this is easy. we will assume a constant input amount of .5 eth. 
// so something like eth/usdc is east because we quote it eith .5 eth to get the exchange rate.
// this gets harder with indermediate bools where we dont ahve eth as a base/quote. amms have non uniform price curves
// so we have to find a input amount that is general enough to get us a reaonsle output amount. we just want a very good estimate her
// for example, if .5 eth = 10,000,000 bonk, we are not going to uqote 1 bonk = x usdc since inputting 1 bonk and 10,000,000 bonk is going to result
// in a completely different price. At the same time, there might be different quotes bot bonk. on uni .5 eth might be 11,000,000 bonk and on 
// sushi .5 eth is 10,000,000 bonk, and for these pools where its bonk and something like pepe, we need a good input amount so we have resaonble estimates. 
// im thinking concurrent hashmap where the key is the pool address and then it has a inner struce with teh zero token first, that exchange erate,
// and another for the one token first, that exchange rate, and doing this for every pool, the problem is given n pools, I need to be ablet o ppoulate this map algorithmically
// and determine everything here from exchange rates to everythinge else, on each reserve update/state update, I will also update these exchange rates.so many I need to store something
// like what I am using as the input for the quote. 