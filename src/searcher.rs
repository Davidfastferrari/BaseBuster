use alloy::network::Network;
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy::transports::Transport;
use log::{debug, info, warn};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;

use crate::calculation::Calculator;
use crate::events::Event;
use crate::market_state::MarketState;
use crate::swap::SwapPath;
use crate::AMOUNT;

// top level sercher struct
// contains the calculator and all path information
pub struct Searchoor<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    calculator: Calculator<T, N, P>,
    path_index: HashMap<Address, Vec<usize>>,
    cycles: Vec<SwapPath>,
    min_profit: U256,
}

impl<T, N, P> Searchoor<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    // Construct the searcher with the calculator and all the swap paths
    pub fn new(cycles: Vec<SwapPath>, market_state: Arc<MarketState<T, N, P>>) -> Self {
        let calculator = Calculator::new(market_state);

        // make our path mapper for easily getting touched paths
        let mut index: HashMap<Address, Vec<usize>> = HashMap::new();
        for (path_index, path) in cycles.iter().enumerate() {
            for step in &path.steps {
                index.entry(step.pool_address).or_default().push(path_index)
            }
        }

        // calculate the min profit percentage
        let flash_loan_fee: U256 = U256::from(9) / U256::from(10000); // 0.09% flash loan fee
        let min_profit_percentage: U256 = U256::from(2) / U256::from(100); // 2% minimum profit
        let initial_amount: U256 = *AMOUNT;
        let repayment_amount = initial_amount + (initial_amount * flash_loan_fee);
        let min_profit = repayment_amount + (initial_amount * min_profit_percentage);

        Self {
            calculator,
            cycles,
            path_index: index,
            min_profit,
        }
    }

    pub fn search_paths(&mut self, paths_tx: Sender<Event>, address_rx: Receiver<Event>) {
        let sim: bool = std::env::var("SIM").unwrap().parse().unwrap();

        // wait for a new single with the pools that have reserved updated
        while let Ok(Event::PoolsTouched(pools, block_number)) = address_rx.recv() {
            info!("Searching for arbs in block {}...", block_number);
            let res = Instant::now();

            // invalidate all updated pools in the cache

            //self.calculator.invalidate_cache(&pools);

            // from the updated pools, get all paths that we want to recheck
            let affected_paths: HashSet<&SwapPath> = pools
                .iter()
                .filter_map(|pool| self.path_index.get(pool))
                .flatten()
                .map(|&index| &self.cycles[index])
                .collect();
            info!("{} touched paths", affected_paths.len());

            // get the output amount and check for profitability
            let profitable_paths: Vec<(SwapPath, U256)> = affected_paths
                .par_iter()
                .filter_map(|path| {
                    let output_amount = self.calculator.calculate_output(path);
                    //let debug_quote = self.calculator.debug_calculation(path);
                    //assert_eq!(output_amount, *debug_quote.last().unwrap());

                    if sim {
                        // if this is a sim, we are concerened about correct amounts out
                        Some((path.clone().clone(), output_amount))
                    } else {
                        // this is not a sim, make sure it is a profitable path
                        if output_amount >= self.min_profit {
                            Some((path.clone().clone(), output_amount))
                        } else {
                            None
                        }
                    }
                })
                .collect();

            info!("{:?} elapsed calculating paths", res.elapsed());
            info!("{} profitable paths", profitable_paths.len());

            for path in profitable_paths {
                match paths_tx.send(Event::ArbPath((path.0, path.1, block_number))) {
                    Ok(_) => debug!("Sent path"),
                    Err(_) => warn!("Failed to send path"),
                }
            }
        }
    }
}
