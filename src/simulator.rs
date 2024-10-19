use alloy::primitives::U256;
use log::{info, warn};
use std::sync::mpsc::{Receiver, Sender};

use crate::events::Event;
use crate::gen::FlashQuoter;
use crate::quoter::Quoter;

// recieve a stream of potential arbitrage paths from the searcher and
// simulate them against the contract to determine if they are actually viable
pub async fn simulate_paths(tx_sender: Sender<Event>, mut arb_receiver: Receiver<Event>) {
    // Construct a new quoter
    let mut quoter = Quoter::new().await;

    // if this is just a sim run or not
    let sim: bool = std::env::var("SIM").unwrap().parse().unwrap();

    // recieve new paths from the searcher
    while let Ok(Event::ArbPath((arb_path, expected_out))) = arb_receiver.recv() {
        // convert from searcher format into quoter format
        let converted_path: Vec<FlashQuoter::SwapStep> = arb_path.into();

        // get the quote for the path and handle it appropriately
        let amount_in = U256::from(1e16);
        match quoter.quote_path(converted_path, amount_in) {
            Ok(quote) => {
                if sim {
                    if quote == expected_out {
                        info!("Success.. Calculated {expected_out}, Quoted: {quote}");
                    } else {
                        info!("Fail.. Calculated {expected_out}, Quoted: {quote}");

                    }
                } else {
                    todo!()
                }

            }
            Err(quote_err) => warn!("Failed to simulate quote for {}", quote_err),
        }
    }
}

