use log::info;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

use crate::FlashSwap::SwapStep;
use crate::market::Market;

pub async fn send_transactions(mut tx_receiver: Receiver<Vec<SwapStep>>, market: Arc<Market>) {
    while let Ok(arb_path) = tx_receiver.recv().await {
        //info!("Received arb path: {:?}", arb_path);
    }
}
