use crate::events::Event;
use alloy::providers::{Provider, ProviderBuilder};
use std::sync::RwLock;
use tokio::sync::broadcast::Receiver;

pub struct Market {
    max_priority_fee_per_gas: RwLock<u128>,
    max_fee_per_gas: RwLock<u128>,
}

impl Market {
    pub fn new() -> Self {
        Self {
            max_fee_per_gas: RwLock::new(0),
            max_priority_fee_per_gas: RwLock::new(0),
        }
    }

    pub async fn update_gas_price(&self, mut block_receiver: Receiver<Event>) {
        let url = std::env::var("FULL").unwrap();
        let provider = ProviderBuilder::new().on_http(url.parse().unwrap());
        while let Ok(Event::NewBlock(_block)) = block_receiver.recv().await {
            let estimated_gas_fees = provider.estimate_eip1559_fees(None).await.unwrap();
            *self.max_fee_per_gas.write().unwrap() = estimated_gas_fees.max_fee_per_gas;
            *self.max_priority_fee_per_gas.write().unwrap() =
                estimated_gas_fees.max_priority_fee_per_gas;
        }
    }

    pub fn get_max_priority_fee(&self) -> u128 {
        *self.max_priority_fee_per_gas.read().unwrap()
    }

    pub fn get_max_fee(&self) -> u128 {
        *self.max_fee_per_gas.read().unwrap()
    }
}
