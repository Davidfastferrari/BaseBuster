use alloy::primitives::{U256, Address};
use log::{debug, info, warn};
use std::sync::mpsc::{Receiver, Sender};
use std::collections::HashSet;

use crate::events::Event;
use crate::gen::FlashQuoter;
use crate::quoter::Quoter;

// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub async fn simulate_paths(tx_sender: Sender<Event>, arb_receiver: Receiver<Event>) {
    // Construct a new quoter
    let mut quoter = Quoter::new().await;

    // if this is just a sim run or not
    let sim: bool = std::env::var("SIM").unwrap().parse().unwrap();

    // blacklisted paths, some error in swapping that wasnt caught during filter
    let mut blacklisted_paths: HashSet<u64> = HashSet::new();

    // recieve new paths from the searcher
    while let Ok(Event::ArbPath((arb_path, expected_out))) = arb_receiver.recv() {
        // convert from searcher format into quoter format
        let converted_path: Vec<FlashQuoter::SwapStep> = arb_path.clone().into();

        // get the quote for the path and handle it appropriately
        let amount_in = U256::from(1e16);
        // if we have not blacklisted the path
        if blacklisted_paths.contains(&arb_path.hash) {
            // get a quote for the path
            match quoter.quote_path(converted_path.clone(), amount_in) {
                Ok(quote) => {
                    if sim {
                        // if we are just simulating, check we got the proper output
                        if quote == expected_out {
                            info!("Success.. Calculated {expected_out}, Quoted: {quote}, Path Hash {}", arb_path.hash);
                        } else {
                            info!(
                                "Fail.. Calculated {expected_out}, Quoted: {quote}, Path: {:#?}",
                                converted_path
                            );
                        }
                    } else {
                        // we need to optimize the amount in
                        let optimized_out = U256::ZERO;
                        // send the optimize path to the tx sender
                        match tx_sender.send(Event::ArbPath((arb_path, optimized_out))) {
                            Ok(_) => debug!("Sent path"),
                            Err(_) => warn!("Failed to send path"),
                        }
                    }
                }
                Err(quote_err) => {
                    warn!("Failed to simulate quote for {}", quote_err);
                    blacklisted_paths.insert(arb_path.hash);

                }
            }
        }
    }
}
