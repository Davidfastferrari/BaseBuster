use alloy::network::Ethereum;
use alloy::providers::RootProvider;
use alloy::transports::http::{Client, Http};
use log::{debug, info, warn};
use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use crate::calculation::Calculator;
use crate::events::Event;
use crate::gen::FlashQuoter;
use crate::market_state::MarketState;
use crate::quoter::Quoter;
use crate::AMOUNT;

// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub async fn simulate_paths(
    tx_sender: Sender<Event>,
    arb_receiver: Receiver<Event>,
    market_state: Arc<MarketState<Http<Client>, Ethereum, RootProvider<Http<Client>>>>,
) {
    // if this is just a sim run or not
    let sim: bool = std::env::var("SIM").unwrap().parse().unwrap();

    // blacklisted paths, some error in swapping that wasnt caught during filter
    let mut blacklisted_paths: HashSet<u64> = HashSet::new();

    // recieve new paths from the searcher
    while let Ok(Event::ArbPath((arb_path, expected_out, block_number))) = arb_receiver.recv() {
        // convert from searcher format into quoter format
        let converted_path: Vec<FlashQuoter::SwapStep> = arb_path.clone().into();

        // get the quote for the path and handle it appropriately
        // if we have not blacklisted the path
        if !blacklisted_paths.contains(&arb_path.hash) {
            // get an initial quote to see if we can swap
            // get read access to the db so we can quote the path
            match Quoter::quote_path(converted_path.clone(), *AMOUNT, market_state.clone()) {
                Ok(quote) => {
                    // if we are just simulated, compare to the expected amount
                    if sim {
                        if *(quote.last().unwrap()) == expected_out {
                            info!(
                                "Success.. Calculated {expected_out}, Quoted: {}, Path Hash {}",
                                quote.last().unwrap(),
                                arb_path.hash
                            );
                        } else {
                            // get a full debug quote path
                            let calculator = Calculator::new(market_state.clone());
                            let output = calculator.debug_calculation(&arb_path);

                            info!(
                                "\nPath Comparison (Hash: {})\n\
                                ----------------------------------------\n\
                                Initial Output {}\n\
                                Debug Output: {:#?}\n\
                                Actual Quote:   {:#?}\n\
                                Path Steps:     {:#?}\n",
                                arb_path.hash,
                                expected_out,
                                output,
                                quote,
                                converted_path,
                            );
                        }
                    } else {
                        /*
                        let (optimized_input, optimized_output) =
                            quoter.optimize_input(converted_path.clone()).unwrap();
                        info!(
                            "Optimized input... Optimal amount in {}, Optimized Amount out {}",
                            optimized_input, optimized_output
                        );
                        */

                        info!("Sim successful... Expected output: {}, Block {}", expected_out, block_number);

                        // send the optimize path to the tx sender
                        let optimized_input = *AMOUNT;
                        match tx_sender.send(Event::ArbPath((arb_path, optimized_input, block_number))) {
                            Ok(_) => debug!("Simulator sent path to Tx Sender"),
                            Err(_) => warn!("Simulator: failed to send path to tx sender"),
                        }
                    }
                }
                Err(quote_err) => {
                    warn!("Failed to simulate quote {}, {:#?} ", quote_err, arb_path.hash);
                    blacklisted_paths.insert(arb_path.hash);
                }
            }
        }
    }
}