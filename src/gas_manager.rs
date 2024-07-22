use alloy::transports::http::{Http, Client};
use alloy::providers::{Provider, RootProvider};
use alloy::primitives::U256;
use log::info;
use alloy::rpc::types::Block;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::Sender;

use crate::events::Event;



pub struct GasPriceManager {
    provider: Arc<RootProvider<Http<Client>>>,
    base_fee_multiplier: f64, // figure out what this is
    max_priority_fee: u128,
}

impl GasPriceManager {
    pub fn new(provider: Arc<RootProvider<Http<Client>>>, base_fee_multiplier: f64, max_priority_fee: u128) -> Self {
        Self {
            provider,
            base_fee_multiplier,
            max_priority_fee,
        }
    }

    // max fee per gas: max total fee you are willing to pay
    // max priority fee: max priority fee you are willing to pay
    // actual base fee: this is the fee set by the network
    // actual priority fee: min(max priority fee, max fee per gas - actual base fee)
    // 1) you always pay the full base fee, this gets burned
    // 2) you pay up to your max priority fee, but no more than whats left after subtracting base free from max fee per gas
    // 3) if base fee exceeds max fee per gas, tx wont process
    // 4) your actual total fee per gas will never exceeed max fee per gas
    pub async fn update_gas_price(&self, mut block_receiver: Receiver<Event>, gas_sender: Sender<u128>)  {
        while let Ok(Event::NewBlock(block)) = block_receiver.recv().await {
            let base_fee_per_gas = block.header.base_fee_per_gas.unwrap();
            let priority_fee = self.provider.get_max_priority_fee_per_gas().await.unwrap();
            let adjusted_priority_fee = priority_fee.min(self.max_priority_fee);

            let total_fee= base_fee_per_gas + adjusted_priority_fee; //* self.base_fee_multiplier) + adjusted_priority_fee;
            match gas_sender.send(total_fee) {
                Ok(_) => info!("Gas price sent"),
                Err(e) => info!("Gas price send failed: {:?}", e)
            }
        }
    }
}
