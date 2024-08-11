use alloy::primitives::U256;
use log::{debug, info};
use std::sync::RwLock;
use tokio::sync::broadcast::Receiver;

pub struct Market {
    gas_price: RwLock<U256>,
}

impl Market {
    // Construct an empty market, populated on first update of block
    pub fn new() -> Self {
        Self {
            gas_price: RwLock::new(U256::from(0)),
        }
    }

    // updaate the gas price
    pub async fn update_gas_price(&self, mut gas_receiver: Receiver<u128>) {
        while let Ok(gas_price) = gas_receiver.recv().await {
            let mut gas_lock = self.gas_price.write().unwrap();
            *gas_lock = U256::from(gas_price);
            debug!("Updated gas price to {}", gas_price);
        }
    }
}
