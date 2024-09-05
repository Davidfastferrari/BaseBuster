
use crate::calculation::Calculator;
use alloy::primitives::U256;
use tokio::sync::broadcast::{Receiver, Sender};
use crate::events::Event;
use log::{info, warn};
use rayon::prelude::*;
use crate::graph::SwapStep;



pub struct Searchoor {
    calculator: Calculator,
    cycles: Vec<Vec<SwapStep>>
}


impl Searchoor {
    // Construct the searcher with the calculator and all the swap paths
    pub async fn new(cycles: Vec<Vec<SwapStep>>) -> Self {
        let calculator = Calculator::new().await;
        Self { calculator, cycles }
    }

    pub async fn search_paths(
        &self,
        arb_sender: Sender<Event>,
        mut reserve_update_receiver: Receiver<Event>,
    ) {
        let flash_loan_fee: U256 = U256::from(9) / U256::from(10000); // 0.09% flash loan fee
        let min_profit_percentage: U256 = U256::from(2) / U256::from(100); // 2% minimum profit
        let initial_amount = U256::from(1e17);
        let repayment_amount = initial_amount + (initial_amount * FLASH_LOAN_FEE);

        // wait for a new single with the pools that have reserved updated
        while let Ok(Event::ReserveUpdate(updated_pools)) = reserve_update_receiver.recv().await {
            info!("Searching for arbs...");
            let start = std::time::Instant::now(); // timer

            let affected_paths: Vec<SwapPath> = panic!("no implemente");

            let profitable_pahts: Vec<Vec<SwapStep>> = affected_paths
                .par_iter()
                .flat_map(|&path| {
                    let output_amount = path.calculate_output(initial_amount);
                    if output_amount > repayment_amount {
                        path.steps
                    } else {
                        None
                    }
                }).collect();







                /* 
            // get all paths that were touched
            let affected_paths: Vec<usize> = updated_pools
                .iter()
                .flat_map(|pool| self.path_index.get(pool).map(|indices| indices.clone()))
                .flatten()
                .collect();
            info!("Searching {} paths", affected_paths.len());

            // check the profitability of each path
            let profitable_paths: Vec<_> = affected_paths
                .par_iter()
                .filter_map(|&path_index| {
                    let cycle = &self.cycles[path_index];
                    let initial_amount = U256::from(amount);
                    let mut current_amount = initial_amount;



                    for swap in cycle {
                        current_amount = self.calculator.get_amount_out(
                            current_amount,
                            &self.pool_manager,
                            swap,
                        );
                        if current_amount <= U256::from(AMOUNT) {
                            return None;
                        }
                    }

                    if current_amount >= repayment_amount {
                        let profit = current_amount - repayment_amount;
                        let profit_percentage = profit * U256::from(10000) / initial_amount;

                        if profit_percentage >= MINIMUM_PROFIT_PERCENTAGE * U256::from(10000) {
                            Some((cycle.clone(), profit))
                        } else {
                            None
                        }
                    } else {
                        None
                    }

                    //if current_amount >= required_amount * PROFIT_THRESHOLD {//* FLASH_LOAN_FEE {
                    //println!("path: {:#?} Current amount: {:#?}", cycle, current_amount);
                    //Some((cycle.clone(), current_amount))
                    //} else  {
                    //   None
                    //}
                })
                .collect();

            //info!("Searched all paths in {:?}", start.elapsed());
            //   .as_millis();
            //info!("done sim at timestamp {}:", now);
            //info!("Found {} profitable paths", profitable_paths.len());
            //simulate_quote(profitable_paths.clone(), U256::from(AMOUNT)).await;
            for path in profitable_paths {
                if let Err(e) = arb_sender.send(Event::NewPath(path.0)) {
                    warn!("Path send failed: {:?}", e);
                }
            }
        }
        */
    }
}


