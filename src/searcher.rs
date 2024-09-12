
use crate::calculation::Calculator;
use crate::AMOUNT;
use alloy::primitives::{U256, Address};
use tokio::sync::broadcast::Receiver;
use std::sync::mpsc::Sender;
use crate::events::Event;
use log::{info, warn, debug};
use rayon::prelude::*;
use std::sync::Arc;
use crate::pool_manager::PoolManager;
use crate::swap::{SwapStep, SwapPath};
use std::collections::HashSet;
use std::time::Instant;
use std::collections::HashMap;

pub struct Searchoor {
    calculator: Calculator,
    path_index: HashMap<Address, Vec<usize>>,
    cycles: Vec<SwapPath>,
}

impl Searchoor {
    // Construct the searcher with the calculator and all the swap paths
    pub async fn new(cycles: Vec<SwapPath>, pool_manager: Arc<PoolManager>) -> Self {
        let calculator = Calculator::new(pool_manager).await;

        // make our path mapper for easily getting touched paths
        let mut index: HashMap<Address, Vec<usize>> = HashMap::new();
        for (path_index, path) in cycles.iter().enumerate() {
            for step in &path.steps {
                index.entry(step.pool_address).or_default().push(path_index)
            }
        }

        Self { calculator, cycles, path_index: index}
    }
    pub async fn search_paths(
        &mut self,
        arb_sender: Sender<Event>,
        mut reserve_update_receiver: Receiver<Event>,
    ) {
        let flash_loan_fee: U256 = U256::from(9) / U256::from(10000); // 0.09% flash loan fee
        let min_profit_percentage: U256 = U256::from(2) / U256::from(100); // 2% minimum profit
        let initial_amount = U256::from(AMOUNT);
        let repayment_amount = initial_amount + (initial_amount * flash_loan_fee);
        let min_profit_amount = repayment_amount + (initial_amount * min_profit_percentage);

        // wait for a new single with the pools that have reserved updated
        while let Ok(Event::ReserveUpdate((updated_pools, block_number))) = reserve_update_receiver.recv().await {
            info!("Searching for arbs...");
            let start = Instant::now();

            self.calculator.invalidate_cache(&updated_pools);

            // from the updated pools, get all paths that we want to recheck
            let affected_paths: HashSet<&SwapPath> = updated_pools
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
                    if output_amount >= min_profit_amount {
                        Some((path.steps.clone(), output_amount))
                    } else {
                        None
                    }
                }).collect();

            info!("{:?} elapsed", start.elapsed());
            info!("{} profitable paths", profitable_paths.len());

            // send to the simulator
            for path in profitable_paths {
                match arb_sender.send(Event::ArbPath((path.0, path.1, block_number))) {
                    Ok(_) => debug!("Sent path"),
                    Err(_) => warn!("Failed to send path")
                }
            }
        }
    }
}