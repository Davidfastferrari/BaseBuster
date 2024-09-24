use alloy::primitives::{U256, Address};
use tokio::sync::mpsc::{Sender, Receiver};
use log::{info, warn, debug};
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Instant;
use std::collections::{HashMap, HashSet};
use alloy::providers::Provider;
use alloy::transports::Transport;
use alloy::network::Network;

use crate::calculation::Calculator;
use crate::market_state::MarketState;
use crate::swap::{SwapStep, SwapPath};
use crate::events::Event;
use crate::AMOUNT;


// top level sercher struct
// contains the calculator and all path information
pub struct Searchoor<T, N, P> 
where 
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>
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
    P: Provider<T, N>
{
    // Construct the searcher with the calculator and all the swap paths
    pub async fn new(cycles: Vec<SwapPath>, market_state: Arc<MarketState<T, N, P>>) -> Self {
        let calculator = Calculator::new(market_state).await;

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

        Self { calculator, cycles, path_index: index, min_profit}
    }


    pub async fn search_paths(
        &mut self,
        paths_tx: Sender<Event>,
        mut address_rx: Receiver<Event>,
    ) {
        // wait for a new single with the pools that have reserved updated
        while let Some(Event::PoolsTouched(pools)) = address_rx.recv().await {
            info!("Searching for arbs...");
            let start = Instant::now();

            // invalidate all updated pools in the cache
            //self.calculator.invalidate_cache(&updated_pools);

            // from the updated pools, get all paths that we want to recheck
            let affected_paths: HashSet<&SwapPath> = pools
                .iter()
                .filter_map(|pool| self.path_index.get(pool))
                .flatten()
                .map(|&index| &self.cycles[index])
                .collect();
            info!("{} touched paths", affected_paths.len());

            // get the output amount and check for profitability
            let profitable_paths: Vec<(Vec<SwapStep>, U256)> = affected_paths
                .par_iter()
                .filter_map(|path| {
                    let output_amount = self.calculator.calculate_output(&path);
                    println!("{:?}", output_amount);
                    if output_amount >= self.min_profit {
                        Some((path.steps.clone(), output_amount))
                    } else {
                        None
                    }
                }).collect();

            info!("{:?} elapsed", start.elapsed());
            info!("{} profitable paths", profitable_paths.len());

            // send to the simulator
            for path in profitable_paths {
                match paths_tx.send(Event::ArbPath((path.0, path.1))).await{
                    Ok(_) => debug!("Sent path"),
                    Err(_) => warn!("Failed to send path")
                }
            }
        }
    }
}
