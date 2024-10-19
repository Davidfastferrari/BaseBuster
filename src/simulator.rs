use std::sync::mpsc::{Receiver, Sender};
use alloy::primitives::U256;
use log::{info, warn};

use crate::events::Event;
use crate::quoter::Quoter;
use crate::gen::FlashQuoter;

// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub fn simulate_paths(tx_sender: Sender<Event>, mut arb_receiver: Receiver<Event>) {
    // Construct a new quoter
    let mut quoter = Quoter::new();

    // recieve new paths from the searcher
    while let Ok(Event::ArbPath((arb_path, expected_out, ))) = arb_receiver.recv() {
        // convert from searcher format into quoter format
        let converted_path: Vec<FlashQuoter::SwapStep> = arb_path.into_iter().map(|step| step.into()).collect();

        // get the quote for the path and handle it appropriately
        let amount_in = U256::from(1e16);
        match quoter.quote_path(converted_path, amount_in) {
            Ok(quote) => {
                // send it to the tx Sender
                info!("Simulation success. Caluclated out {}. Simulated out {}", expected_out, quote);
            } 
            Err(quote_err) => warn!("Failed to simulate quote for {}", quote_err)
        }
    }
}