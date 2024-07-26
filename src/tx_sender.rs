use crate::events::ArbPath;
use log::info;
use tokio::sync::broadcast::Receiver;

pub async fn send_transactions(mut tx_receiver: Receiver<ArbPath>) {
    while let Ok(arb_path) = tx_receiver.recv().await {
        info!("Received arb path: {:?}", arb_path);
    }
}
